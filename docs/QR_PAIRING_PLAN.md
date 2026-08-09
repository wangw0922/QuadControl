# 方案:扫码配对连接流程(Mac/Windows → Android)— rev5(按 sol 五轮评审修正)

## 背景与问题

用户要求 Mac/Windows 控制安卓的连接体验对齐三星多屏联动:**安卓端扫描控制端显示的
二维码,经局域网 Wi-Fi 连接;或 USB 插线 + 允许调试**。

## 核心机制(全部官方,手机零安装)

Android 11+ 无线调试的二维码配对协议:

1. 控制端生成一次性 service name + password:
   - **service name 用官方形状 `studio-<10位随机>`**(Android Studio 同款;AOSP 设备端
     虽接受任意名,但只有该形状有 OEM 上的既成可靠性;不自创 `quadcontrol-` 前缀)
   - **password 固定 12 字符**,字符集仅 `[0-9A-Za-z]`(不含 `\ ; , :` 等会破坏
     WIFI QR token 的字符,从字符集层面免去 escaping;字符集本身写测试守护)
2. 渲染二维码 `WIFI:T:ADB;S:<name>;P:<password>;;`
3. 手机:开发者选项 → 无线调试 → 「使用二维码配对设备」扫码
   (首次启用无线调试时,手机可能额外要求确认信任当前 Wi-Fi 网络——文案要提)
4. 扫码后手机广播 `_adb-tls-pairing._tcp`;控制端**只接受 instance name 与本次
   `S:` 字段完全一致的服务**(多手机/多开发机同时配对时禁止按类型取第一条)
5. `adb pair <ip:port>`(地址来自精确匹配的服务),password 经 **piped stdin** 写入
6. **pair 成功后 adb 会自动尝试连接**:解析 pair 输出中的 GUID,轮询 `adb devices -l`
   等待对应 GUID/serial 进入 `device` 状态。**显式 `adb connect` 仅作为该 GUID
   自动连接失败后的降级路径**,endpoint 必须与该 GUID 对应,禁止从所有
   `_adb-tls-connect._tcp` 中任取
7. 连接就绪 → 进入镜像/控制会话(后续命令显式 -s <主 serial>)

### GUID / serial / connect instance 关联规则(定案)

- pair 成功输出形如 `Successfully paired to <ip:port> [guid=adb-XXXX-YYYY]`:
  以正则提取 `guid=` 后的整个 `adb-…` token,规范化 = 原样保留(不改大小写、
  不截断),记为 **round GUID**
- `adb devices -l` 中的无线条目 serial 判定为本轮设备,当且仅当
  `serial == GUID` 或 `serial 以 "GUID." 为前缀`(mDNS instance 形如
  `adb-XXXX-YYYY._adb-tls-connect._tcp`)
- 降级 connect 的 endpoint 只能取 instance name 满足上述同一判定的
  `_adb-tls-connect._tcp` 服务
- **GUID 解析失败**(adb 版本差异导致输出变化):进入显式错误态
  `pair 成功但输出无法解析`,提示重试或走手动 connect 降级;**禁止**退化为
  "取 devices 里新出现的任意无线条目"这类猜测
- 上述解析全部为纯函数,用真机输出固件做单测;adb 版本升级导致的输出漂移由
  固件测试第一时间暴露

### 同设备双条目去重(定案)

同一设备可能同时出现 `<ip:port>` 与 GUID 两个 serial。规则:
- **同源判定只认强身份**:`adb -s <serial> shell getprop ro.serialno` 两端一致。
  **不使用 product/model/device 三元组**(同型号两台手机三元组相同,会被错误合并);
  ro.serialno 读取失败(offline/unauthorized)时**不合并**,如实展示为两个条目
- **主 serial 选择:GUID 优先,而非恒取 GUID**——手动 `connect <ip:port>` 或
  mDNS 不可用的场景只会产生 ip:port 形式条目,此时 **ip:port 即主 serial**,
  全部功能照常工作(仅在换网/DHCP 续租后需重连,连接页文案注明)
- 会话与后续命令只用主 serial;去重成立时 ip:port 副条目不主动 disconnect
  (避免误伤),UI 只展示一台设备

## 证据边界(如实)

已实测(findings 2026-08-07,三星 SM-S9180,macOS):命令行 `adb pair <ip:port> <码>`、
`adb connect`、`adb mdns services` 发现两类服务。
**未实测(本切片新增验收项)**:QuadControl 生成 QR → 三星扫码 → 自动 pair →
对应设备 online 的全链路;Windows 全部路径。

## 密码与子进程纪律(定案,不留给实现)

