# QuadControl 总体计划：从现状到终态

更新：2026-08-10。本文回答一件事：**从今天的仓库状态出发，还差哪些步、每步的
边界与验收是什么，才算达到最终产品效果**。

权威关系：控制方向与架构事实以 [CONTROL_ARCHITECTURE.md](CONTROL_ARCHITECTURE.md)
为准；产品要求以 [PRODUCT_REQUIREMENTS.md](PRODUCT_REQUIREMENTS.md) 为准；GUI
切片细节以 [GUI_PLAN.md](GUI_PLAN.md) 为准；当前进度以 [STATUS.md](STATUS.md)
为准。本文不复述它们的内容，只做**路线整合**——与上述文档冲突时以上述文档为准。

---

## 一、终态定义（验收得了的"最终效果"）

一名普通用户（Android 象限）或开发者用户（iPhone 象限）在 Windows、macOS 或
Linux 任一台式机上安装 QuadControl 后：

1. 打开桌面端即见**设备列表**（真实发现的 Android 与 iPhone 设备，含状态）。
2. 新手机通过**扫码配对**接入（≤3 步，绝不要求手输 43 字符令牌）。
3. 对 Android 设备**一键开始会话**：scrcpy 独立窗口出画面与控制，桌面端提供
   声音输出切换、熄屏控制、截屏、断开；会话结束后设备侧亮度/旋转/超时恢复，
   无孤儿进程。
4. 对 iPhone（Windows/Linux）：GUI 内渲染 WDA MJPEG 画面，支持点按转发、文字
   发送、亮屏/回主屏/截屏；（macOS）出引导卡一键调起系统 iPhone Mirroring。
5. 手机端：iOS App 与 Android App 各自提供「扫描配对码」入口、连接状态、以及
   被控期间**始终可见**的提示与一键断开。
6. 三平台提供安装包；界面全部 zh-Hans + en 双语。

终态**不包含**（边界，见下节）：macOS→iPhone 的任何自研控制、Windows/Linux→
iPhone 的熄屏控制与商店分发、任何绕过锁屏/隐藏控制的能力。

## 二、系统边界（不随实现进度改变的硬约束）

### 2.1 引擎归属——我们只做封装与统一控制端

| 象限 | 引擎 | 我们交付的边界 |
|---|---|---|
| Win/macOS/Linux → Android | scrcpy（外部，Apache-2.0，钳制 ≥4.1 拒 prerelease） | 发现、配对、启动配置、生命周期与清理、GUI。**视频窗口归 scrcpy 自渲染，GUI 不渲染 Android 视频** |
| Win/Linux → iPhone | go-ios + WebDriverAgent | GUI 内渲染 MJPEG、控制转发、配置引导。**UI 自动化而非输入注入**，游戏/多指/高帧率场景达不到 scrcpy 水平，此为机制边界 |
| macOS → iPhone | Apple iPhone Mirroring | **只做检测与 `open -b com.apple.ScreenContinuity` 引导**。驱动/注入/自动化其界面 = 越界，禁止 |

### 2.2 安全与产品红线（全阶段有效）

- 绝不修改设备显示刷新率（用户红线）；限帧只在编码器侧。
- 除 Android scrcpy 已明确约束的例外外不使用私有 API；不越狱、不 Root、不绕过
  锁屏/密码/Face ID；不隐藏控制、不默认无人值守；所有会话用户主动发起、可见、
  可断开。
- GUI 中**不存在**刷新率选项这一控件本身。
- 配对令牌不得缩短或降熵（HMAC 转录可被同网被动监听离线爆破）；低摩擦路径 =
  扫码，不是弱令牌。
- 设备标识不入仓库；采集的屏幕内容验证后删除；Team ID 只走命令行参数。
- 双语（zh-Hans + en）覆盖一切用户可见字符串；机器可读输出保持英文。
- 会话结束恢复设备亮度、旋转、超时。

