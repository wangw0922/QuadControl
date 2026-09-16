# P4 iPhone 控制面板方案（界面 C；= GUI_PLAN G4）

状态：**已实施**（P4.0 #27 / P4.1 #28 / P4.2 #29，2026-09-14；真机验收未执行）。方案定稿于 2026-09-13，一轮 Fable 对抗性评审，3 阻塞 / 12 应改全部采纳，
裁决见第六节）。范围与验收以 [MASTER_PLAN.md](MASTER_PLAN.md) P4 为准；能力与限制
的事实来源是 [agents/ios-wda/README.md](../agents/ios-wda/README.md)（真机测量）与
[CONTROL_ARCHITECTURE.md](CONTROL_ARCHITECTURE.md)。

## 一、目标与边界

交付：侧栏选中 iPhone 时，主区显示界面 C。

- **Windows / Linux**：GUI 内渲染 WDA 的 MJPEG 画面；点击画面转发点按、拖动画面 = 滑动；文字转发卡；
  「唤醒屏幕」/ 截屏 / 回主屏三按钮；「最近使用的应用」卡（见下）；「已连接 · N 帧/秒」
  标签；「声音保留在手机上」说明卡；断开连接。
- **「最近使用的应用」卡**（P4.3）：iOS 的多任务卡片界面从电脑这侧**不可达**
  （Home 键机型要双击，一次 XCUITest 按键约 0.5 s，凑不出双击窗口；WDA 也没有公开
  的切换器端点），`GET /wda/apps/list` 又只返回**前台**那一个应用。等价物是：GUI 按
  `GET /wda/activeAppInfo` 记下本次会话里见过的前台应用（最新在前，至多 8 个，排除
  SpringBoard），点一下用 `POST /wda/apps/activate` 切回去。应用名来自会话起来后
  异步跑一次的 `ios apps --list`，拿不到就回退成 bundle id 的最后一段。
- **macOS**：不起任何 WDA 链路。只显示引导卡：检测 `/System/Applications/iPhone
  Mirroring.app` 是否存在，「打开」= `open -b com.apple.ScreenContinuity`；不存在时
  显示系统版本要求。**不驱动、不注入、不自动化**该应用。
- 一律**不做**：熄屏控制（XCUITest 机制不可达，UI 不得暗示）、输入密码解锁、任何
  锁屏绕过、后台无人值守。
- **同一时刻最多一个 iOS 会话**（go-ios 隧道信息服务是每主机一份，多隧道行为未
  验证；多设备不在 P4 范围）。

固有限制原样呈现在界面文案里：UI 自动化不是输入注入；需要开发者证书签名的 WDA；
声音不能转发；锁屏时画面为黑、文字不送达；Linux 需 usbmuxd 守护进程在运行。

## 二、进程与数据流（Win/Linux）

```
GUI (Tauri)
 └─ IosSession 监督线程（独占所有子进程）
     ├─ ios tunnel start --userspace --udid U        （iOS 17+ 必需，长驻）
     ├─ ios runwda --udid U --bundleid B --testrunnerbundleid T --xctestconfig X  （长驻）
     ├─ ios forward --udid U <L8100> 8100            （WDA HTTP）
     ├─ ios forward --udid U <L9100> 9100            （WDA MJPEG）
     └─ MJPEG 代理（Rust，127.0.0.1:<Lproxy>，仅本会话）
           上游 127.0.0.1:<L9100>/ → 切帧、单槽最新帧、计帧/计丢帧 → 下游 WebView <img src>
```

- `<L8100> / <L9100> / <Lproxy>` 每会话由 OS 分配（bind 0 取端口）。**例外**：go-ios
  的隧道信息服务用其固定默认端口（实现时以钳制版本的 `ios tunnel start --help`
  为准）；启动前探测该端口，被占用则返回 `tunnel_port_busy`，不静默复用别人的隧道。
- WebView 只连代理端口，从不直连 go-ios 的转发端口。CSP 已允许
  `img-src http://127.0.0.1:*`，不再放宽（`connect-src` 保持 `'self'`）。