- `adb pair <address>` 以参数数组直接 spawn,**不经过 shell**
- password 经 pipe 写入 stdin(AOSP `commandline.cpp` 用 `std::getline` 读取,
  Android Studio 同做法),写入 password+换行后立即关闭写端
- password 不进 argv、环境变量、文件、日志、错误详情;**完整 QR payload 同样不记录**
- 子进程设超时与输出上限;取消/超时必须 kill 并 reap
- 承诺范围:QuadControl 自身不记录 secret;不为第三方 adb 在用户主动开 trace 时的
  行为背书
- 不绕过 adb CLI 直接实现 pairing 协议(内部 API 不稳定 + 密钥管理负担)

## 预检(启动扫码流程前)

- `adb version` 解析 `Version <X.Y.Z>` 行(客户端版本),判定规则(定案):
  - **< 30.0.0:硬性拦截**(`adb pair` 于 platform-tools 30.0.0 引入),给升级指引
  - **≥ 30.0.0:放行**;实测基线 37.0.1,低于基线未实测的版本给一次性提示
  - **不对 mDNS backend 的实现/默认值做任何版本断言**(该实现在 adb 历史上
    多次变更,三审已证明按版本推断 backend 会引入错误事实)
  - 版本选择规则:以 `adb version` 输出的客户端版本为准。**不依赖"adb 自会
    重启旧 server"**(adb 只比较 server 协议版本,不同 platform-tools 构建
    不保证触发重启)。预检默认不动用户已有的 server;当客户端 ≥30 但
    `adb mdns check` 失败时,给出**用户显式触发**的「重启 adb server」操作
    (`adb kill-server` + `adb start-server`,提示会短暂断开其他 adb 会话),
    重启后重跑 mdns check——把"旧 server 不支持 mDNS"从死路变成一步可恢复
- **mDNS 可用性的唯一权威判定是运行时探测 `adb mdns check`**(它查询的是
  实际运行的 server),不做静态版本推断
- 版本解析为纯函数 + 固件单测
- **「mDNS 不可用」与「扫码超时」是两种不同的用户可见错误**,不得混同
- 已知限制如实报告:`adb mdns services` 只输出 IPv4 endpoint

## 失败路径与降级

| 状态 | 表现/恢复 |
|---|---|
| adb 缺失/过旧 | 预检拦截,给安装/升级指引 |
| mDNS backend 不可用 | 预检拦截,提示改用「配对码手动配对」或 USB |
| 扫码超时(QR 过期) | 计时器到期 → **round-close(timeout)** |
| 用户取消/退出扫码页/服务消失 | 按可观察事件分类(服务消失、pair 非零退出、UI 取消),**不假设存在稳定的"用户拒绝"错误码** → **round-close(cancelled / service-lost)** |
| 多手机同时扫码/网络多个 pairing 服务 | 不同名服务:精确 instance name 匹配天然免疫。**同名竞争**(多台手机扫了同一张码):首个 pair 成功者胜出 → **round-close(success)**;败方手机侧自然显示配对失败 |
| pair 成功但设备未 online | 等待超时后走该 GUID 的显式 connect 降级;再失败给诊断文案 |
| AP/client 隔离 | **手输 IP 无法绕过**(单播同样被断)。降级:USB / 换非隔离 Wi-Fi / 两端连同一移动热点(Android 官方同建议) |
| mDNS 被拦但单播可达 | 引导用户走手机「使用配对码配对设备」页:**pairing IP:port + 手机屏幕上的 6 位码**(不能复用 QR secret——手机不会显示它);connect 阶段用无线调试主页的 **connect IP:port(与 pairing 端口不同)** |
| Wi-Fi 切换/无线调试关闭/endpoint 变化 | 会话断开检测 + 重连引导 |
| USB:未开调试/unauthorized/offline | 分状态给文案;Windows 额外提示可能需要 OEM USB 驱动 |

手动降级路径必须区分两组端口:「配对码配对」页的 pairing IP:port + 6 位码,与
无线调试主页的 connect IP:port——两者端口通常不同,UI 分开引导。

### 统一 round-close 流程(定案)

成功、超时、取消、服务消失四种出口走**同一个**关闭函数,唯一差别是结束原因码。
动作固定为:kill 并 reap 全部在飞 pair 子进程 → round-id 作废(此后迟到的
**mDNS 事件与 pair 结果**一律按 round-id 丢弃)→ secret 清零(边界见下)→
二维码从 UI 移除。不允许任何出口路径绕过该函数或只执行子集。

