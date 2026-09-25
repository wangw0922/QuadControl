# P4.4 方案：macOS→iPhone 画面引擎换成 QuickTime USB 屏幕流（提案，评审后修订）

状态：**提案，一轮 Fable 对抗性评审后修订；门 1 已通过**（2026-09-16，用户已确认按此方案做；数据见 [agents/ios-wda/README.md](../agents/ios-wda/README.md)「Gate 1」行：上游 58 fps，渲染侧完全运动的秒里 50–56 变化帧/秒，JPEG 编码 8 ms/帧）。范围只含
**macOS 宿主 + iPhone**；Windows/Linux 的同类替换（需要 libusb 驱动与替代 usbmuxd）
不在本片，留待 P6 之后另立方案。事实来源：
[agents/ios-wda/README.md](../agents/ios-wda/README.md)「Third measurement pass」。

## 一、要解决的问题与证据

用户在 macOS 上用 QuadControl 的 iPhone 面板（P4，WDA MJPEG 截屏流）反馈「卡」，
目标是接近 Apple「iPhone 镜像」的流畅度。真机（iPhone SE 3，iOS 26.5，macOS 26.5）
测量把「卡」拆成两半：

| 环节 | 现状（WDA MJPEG） | QuickTime USB 流（经 Apple CoreMediaIO 实测） |
|---|---|---|
| 帧率 | 14–17 fps（设备端截屏是瓶颈） | **50.6 fps** 平均，运动窗口 53.9 fps，原生 750×1334，100 s 零丢帧 |
| 帧间隔 | 约 60–70 ms（由帧率推算） | 中位 **16.9 ms**，p99 53 ms |
| 回主屏→画面变化到达 | 0.33 s | ≤ 0.367 s（持平；两者都被 XCTest 按键耗时占满） |
| 点按 / 滑动（控制侧） | `wda/tap` 0.8–1.5 s，拖动 1.7–3.1 s | **不变**，控制仍走 WDA |

结论：换画面引擎能把帧率提高约 3 倍、去掉丢帧；**不能**缩短每次手势约 1 s 的控制
延迟，那是 XCUITest 事件合成的固有开销（`waitForIdleTimeout`、`animationCoolOffTimeout`、
`defaultActiveApplication`、`snapshotMaxDepth`、`wda/touchAndHold` 全部试过，无效；
2026-09-15 记录的 0.01 s 是对失效会话的 404 响应，不是真点按）。这一点必须原样告诉
用户：画面会丝滑，点一下仍要等约一秒。

测量的一个瑕疵要记录：50.6 fps 那次 100 s 采集之前，同一台手机被
`quicktime_video_hack activate` 加过第六个 USB 配置，去激活后该配置仍在描述符里
（重新插拔才消失）。Apple 自己的采集也会切到这个配置，污染可能性低，但按工作纪律
第 3 条，第五节的门 1 必须在**重新插拔后**的设备上重跑一次，两组数字都记。

## 二、产品判断（需要用户确认的两条）

[CONTROL_ARCHITECTURE.md](CONTROL_ARCHITECTURE.md) 目前写的是「Mac→iPhone 不自研，
只引导到 iPhone 镜像」，论据有三：自研会**更差**、**更脆弱**、**无法上架**。本提案把
macOS 的 iPhone 面板从「调试专用路径」升为可用路径，是对该判断的**修订**，改写架构
文档时三条论据要逐一回应，不能只驳一条：

- 「无法上架 / 私有 API」——**不成立**。走的是公开 API：CoreMediaIO 的
  `kCMIOHardwarePropertyAllowScreenCaptureDevices` + AVFoundation 采集设备，就是
  QuickTime Player「新建影片录制→选 iPhone」用的那条路；不越狱、不绕锁屏。红线不变。
- 「更差」——**在控制上仍成立**：镜像的控制是毫秒级，我们是约 1 s。画面上不再成立
  （50 fps 原生分辨率）。
- 「更脆弱」——**仍成立**：需要用户自签 WDA（免费团队 7 天过期）、WDA runner 有间歇
  自亡（第六节）、iOS 26 会要求输入设备密码验证 XCTest。