- WDA 的 bundle id 必须自定义（个人团队签不了默认值），本片通过环境变量
  `QUADCONTROL_WDA_BUNDLE_ID` / `QUADCONTROL_WDA_TESTRUNNER_ID` /
  `QUADCONTROL_WDA_XCTESTCONFIG` 提供；设置界面归 P7。go-ios 路径沿用 `IOS_BIN`。
- **go-ios 版本钳制**：启动时 `ios version`，低于 **v1.2.1**（唯一实测版本）拒绝
  （`ios_version`），更高版本放行并写警告日志。
- **Windows**：`start_ios_session` 与 Android 同理直接返回 `windows_session_unsupported`
  （无 Job Object 无法收回进程树，P6）。代码仍需 `cfg(windows)` 编译通过。
- **macOS 开发路径**：仅 `cfg(debug_assertions)` **且** 环境变量
  `QUADCONTROL_IOS_DEV_WDA=1` 时允许在 macOS 上走 Launch，用于本机真机验收；
  release 构建不含该分支，macOS 产品路径仍只有引导卡。
- **GUI 崩溃路径**（Linux）：所有 iOS 子进程 `pre_exec` 设
  `prctl(PR_SET_PDEATHSIG, SIGTERM)`，GUI 被 SIGKILL 时子进程随之退出；macOS 开发
  路径不覆盖此项，文档如实标注。
- `--udid` 出现在 argv（`ps` 可见），不违反「不入仓、不持久化」；stderr 尾部含 UDID
  与 bundle id，**iOS 的 `stderr_tail` 永不进 DTO**，只写本进程 stderr。

### 退出序列（三条路径都要无孤儿、端口释放）

`request_stop` / GUI 退出：停代理 → `DELETE /session`（超时 ≤ 1 s，礼貌性）→ 对
tunnel / runwda / forward×2 **并行**下终止梯子（全部 SIGINT，统一等 3 s；剩余全部
SIGTERM，等 2 s；剩余 SIGKILL）→ 全部 reap。最坏墙钟 5 s + 收尾，
`IOS_SHUTDOWN_DEADLINE = 7 s`。进程自亡：监督线程发现任一子进程退出即按同一序列
收回其余，状态置 `Exited`/`Failed`。GUI 退出时 Android 与 iOS 两个 store 先全部
`request_stop`，再共用同一个 `deadline_at` 依次等 done，timed_out 合并上报。
**真机修正（2026-09-15）**：macOS 上 AppleScript `quit` / Cmd+Q 走 NSApp 终止流程，
Tauri **不发** `ExitRequested`，只挂在那里等于没挂（实测 GUI 1 s 内退出、4 个 go-ios
子进程被留下）。清理因此同时挂在最终的 `RunEvent::Exit` 上**同步**执行（实测退出耗
时 3 s、子进程归零）；`ExitRequested`/`CloseRequested` 的后台路径保留给能拦住的入口。

### 为什么要一个 MJPEG 代理

1. 帧率标签需要计帧，`<img>` 对 MJPEG 不暴露逐帧事件。
2. 链路死亡（tunnel/runwda/forward 任一退出、上游 EOF）时代理一关，前端画面立即断。
替代方案已核对：前端 `fetch()` 流式解析要放宽 `connect-src`；Tauri 自定义协议不适合
无限流。

代理行为：accept 循环，**广播给所有下游**（每条连接一个写线程，互不驱逐），上游始终
最多一条；按 multipart 边界切帧，单帧上限 4 MiB（超限断流报 `proxy_failed`）；保留
「最新一帧 + 递增序号」，下游慢时旧帧被覆盖并计 `backpressure_drops`（定义：发布新帧时
上一帧一个写线程都没取走过；无下游不计）；