### 2.3 能力不对称（必须如实呈现在 UI 与文档中）

- 熄屏控制：→Android 可达（已实测）；macOS→iPhone 由 Apple 覆盖（未实测）；
  **Win/Linux→iPhone 不可达**，UI 不得暗示。
- Win/Linux→iPhone 需开发者证书 + WDA 安装，**不能作为消费者商店产品**。
- 音频：iPhone 声音不能转发到电脑（系统限制），UI 明示。

### 2.4 基础设施边界（2026-08-10 现实）

- CI 跑在**本机自托管 runner**：`qc-macos`（本机 Mac，launchd 常驻）+
  `qc-ubuntu`（本地 QEMU Ubuntu 22.04.5 arm64 虚拟机，systemd 常驻）。私有
  仓库不计 Actions 分钟。PR 跑 ubuntu，push main 跑 ubuntu+macOS 全矩阵。
- **Windows 目前无 CI、无真机验证环境**——这是当前最大的覆盖缺口，影响 P2 与
  P6（见下）。GitHub 托管分钟 2026-09-01 恢复后可临时用 `windows-2025` 补测。
- Linux 运行时基线 = Ubuntu 22.04 LTS；本地 VM 为 arm64，x86_64 尚无实跑环境。

## 三、现状基线（截至 2026-08-10，PR #17 已合并）

已完成（详见 STATUS.md）：macOS→Android scrcpy 4.1 全链路含熄屏；
`quadcontrol-android`（Session 库 + CLI，扫码配对 `pair-qr`）；iPhone WDA 链路
经 macOS 宿主实测；GUI G0 空壳 + **界面 A/B 高保真前端已合并**（设计令牌、双语、
本地 OFL 字体；A/B 目前接 mock 数据）；自托管 CI 全链路绿；设计交接包
`design_handoff_quadcontrol_gui/` 入库（A–E 五界面规范）。

`run_adb` 未被调用的 stdin 分支已删除（#18，2026-08-10 合并）——sol 评审发现的
潜伏缺陷，当时不可达。

## 四、分步计划

约定：每步给出【范围】【验收】【依赖/风险】。评审深度按 CLAUDE.md 的变更分级
执行，不在此重复。GUI 切片的技术细节（所有权模型、退出序列、Slint 止损条件等）
以 GUI_PLAN.md 为准，本文只标注它们在总路线中的位置与顺序。

### P0 收尾清理（当前批次遗留，短）

- ~~**P0.1 裁决 `run_adb` stdin 分支删除**~~：**已完成**（#18，2026-08-10）。
- ~~**P0.2 G0 Linux 运行时闭环**~~：**已完成**（2026-08-10，本地 Ubuntu 22.04.5
  arm64 虚拟机实跑）。**G0 关闭，未触发 Slint 止损条件。** 实测：
  - 真实 GNOME/mutter 桌面上正常渲染 960×640 窗口；中文（zh_CN.UTF-8）全部正确。
  - MJPEG 持续 **14.55 fps / 5 分钟**（阈值 ≥10），服务端背压计数 **0**；
    应用 RSS 441.9 MB → 444.3 MB（+0.55%），WebKit 进程 +0.83%，**无泄漏**。
  - CSP `img-src 'self' http://localhost:*` 实证可用（流被消费且画面渲染出来）。
  - **边界**：MJPEG 源为**合成流**（本机未装 go-ios，现搭 WDA 属另一条链路）。
    本次证明的是渲染端能力；**WDA 特有行为的验证归 P4**，届时用真流复测。
  - **附带发现（待修，见 P1）**：英文 locale 下界面 A 头部在 960px 宽度溢出，
    设备名被压成 `P…`；中文字符串较短所以在 macOS 上未暴露。
  【遗留风险】arm64 与 x86_64 的 WebKitGTK 差异仍未覆盖，留给 P6。

### P1 设备列表真数据（= GUI_PLAN G1，界面 A 左栏落地）