- 值得做的理由：走 USB、不要求同一 Apple 账户、不要求 Wi-Fi/蓝牙、不要求手机锁定，
  任何「信任此电脑」的手机都能接（多台手机切换只测过一台，见第五节）。
- **引导卡保留**：界面 C 在 macOS 上提供两条路——「iPhone 镜像（Apple，控制最快）」
  与「QuadControl 直连（USB，50 fps 画面，点按约 1 s）」，文案如实标注差异。

**要用户回答的两条**：(1) 接受上述修订及其代价；(2) 「卡」指的是画面帧率还是点按
反应——本片只改善前者，后者是机制底。

## 三、架构

```
GUI (Tauri, macOS)
 └─ IosSession 监督线程
     ├─ [新] quadcontrol-ioscapture（Swift 可执行，apple/ 包）              ← 先启动
     │      AVCaptureSession(iOS 屏幕设备) → JPEG 编码 → MJPEG 服务 127.0.0.1:<Lcap>
     ├─ [等首帧，再等 ios list 重新看到 UDID]
     ├─ ios tunnel start --userspace ...
     ├─ ios runwda / ios forward 8100          （采集模式下不再起 forward 9100）
     └─ MJPEG 代理（现有）：上游改指向 <Lcap>；下游 WebView <img> 不变
```

- **引擎归属**：采集端是我们自己的 Swift 可执行（约 200 行，AVFoundation 公开 API），
  不引入 `quicktime_video_hack`（它在 macOS 上被 `AppleUSBHostiOSDevice` 内核驱动挡住，
  libusb 拿不到接口，见 README）。编码、传输、协议全是 Apple 的，我们只做采集设备的
  打开与转发，与 scrcpy 封装同级，不算自研引擎。
- **为什么先用 MJPEG 而不是原生图层**：复用 P4 全部链路（代理、`<img>`、帧率标签、
  点按坐标映射、断流处理），GUI 侧只换上游。代价是每帧一次 JPEG 编码（**未测**，门 1
  同时量）和 WKWebView `<img>` 在 50 fps 下的渲染上限（**未测**，门 1 就是为它设的）。
  门 1 过不了的升级路径：先用一个独立的约 50 行 `AVCaptureVideoPreviewLayer` 小窗口
  证明原生图层本身达标（门 2），再碰 Tauri 透明 WebView + 坐标映射；不直接在 GUI 里做。
- **启停顺序是硬约束**（真机：采集启动/停止都会重枚举 USB，杀掉活着的 userspace 隧道
  与 WDA runner）：
  - 启动：采集进程 → 等到 MJPEG 首帧 → 轮询 `ios list` 直到再次看到 UDID（记录这个
    间隔，首帧到达不等于总线稳定）→ `ios tunnel start` → `runwda` → `forward 8100` →
    `/status` 就绪 → 建会话。
  - 停止：`DELETE /session` → 采集进程与 tunnel/runwda/forward **同一批**进现有
    `terminate_all` 并行梯子（SIGINT 3 s → SIGTERM 2 s → SIGKILL）。不串行：两轮梯子
    最坏 10 s，装不进 `IOS_SHUTDOWN_DEADLINE = 7 s`，而且停采集本来就会杀掉隧道。
    采集进程对 SIGINT 与 SIGTERM 都当干净退出。
  - 采集进程中途退出 = 隧道必死 → 整个会话按现有路径收回：≤ 1 s 进 `Stopping`，
    ≤ 7 s 进 `Failed`（`supervise()` 先走梯子再置态，5 s 内显示失败做不到）。
  - **后续项（本片不做）**：采集存活时单独重启 tunnel/WDA。README 的规则只禁止在隧道
    存活期间启停采集，不禁止反向；WDA 间歇自亡时这能保住画面。验收里量一次
    「WDA 自亡→整会话恢复」的往返时间作为基线。
- **GUI 被 SIGKILL 时的孤儿**：macOS 无 PDEATHSIG；孤儿采集进程会独占设备、状态栏停在
  演示值，下一次会话起不来。采集进程的 stdin 改为管道（现有 `spawn_child` 是
  `Stdio::null()`，需要新 spawn 变体），进程读到 stdin EOF 即退出；这也覆盖 Cmd+Q 不发
  `ExitRequested` 的路径。