> 原契约是「最新下游连接获胜」，依据「WebKit 对 MJPEG 会自行重连」。**真机实测
> （iPhone SE 3 + macOS WKWebView，2026-09-15）证伪**：用 curl 连一次代理端口做计数，
> 代理按旧契约关掉了 WebView 那条连接，此后 `<img>` 永远停在最后一帧、**不会重连也不
> 报错**，而转发端口上的帧是新的。故改为广播、不再踢人；GUI 侧另加一个限流到 2 s 的
> `error` 兜底重连（WKWebView 在流中途被断开是否触发 `error` 尚未实测）。代价：读停但
> 不关闭的下游会占住一个写线程直到对端关闭或 `stop()`。
`FrameStats { frames, backpressure_drops, last_frame_at }` 原子共享。不解码 JPEG、
不做黑帧检测。**停帧不是锁屏信号**（真机测量：锁屏后截图是合法全黑 PNG、无错误），
锁定提示只由 `/wda/locked` 驱动。

### WDA 调用清单（全部带超时与输出上限）

| 动作 | 调用 | 备注 |
|---|---|---|
| 就绪探测 | `GET /status` | 起 WDA 后轮询，总预算 60 s |
| 建会话 | `POST /session` `{"capabilities":{"alwaysMatch":{}}}` | 取 sessionId |
| MJPEG 参数 | `POST /session/{s}/appium/settings` `{"settings":{"mjpegServerFramerate":30,"mjpegServerScreenshotQuality":30}}` | 帧率只在编码侧限，与设备刷新率无关。真机 2026-09-15：15/50 ≈ 14 fps、帧约 40 KB；30/30 ≈ 17 fps、帧约 37 KB（取这组）；`mjpegScalingFactor` 50 不提升帧率（瓶颈在设备端截屏），不用 |
| 窗口尺寸 | `GET /session/{s}/window/size` | 随 1 s 状态轮询刷新（旋转会变） |
| 点按 | `POST /session/{s}/wda/tap` `{"x","y"}` | 会话作用域。**不用 W3C `/actions`**：真机 2026-09-15 同一次点按 `actions` 要 1.50 s，`wda/tap` 只要 0.01 s 且效果相同（用户反馈的「反应慢」即此）。**2026-09-16 复测推翻**：0.01 s 是对失效会话的 404 响应，不是真点按；有效会话里 `wda/tap` 为 0.8–1.5 s，所有 WDA 设置都不能缩短（见 agents/ios-wda/README.md 第三轮），当前按约 1 s 计。锁定时不转发 |
| 回主屏 | `POST /session/{s}/wda/pressButton` `{"name":"home"}` | 会话作用域。**不用 `/wda/homescreen`**：真机上前台已是 SpringBoard 时它不按键（停在第二屏就回不到第一页）；顶层 `/wda/pressButton` 回 `unknown command` |
| 滑动 | `POST /session/{s}/wda/dragfromtoforduration` `{"fromX","fromY","toX","toY","duration"}` | 真机已验证；锁定时不转发；时长钳制 0.05–2 s |
| 唤醒屏幕 | `POST /session/{s}/wda/unlock` | 公开 XCUITest 操作（Home + 上滑）；有密码停在密码页，由用户自己解锁，绝不发送密码。**已测量**（2026-09-15）：有密码时阻塞约 8 s 后 500 并停在密码页 → `device_locked`；刚锁不久的窗口期内直接解锁 |
| 锁定状态 | `GET /session/{s}/wda/locked` | 驱动「已锁定」提示、禁用文字卡与点按；唤醒的效果核验 = 前后对比 |
| 前台应用 | `GET /session/{s}/wda/activeAppInfo` | 「最近使用的应用」卡的数据来源。真机 2026-09-15：**0.24 s**，回 `{"value":{"bundleId":..,"pid":..,"name":..}}`（`name` 常为空串）。每 2 轮状态轮询查一次。**不用 `/wda/apps/list`**：真机上它只返回**前台**应用，列不出后台应用 |
| 切换应用 | `POST /session/{s}/wda/apps/activate` `{"bundleId":..}` | 真机 2026-09-15 **有效**（把 Chrome 切到前台）。锁定时不转发；非锁定的失败 → `activate_app_failed` |
| 应用名 | `ios apps --list --udid=U` | 每行 `<bundleId> <name…> <version>`，名字可含空格、版本在最后（真机 9 行样本核对，2026-09-15）。会话起来后异步跑一次，10 s 预算，只读 stdout（stderr 带 UDID，丢弃）。失败就回退成 bundle id 最后一段 |
| 截屏 | `GET /screenshot` | base64 PNG → 用户「图片」目录（无则回退 home）`QuadControl-<时间戳>.png`，回传路径 |
| 文字 | `GET /session/{s}/element/active`（POST 在 WDA 16.12.8 未处理）→ `POST /session/{s}/element/{e}/value` → `GET /session/{s}/element/{e}/attribute/value` 回读 | **禁用 `/wda/keys`**（真机：返回成功但不送达）。回读不含发送内容 → `text_unconfirmed`，文案「无法确认已送达，请在手机上核对」（安全输入框回读为空属正常） |