**事件仲裁(防误杀竞态)**:`_adb-tls-pairing._tcp` 只在配对进行期间广播,
**成功路径上服务撤销可能先于 pair 成功输出到达**。因此规则为:只要本轮还有
pair 子进程在运行,`service-lost` 事件**不得单独终结轮次**——先等待该子进程
退出(有限超时):退出成功 → 消费输出、提取并保存 GUID,再执行
round-close(success);退出失败或超时 → 才执行 round-close(service-lost)。
`service-lost` 立即生效的唯一情形是当前没有任何在飞 pair 子进程。

**secret 清零边界(如实)**:清零承诺仅限 QuadControl 自有内存中的副本
(生成器持有的 secret、QR payload 字符串、传给渲染器的缓冲);写入 stdin 后
关闭写端。**OS 管道内核缓冲与 adb 进程自身内存不受本进程控制**,不做无法
验证的承诺——这与既有的"不为第三方 adb 的 trace 行为背书"同一边界。

## 模块归属(定案)

- **`quadcontrol-android::wireless`**(新模块):QR payload 纯函数、完整 mDNS entry
  解析(instance/地址/端口)、pair/connect 状态机、子进程超时与取消。与该 crate
  现有职责(设备选择、scrcpy 生命周期)同属产品路径,符合 GUI_PLAN 第 4/7/27 行边界
- **`adb-probe` 保持严格只读**,不加 pair/connect,不扩命令白名单;其 `parse_mdns`
  只出布尔,不满足需要——解析逻辑在 wireless 模块内新写;若抽公共纯 parser,只共享
  解析函数,不共享 runner
- **`crates/pairing` 不放 ADB 特有逻辑**(现为通用骨架;将来真出现跨设备类型的通用
  配对状态模型再抽)
- CLI:**挂在现有 `quadcontrol-scrcpy` 二进制下新增三个子命令**(不新增 bin;
  该 bin 已是 Android 象限产品 CLI 入口):
  - `pair-qr`:主路径,终端渲染二维码 + 自动闭环
  - `pair-code <ip:port>`:手动降级路径,6 位配对码经交互式 stdin 输入
    (不作为参数,与 QR 路径同一 secret 纪律)
  - `connect <ip:port>`:显式 connect 降级(对应无线调试主页的 connect 端口)
  - 三个子命令共用 wireless 模块同一状态机,GUI 连接页的「手动配对」链接
    即这两条降级路径的图形入口

## GUI(G3 重塑为连接优先)

- 打开即见两张卡:「扫码连接」(大二维码 + 手机端路径指引 + 剩余有效时间)与
  「USB 连接」(检测到即亮)
- 二维码只在本机屏幕显示;每次打开/过期重新生成
- 无线延迟劣于 USB(实测 shell 44ms vs 2ms),连接页标注低延迟场景建议 USB
- Windows 同一 Rust 核心复用;**Windows 定位为「官方支持、仓库尚未实测」**

## 验收

- 单测:QR payload 纯函数(字符集守护)、mDNS 输出解析(真机固件样本,含多服务
  竞争样本)、pair 输出 GUID 提取(真机固件 + 解析失败样本)、adb 版本判定、
  双条目去重、状态机错误分型、轮次关闭(迟到事件按 round-id 丢弃)、
  **事件仲裁两条关键顺序**:
  ① `service-lost` 到达时 pair 在飞 → pair 成功输出被消费、GUID 提取保存 →
  `round-close(success)`(服务撤销不得抢先杀掉获胜进程);
  ② `service-lost` 到达时 pair 在飞 → pair 失败或等待超时 →
  `round-close(service-lost)`
- 真机(macOS + SM-S9180):QR 全链路(生成→扫码→自动 pair→GUID online→镜像);
  **手动降级路径 `pair-code` 与 `connect` 两条全链路各自实测**;USB 回归;
  扫码超时、退出扫码页、pair 后拔 Wi-Fi 三个失败路径实测
- **Windows 门禁(后置,与现有 Windows 待验证项合并)**:真机全链路 + 防火墙提示 +
  冷启动/已有 adb server 两种状态
- CI:纯函数与解析测试三平台

## 切片顺序

1. `quadcontrol-android::wireless` 模块 + 单测(luna)
2. `quadcontrol-scrcpy` 三个子命令 **`pair-qr` / `pair-code` / `connect`** 同片交付,
   macOS 真机三条链路各自验收(luna 实现,Fable 实测)
3. GUI 连接页(G3 重塑范围)
4. Windows 真机验证(合并进现有 Windows 验证批次)

## 红线(不变)

不动手机刷新率;不绕锁屏;会话结束恢复原状;probe 只读边界不动。