- **helper ↔ 代理的上游契约**（全部来自 `mjpeg.rs` 现状）：先 bind 再向 stdout 打印
  一行端口并**显式 flush**（管道下 stdout 全缓冲）；代理在 `UPSTREAM_CONNECT_BUDGET`
  内连上并立刻发 `GET / HTTP/1.1`，收到 GET 前不推流；每个 part 必带 `Content-Length`
  （否则代理逐字节扫边界）；不用 chunked；单帧 ≤ `MAX_FRAME_BYTES` 4 MiB；整个会话只接
  一条上游连接并保持到结束。stdout 需要有界读取（现有 `spawn_child` 把 stdout 设为
  null）。
- **权限**：采集设备走相机 TCC。真机只有两个数据点：Terminal.app 起的探针弹窗；代理
  shell 起的进程被静默拒绝、没有弹窗。「由 app 拉起的子进程权限归 QuadControl.app」是
  断言，**未测**（未签名的 SwiftPM 产物可能落进静默拒绝路径）。因此 helper 启动先查
  `AVCaptureDevice.authorizationStatus` / `requestAccess`，用独立退出码区分
  `capture_permission_denied` / `capture_device_not_found` / `capture_device_busy`；
  监督线程的「等首帧」是独立状态「等待相机权限」，**不套固定预算**——用户点「允许」
  的时间不可预测。产品包需要 `NSCameraUsageDescription`，文案说明是「读取 iPhone 屏幕」。
- **macOS 开发门保留**。本片仍走 `cfg(debug_assertions) && QUADCONTROL_IOS_DEV_WDA=1`。
  去掉门等于把两项 P7 内容拉进来：release app 从 Finder 启动没有 shell 环境，
  `wda_ids_from_env` 会回落到个人团队签不了的默认 bundle id，必失败；go-ios 把含私钥的
  `selfIdentity.plist` 写进 cwd，release 的 cwd 是 `/`。「持久化 WDA 三个 id + 受控 cwd」
  归 P7，之后再开产品路径。
- **日志红线**：helper 不得输出 `AVCaptureDevice.localizedName`（= 手机名）与 `uniqueID`；
  UDID 只在 argv。
- **点按视觉反馈**：50 fps 画面 + 1 s 点按会比 17 fps 更像「没点上」。界面 C 在点按
  发出时立即画一个短暂标记，本片顺手加。
- 单会话不变；QuickTime Player / iPhone 镜像同时抢采集设备的行为未测，列入验收。

## 四、改动落点（按门的顺序）

| 顺序 | 位置 | 改动 |
|---|---|---|
| 0 | `agents/ios-wda/tools/`（或 `apple/`） | **先把计量探针入仓**：README 里 50.6 fps 的 AVFoundation 探针目前没有在仓库里，数字不可复现 |
| 1（门 1，零 Rust 改动） | 手工 | 探针加最小 MJPEG 服务 → 手工起 tunnel/runwda/forward 8100 → GUI 以现有 `LaunchMode::Attach { http_port, mjpeg_port }` 指向探针端口 → 按第五节量渲染侧帧率 |
| 2 | `apple/`（SwiftPM） | 探针长成 `quadcontrol-ioscapture`：`--port 0` 打印端口行、`--fps`/`--quality`、权限退出码、stdin EOF 退出、SIGINT/SIGTERM 干净退出；`swift test` 覆盖 MJPEG 封装与端口行解析 |
| 3 | `crates/quadcontrol-ios` | `IosLaunchOptions.mode` 新增 `LaunchWithCapture { capture_bin, .. }`；新 spawn 变体（stdin 管道、stdout 有界读）；按第三节顺序启停；采集进 `terminate_all` 同批；不起 `forward 9100`；WDA 的 `appium/settings` 不再下发 MJPEG 参数；「等待相机权限」状态 |
| 4 | `crates/quadcontrol-gui` | macOS 界面 C 两条路的卡片；点按即时标记；帧率标签沿用；`Info.plist` 加 `NSCameraUsageDescription` |
| 5 | 文档 | `CONTROL_ARCHITECTURE.md` macOS 一格按第二节逐条改写；`IOS_PANEL_PLAN.md` 数据流图更新；`STATUS.md` / `MASTER_PLAN.md` 加 P4.4 |