「符号存在 ≠ 可调用 ≠ 真的产生效果」：结果码来自效果核验，不是 HTTP 200。

**会话会被作废，客户端要自愈**（真机 2026-09-15）：另一个客户端对同一台 WDA
`POST /session` 之后，原 session id 对**任何** session 作用域端点都返回 HTTP 404
`invalid session id`（WDA 同一时刻只保留一个会话；重启 WDA 同理），当时的表现是
`send_text` 在第一步 `locked()` 上就报 `wda_session_failed`。故 `Client` 的 session id
改为共享状态，session 作用域请求撞上「404 + invalid session」时重建会话并**重试一次**
（只一次），重建后重放 MJPEG 的 fps/quality。`send_text` 取到元素 id 之后的写入与回读
不重试——元素 id 属于旧会话，硬重试会对着新会话的陌生元素写。

## 三、代码落点（三个 PR）

### P4.0 `crates/quadcontrol-process`（独立 PR，先行）

把 `configure_process` / `terminate` 梯子 / 有界等待 / `RingBuffer` / `drain_stderr` /
`STDERR_LIMIT` 从 `quadcontrol-android` 搬到新 crate，`quadcontrol-android` re-export。
`terminate` 的信号升级判断改为参数传入（CLI 传原全局函数，GUI 传恒 false），
**不搬全局量**。验收 = Android 既有测试全绿，零行为变化。

### P4.1 `crates/quadcontrol-ios`（lib）

- `resolve_ios()`、`check_version()`；`list_devices()`：P1 写在 GUI 里的
  `parse_ios_devices` / `extract_device_list` 搬进来，GUI 改为依赖；用真机 `ios list`
  输出校准。
- 钳制版本的 `ios tunnel start --help` / `runwda --help` / `forward --help` / `version`
  文本作为 fixture 入仓（`tests/fixtures/go-ios-1.2.1/`）；假 `ios` 二进制的 argv 断言
  对着 fixture 写，语法漂移会变成红测试。
- `wda::Client`：阻塞 HTTP（`ureq`，每请求超时 5 s，响应上限 8 MiB），上表全部方法，
  错误映射到稳定错误码。
- `mjpeg::Proxy`：如上节。
- `session`：`IosLaunchOptions { udid, mode }`，`mode = Launch { ios_bin, wda_ids }`
  | `Attach { http_port, mjpeg_port }`。**Attach 只存在于本库与其测试**，GUI command
  层不接受。`spawn_supervised() -> IosSessionHandle`、`shutdown_all()`，语义与 Android
  一致；并行梯子；Linux `PDEATHSIG`。

### P4.2 GUI

后端新增 command（全部 async + `spawn_blocking`）：`host_platform`、
`start_ios_session`、`stop_ios_session`、`ios_session_status`（state / fps / locked /
proxy_port / window_size）、`ios_tap {x,y}`（内容矩形归一化 0–1）、`ios_send_text`、
`ios_home`、`ios_wake`、`ios_screenshot`、`open_iphone_mirroring` 与
`iphone_mirroring_available`（仅 macOS）。`IosSessionStore` 并入退出清理。