- 【范围】adb probe 解析 + `ios list` 双列接入 Tauri 后端 command（async +
  `spawn_blocking`）；界面 A 侧栏从 mock 切换为真实设备与状态徽章；轮询与
  在线/离线迁移；**顺带修 P0.2 发现的英文头部溢出**。
- 【验收】真机 Android + iPhone 同时插拔，列表 5 秒内收敛且无 UI 冻结；
  双语；`cargo test` 覆盖解析与状态机。

### P1.5 会话监督库（= GUI_PLAN G1.5，G2 前置）

- 【范围】`quadcontrol-android` 增加 `try_wait` 非消费轮询与 `SessionHandle`；
  监督线程独占 Session；stop 通道；库统一暴露有界等待期限（> 终止梯子总时长）。
- 【验收】库单测覆盖正常停止/超时强杀/进程自亡三路径；无孤儿（复用现有
  no-orphan 测试模式）。

### P2 Android 一键会话（= GUI_PLAN G2，界面 A 主区落地）

- 【范围】起停按钮接 Session；状态卡（运行/退出码/stderr 尾）；熄屏与声音
  选项接 scrcpy 参数；「断开连接」走 stop 通道 + 有界等待；退出拦截广播 stop。
- 【验收】macOS 真机验收：起停 20 轮无孤儿、无 forward 残留；界面 A 全部控件
  行为与设计稿一致。
- 【依赖/风险】**Windows Job Object（进程树回收）是本切片的合并门禁之一，但
  当前无 Windows 环境**。处置：Job Object 代码与单元测试先行合并（`cfg(windows)`
  编译由 CI 恢复后的 windows leg 或后续 Windows 主机验证），**真机无孤儿验收
  挂起为 P6 的显式关卡**，在 STATUS.md 如实标注"Windows 未验收"。

### P3 配对向导接真 adb（= GUI_PLAN G3，界面 B 落地）

- 【范围】`adb pair/connect` 作为新的有状态写操作独立设计（参数验证、超时、
  输出上限、安全边界；不冒充只读 probe）；界面 B 二维码区接真配对负载
  （host+port+token，30 秒轮换）；扫码成功自动进第 3 步并开始控制；手动输入
  表单复用同一校验。
- 【验收】真机扫码配对端到端 ≤3 步成功；配对密钥永不落日志；`pair-qr` 既有
  测试全绿（含本轮修复的竞争测试在慢 VM 上 50 连跑不挂）。

### P4 iPhone 控制面板（= GUI_PLAN G4，界面 C 落地）

- 【范围】按交接包界面 C 实现：左侧 246×500 WDA MJPEG 流（每会话独立本地
  端口，CSP `img-src` 仅 localhost）；点击画面转发点按；文字转发卡；亮屏/
  截屏/回主屏按钮（经 WDA）；「已连接 · N 帧/秒」标签；macOS 上替换为引导卡
  （`open -b com.apple.ScreenContinuity`）+「了解详情」。
- 【验收】iPhone SE 3 真机（经 go-ios）：MJPEG ≥10fps、点按/文字/三按钮全部
  生效；macOS 引导卡调起系统镜像；「声音保留在手机上」说明卡呈现。
- 【依赖】P0.2 已在 Linux VM 验证过 MJPEG 渲染路径。

### P5 手机端 App（界面 D / E）

- **P5.1 iOS 被控端更新（界面 D）**：现有 `apps/ios` SwiftUI 工程按交接包
  改造——扫码入口、手动输入表单（复用 ConnectionModel 校验）、状态卡接现有
  `ConnectionStatus`、安全注脚。字符串入 zh-Hans/en lproj。
  【验收】真机扫桌面端二维码完成配对；界面对照交接包截图核对。