不改：WDA 调用清单、点按/滑动/回主屏/文字/最近应用；Windows/Linux 行为（仍是 WDA MJPEG）。

## 五、验收（真机 iPhone SE 3，macOS 26.5）

1. **门 1，先于全部实现，且在手机重新插拔后**（**已通过，2026-09-16**：渲染侧在完全运动的秒里 50–56，见 README；运动源须用主屏拖动，秒表页只送 30 fps）：探针 MJPEG → 现有代理 → `<img>`。
   帧率**在渲染侧取数**：页面里 rAF + canvas `drawImage` 逐帧像素比对，计「变化帧/秒」，
   持续 60 s **≥ 45 fps**；代理的 `frames` 标签只作旁证（它计的是上游切出的帧，WebKit
   再慢也会把 socket 读空，反映不了绘制）。同时记 JPEG 编码每帧耗时。达不到就走门 2
   （独立原生图层小窗口），重新过评审。
2. 回主屏 send→画面变化 ≤ 0.4 s（与 README 数字持平）；滑动跟手（主观），帧率不掉。
3. 启停 20 轮：无孤儿进程（`pgrep` 归零）、每轮隧道都在采集首帧与 `ios list` 恢复之后
   建立（记录间隔）、退出用时 ≤ 7 s。
4. 采集进程被 `kill -9`：≤ 1 s 进「停止中」，≤ 7 s 显示会话失败且其余子进程收回。
5. GUI 被 `kill -9`：采集进程随 stdin EOF 退出，状态栏演示值消失。
6. 相机权限：`npx tauri dev` 从 Terminal 起、以及 `.app` 从 Finder 双击起，各记一次
   弹窗归属与拒绝后的文案；QuickTime Player 同时打开时的行为记录在案。
7. 设备列表在重枚举期间是否短暂丢失设备、前端是否因此拆卡片或断会话——记录并修正。
8. 状态栏演示值、声音不转发、点按约 1 s 三条限制在界面文案中可见。
9. 「WDA 自亡→整会话恢复」往返时间量一次，作为后续项基线。
10. 采集内容（截图/流）验证后删除；UDID、手机名不进日志与文档。
11. 多台手机：`AVCaptureDevice.uniqueID` 与 UDID 的对应关系测一次，决定 helper 如何选设备。

## 六、风险与未决

- WKWebView `<img>` 50 fps 渲染上限与 JPEG 编码开销都未测（门 1）。
- WDA runner 曾两次在启动后约 1 分钟内「lost connection to testmanagerd」自亡
  （2026-09-16），空闲 196 s 与一串 11 次调用都复现不了；记为间歇性，本片按现有
  `Failed` 收回，不根治。
- iOS 26 在 WDA 长时间未用后重新启动时会要求输入设备密码（「验证以访问 XCTest」），
  由用户在手机上输入；GUI 在「启动中」状态提示这一点。
- 控制延迟约 1 s 是机制底，本片不承诺改善；要毫秒级控制，答案仍是 Apple 的 iPhone 镜像。

## 七、评审裁决（2026-09-16，Fable 子代理 finder，17 条）

阻塞 4 条全部采纳：门 1 改在渲染侧取数（#1）；门 1 用现有 `Attach` 模式零 Rust 改动
先跑、探针先入仓（#2）；停止改为同批并行梯子、第一级是 SIGINT（#3）；保留 macOS 开发门，
产品路径的两项前提归 P7（#4）。应改 10 条采纳 9 条：数字修正（#5）、插拔后重跑（#6）、
权限退出码与无预算等待（#7）、`ios list` 恢复门（#8）、stdin EOF（#9）、上游契约（#10）、
失败时序（#11）、局部重连列为后续项（#12）、不起 forward 9100（#13）；#14 的「三条论据
逐一回应」采纳，「卡指画面还是点按」转为第二节向用户提问。建议 3 条采纳：点按标记
（#15）、日志红线（#16）、门 2 独立小窗口（#17）。裁决：修改后可实施；实施前仍需用户
对第二节两问表态。