错误码：`ios_not_found`、`ios_version`、`usbmuxd_unavailable`、`tunnel_port_busy`、
`tunnel_failed`、`wda_not_installed`、`wda_signature_expired`（runwda 的对应错误）、
`wda_unreachable`、`wda_session_failed`、`forward_failed`、`proxy_failed`、
`device_locked`、`text_unconfirmed`、`screenshot_failed`、
`ios_session_already_running`、`windows_session_unsupported`；实现时补充：
`ios_process_error`（库的底层 IO 错误）、`ios_session_not_running`、
`macos_uses_iphone_mirroring`（macOS 后端第二道闸）、`iphone_mirroring_failed`、
`unsupported`、`ios_link_lost`（tunnel/runwda/forward 自亡后的前端文案）、
`no_active_element`（真机：无聚焦输入框时 `GET element/active` 回 nosuchelement）、
`activate_app_failed`（P4.3：`apps/activate` 的非锁定失败，例如应用已被卸载）。

前端：`renderSession()` 按 `device.platform` 分流，macOS 渲染引导卡变体；画面容器按
`window/size` 的长宽比定尺（不用固定 246×500 硬拉伸），点击按实际内容矩形归一化；
`img.src` **只在状态迁移时设置/清空**，运行中的每秒重渲染不碰 `src`；`locked` 时禁用
文字卡与点按并显示「手机已锁定，请在手机上解锁后继续」；侧栏 iOS 设备状态：go-ios
列出即 `available`（实现时查 `ios info` 是否有信任字段，有则用于徽章）。双语全部走
`messages`。

## 四、验证与验收

自动化（合并门禁）：
- P4.0：Android 既有测试全绿。
- P4.1 单测：解析（`ios list` JSON / 行式 / 空 / 非法、WDA JSON、版本钳制、
  fixture argv）；代理用合成上游验证计帧、单槽覆盖计 `backpressure_drops`、下游重连
  3 次帧仍持续且上游只连一次、停止后连接关闭、超限断流；session 用假 `ios` 二进制
  + 测试内假 WDA HTTP 服务覆盖：正常起停、runwda 立即失败、`/status` 超时、
  **4 个假子进程全部忽略 SIGINT 时 `request_stop` 到 done 墙钟 < 7 s**、
  `shutdown_all` 超时上报、文字回读不一致 → `text_unconfirmed`、Linux 上 kill -9
  测试进程后假子进程消失（`cfg(target_os = "linux")`）。
- P4.2：DTO 映射、含 letterbox 的坐标换算。
- fmt / clippy `-D warnings` / test；GUI crate 同样三项。

真机（MASTER_PLAN P4 验收，本机 macOS 开发路径执行，需用户配合接 iPhone SE 3）：
1. 安装 go-ios ≥ 1.2.1；Team ID 命令行传入重签 WDA 并安装；
2. Launch 模式跑 GUI：**MJPEG 帧率实测记录**（≥10 fps 是假设，README 只有 2.4 fps
   截图上限；达不到时阈值是产品决策，不算实现失败）；点按 / 文字 / 唤醒 / 截屏 /
   回主屏全部产生可见效果；锁屏后「已锁定」提示出现，并记录代理 `frames` 是否继续
   增长，回写 README；
3. 断开 20 轮：监督线程记录的 PID `kill -0` 得 `ESRCH`、`pgrep -x ios` 为空、
   `lsof -i :<每个端口含隧道信息端口>` 为空、手机上 WDA 已退出；
4. macOS 引导卡调起系统 iPhone 镜像。

**执行记录（2026-09-14/15，iPhone SE 3 + iOS 26.5 + WDA 16.12.8 + go-ios 1.2.1，
macOS 宿主 debug 路径）**：