- **P5.2 Android 被控端 App 新建（界面 E）**：三步卡、扫码连接、状态卡、
  常驻前台通知「正在被电脑控制」（可见性红线的落地形态）+ 点击断开。
  【验收】被控期间通知常驻不可清除；点击断开立即生效；熄屏时通知仍在。
  【风险】这是全新 App 工程（技术栈选型：优先 Kotlin 原生最小工程），是
  P5 中体量最大的一步，必要时再拆片。

### P6 Windows / Linux 真机引擎验证（消掉"未实测"标注）

- 【范围与验收，逐项打勾并同步 STATUS.md】
  1. Windows 上 scrcpy 起停 + 熄屏 + 清理（含 P2 挂起的 Job Object 真机无孤儿）；
  2. Windows 上 go-ios 隧道驱动 + WDA 全链路；
  3. Linux x86_64（基线 22.04）上 GUI 渲染 + scrcpy + go-ios；
  4. Windows/Linux 各自的 GUI 全功能冒烟（界面 A–C）。
- 【依赖】需要一台 Windows 主机（或 9/1 后托管 runner 补编译级验证——但真机
  设备验证仍需实体机）；Linux x86_64 需一个实体/虚拟环境。**此步不完成，
  终态第 1–4 条在 Win/Linux 上只是"预期可用"。**

### P7 打包发布（= GUI_PLAN G5）

- 【范围】三平台安装包（Tauri bundle：msi/dmg/deb+AppImage）；版本号与更新
  说明；scrcpy/go-ios 的获取与版本钳制策略（随包附带 vs 引导安装，需专项
  决策并记录进 DECISIONS.md）；LICENSES 汇总核对（scrcpy Apache-2.0、go-ios
  MIT、字体 OFL）。
- 【验收】三平台从安装包冷启动到完成一次 Android 会话与一次 iPhone 会话
  （macOS 为引导卡路径）。

### 横切任务（不阻塞主线，穿插进行）

- **CI-W**：Windows CI 腿恢复（9/1 托管分钟恢复后加回 `windows-2025`，或接入
  Windows 自托管 runner）。P2 的 `cfg(windows)` 代码与 P6 依赖它。
- **CI-SPEED**：**已解决**（2026-08-10）。把 `CARGO_TARGET_DIR` 移出被
  `actions/checkout` 的 `git clean -ffdx` 清掉的目录后，自托管 runner 的增量编译
  真正生效：单个 PR 的墙钟从约 25 分钟降到 **2 分钟以内**（GUI 21m24s → 1m11s，
  Rust 2m5s → 42s，同一份纯文档变更前后对照）。
  余下备选（当前不需要，若日后再变慢再考虑；属 CI 门禁变更需走完整评审）：
  Rust job 挪到空闲的 `qc-macos` 以消掉串行排队；PR 阶段 GUI 改 debug 构建；
  提高 VM 的 4 核/4GB 配额（宿主 10 核/32GB）。
- **文档同步纪律**：每片合并同步 STATUS.md；架构级变化进 CONTROL_ARCHITECTURE.md；
  一次性决策进 DECISIONS.md。本文只在**路线本身变化**时修订。

## 五、顺序与依赖一览

```
P0.1 ─┐
P0.2 ─┼─→ P1 → P1.5 → P2 ─→ P3 ─→ P4 ─→ P5.1 ─→ P7
      │                │                └→ P5.2 ─↗
      └───────────────（CI-W）──→ P6 ──────────────↗
```

- 主线：P1 → P1.5 → P2 → P3 → P4 → P5 → P7，每片独立 PR、独立验收。
- P6 与 CI-W 依赖外部条件（Windows 环境 / 9月配额），与主线并行，但 **P7 发布
  以 P6 完成为前提**（不发布未在目标平台真机验证过的安装包）。
- 里程碑：**M1 =“单机可用”**（P2 完成：macOS 上 Android 全流程）；
  **M2 =“四端打通”**（P5 完成：双手机端 + 双象限 GUI）；
  **M3 =“三平台可装”**（P6+P7 完成：终态达成）。