| 项 | 结果 |
|---|---|
| 启动 | 隧道注册判据修复后一次成功；点「开始控制」到画面出现约 10–15 s |
| 画面 | 代理 5 s 59 帧（≈11.8 fps），标签 12–15 帧/秒；广播修复后手机屏幕变化实时跟随（Chrome 打开、键盘弹出、锁屏黑屏均即时反映） |
| 点按 | GUI 点画面 → 手机 Google 搜索框聚焦、键盘弹出 |
| 文字 | GUI 发送 `quadcontrol 42` → 手机搜索框出现该文本，界面「已发送」 |
| 截屏 | `~/Pictures/QuadControl-<时间戳>.png`，750×1334 |
| 回主屏 | 前台应用回到 springboard |
| 锁定 | 锁屏后 1 s 内出现「手机已锁定」提示，文字卡禁用；「唤醒屏幕」在无需密码的窗口期内直接解锁，有密码时直连实测停在密码页并报 `device_locked` |
| 会话自愈 | 第三方 `POST /session` 作废 GUI 会话后，GUI 下一次调用自动重建，无可见故障 |
| 断开 | 实测 2 轮：断开后 go-ios 进程与监听端口在 7 s 内归零；20 轮未完成 |

真机暴露并已修的缺陷见 [agents/ios-wda/README.md](../agents/ios-wda/README.md) 第二轮
测量表与 P4.3 提交说明。**未闭环**：macOS 上用 SIGTERM 结束 GUI 会留下 go-ios 子进程
（无 PDEATHSIG，方案已标注）；WDA 帧率标签在锁屏时仍计数（黑帧），属预期。
真机项若无法执行，STATUS.md 如实标「未真机验收」，不阻塞合并；**M2 宣称时注明
「WDA 链路在 macOS 宿主 debug 路径验证，Linux 未跑」**。

## 五、风险与未决

- `ios tunnel start` 若实测仍需提权，界面明示，不静默提权。
- WDA 由用户自备签名版本，本仓库不携带 WDA 源码或二进制。
- 帧率标签是代理计数，不是设备刷新率；GUI 无任何刷新率控件（红线）。
- **并行梯子的已知边界**（P4.1 复审 G1 残留）：`terminate_all` 每级对整个进程组
  发信号，但完成判据是「所有直接子进程可收尸」。若组长全部已退出而组内仍有辅助
  进程，梯子在第一级就返回、不再升级。改成等「进程组为空」是另一个语义，留待真机
  验收第 3 条（`pgrep -x ios` 为空）暴露后再定。
- **MJPEG 上游的三条假设未测量**：accept 后不发请求 WDA 就推响应头、边界写法、每段
  带 Content-Length。代理能吃下带/不带 `--` 的边界与无 Content-Length 的段，但上游
  读头阶段没有超时；真机验收时校准。
- GUI 的会话槽位在预检（`ios version` + 端口探测，几百毫秒）期间是 `Starting`
  占位：此时「断开连接」只记日志不生效，退出应用也收不到尚未诞生的句柄。堵这个
  窗口需要给库的 `spawn_supervised` 加可取消预检，留待真机验收后视需要再做。
- macOS 产品路径的引导卡只有在 go-ios 把设备列出来时才可达；未装 go-ios 的 Mac
  用户看不到「打开 iPhone 镜像」。P7 决定是否让引导卡不依赖设备发现。

## 六、评审裁决记录（2026-09-13）

阻塞 B1 串行梯子超预算 → 并行梯子；B2 生产路径无平台可验 → macOS debug + 环境变量
开关；B3 `backpressure_drops` 恒零 → 单槽缓冲。应改 S1–S12 全部采纳（停帧≠锁屏、
PDEATHSIG、隧道固定端口与单会话、Attach 不进 GUI、img.src 只在迁移时设、内容矩形
坐标、「唤醒屏幕」命名与效果核验、锁定禁用输入、可证伪的无孤儿验收、版本钳制与
fixture、P4.0 拆 PR 且升级判断参数化、`begin_shutdown` 并行）。建议 G1–G7 采纳。
