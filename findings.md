# QuadControl Findings

## 2026-08-04 初始检查

- 项目目录最初只有 `.DS_Store` 和 `QuadControl_四端通用控制软件制作计划.md`，不是 Git 仓库。
- 现有计划已明确技术栈、仓库结构、M0-M6 顺序、安全边界和首个任务，不需要重新发明产品范围。
- 本轮应只完成计划“首个任务”：Monorepo、Rust workspace、核心文档、四端最小工程说明、Android ADB 能力探测 CLI 与单元测试。
- iPhone 全局控制、Screen Curtain 下 ReplayKit、Windows/macOS 软件 HID 都必须经过真机 P0；本轮只能标记实验或未验证。
- Android ADB 探测可通过 `adb version`、`adb devices -l` 和 `adb mdns services` 获取本轮需要的可观察信息；解析逻辑应与命令执行解耦，以便无需真机做单元测试。
- 独立评审确认：`_adb-tls-pairing._tcp` 仅表示配对服务被广播，`_adb-tls-connect._tcp` 仅表示安全连接服务可发现，网络设备条目仅表示连接状态；只读探测不能可靠证明 `paired_with_this_host`。
- ADB probe 必须保持被动：参数数组调用三个白名单命令，设置超时与输出上限，不执行配对、连接、shell 或 server 管理命令。
- 本机缺少 Rust、ADB、.NET、Gradle 和完整 Xcode；Docker 可用但没有缓存 Rust 镜像。主验收需优先尝试容器化 Rust 验证，原生四端构建必须明确 SKIP。
- 用户指定的 `AGENTS.md` 原文仍含 WLM/Walmart 风险示例，以及主协调器 `x high` 与规划 `high` 的不一致；本轮按原文保留，不静默校正。
- 主协调器完整读取实现后发现并修正：mDNS connect 服务广播不能作为 `wireless_connected`；该字段现在只由 `adb devices -l` 中 `device` 状态的 TCP/mDNS transport 推导。
- USB transport 只在 `adb devices -l` 含显式 `usb:` 元数据时报告；无法可靠识别的非网络序列号报告 `unknown`，避免把所有非 TCP 设备误报为 USB。
- JSON 序列化已改为稳定小写字段并完整转义控制字符，避免 `DeviceState::Other` 的 Debug 文本生成无效 JSON。
- 容器镜像、容器 apt 和临时 rustup 三条验证路径均受当前网络速度/停滞阻断；不能声称 Rust 编译、Clippy 或单元测试通过。
- 最终独立复核确认原唯一 major 已由真实 `SystemRunner`/CLI 集成测试代码闭合；当前共 24 项测试。唯一剩余阻塞是这些测试与 Rust 验证命令尚未实际运行。

## 2026-08-04 重启后继续

- 用户显式启用了 Build macOS Apps 与 Build iOS Apps 插件，并要求继续实现以及给出手机连接/测试方法。
- `swiftpm-macos` 要求无 Xcode 工程时优先用 SwiftPM，按 `swift build`、`swift run`、`swift test` 验证，且不得把纯 SwiftPM 可执行文件描述为完整 `.app` bundle。
- `swiftui-ui-patterns` 要求新 SwiftUI 工程先读取 `references/app-wiring.md`，保持状态所有权清晰、视图小而聚焦，并在继续前实际构建验证。
- `ios-debugger-agent` 要求先发现已启动的 Simulator，再设置工程、scheme 与 simulator 默认值；如果没有 Booted Simulator，不得自行启动，需把该步骤交给用户。
- 本轮可验证目标收敛为 macOS/iOS 的“用户可见连接诊断闭环”，不将 ReplayKit 画面、Screen Curtain、HID 或全局输入标为已实现。
- 当前 Swift 6.3.2/arm64 macOS 26 CLI 可用；完整 Xcode、`simctl`、ADB、Rust、XcodeGen、Tuist 均不可用。
- XcodeBuildMCP 默认工程、scheme、simulator 全为空；`list_sims` 因缺少 `simctl` 失败，因此本轮不能运行 iOS Simulator 或真机签名流程。
- XcodeBuildMCP 当前会话只暴露 Simulator 工作流，没有 project scaffolding、macOS 或 physical-device 工具；工程生成需依赖仓库内源码/清单，不能声称插件已完成真机构建。
- Swift CLI 已实际导入 `Foundation`、`Network`、`CryptoKit`、`SwiftUI` 成功；可在 Command Line Tools 环境构建和测试 macOS SwiftPM 连接核心及 SwiftUI 可执行目标。
- 独立 Apple 方案评审结论为 `NEEDS-CHANGES`：诊断连接方向可接受，但 `.proto` 需先定义唯一 wire contract，认证必须补齐双向 nonce/proof、HKDF session key、原子消费与带 MAC/序号的心跳。
- 评审要求默认 loopback、LAN 显式选择 Wi-Fi/有线、private/link-local peer 校验，以及 32-byte token/180 秒 TTL/10 秒握手/4 KiB frame/8 pending/1 active/5 秒心跳/15 秒超时等固定边界。
- token 不可进入参数、URL、Bonjour、env、持久存储或日志；终端显示与 iOS SecureField 仍存在肩窥/截图/内存不可物理清零的残余风险，必须如实披露。
- 实现会话初稿可用 `swift build --disable-sandbox` 编译，但 listener 未实现 heartbeat 定时/15 秒超时、pending/active/限速，Mac 未输出可连接端口，client 认证后也不发送 heartbeat；文档相应宣称不成立，必须由主协调器修正。
- 初稿 token 用 stdout `print` 输出，可能被重定向；LAN peer 字符串前缀判断也可误接收伪装 hostname，均违反评审合同。
- 当前 CLT 不含 XCTest 或 Swift Testing；`swift test` 只构建空测试模块并退出 0，不能视为测试通过。需要实际可执行 self-test，并把 XCTest 保留给完整 Xcode。
- SwiftPM 在当前受管沙盒里需 `--disable-sandbox` 才能解析 manifest；根 package 无第三方依赖/插件，可用该参数做本轮本地验证，但用户普通环境仍应优先默认 `swift build/test`。
- Swift 6 对可执行 target 的顶层常量会按模块可见性检查 private 类型，且非 throwing `@autoclosure` 不能直接包裹 `try`；self-test 必须先求值可抛操作再传入断言。
- 真实 loopback self-test 已证明 listener 可分配端口、双向认证并完成首个 MAC heartbeat/ack；CLI 本身在当前 PTY 工具中无 controlling `/dev/tty`，按安全合同拒绝显示 token，而不是回退到 stdout。
- iOS manifest 仍需补齐可安装 Info.plist 基础键和明确 scheme；连接教程/构建脚本也需从概要扩展为可复制执行的 XcodeGen、签名、同网段与预期日志步骤。
- 原 iOS client 允许任意非空 host，理论上可主动连接公网 listener；共享 `DiagnosticAddressPolicy` 后，client 和 server 使用同一数值 IP 解析逻辑，域名字符串前缀无法伪装成私网地址。
- Android 当前没有可连接的 app/transport；用户可测试的只有 USB 授权与被动 ADB probe。连接文档必须明确这一点，不能把 `adb devices` 描述成 QuadControl 会话。
- 终审前自查发现初版 Swift Codable 合成键（如 `connectionID`）与 schema 的 snake_case 不一致；现在使用显式 CodingKeys 和 snake_case type raw values，`.proto` 再次成为可核对的唯一逻辑 contract。
- 原 heartbeat transcript 把 JSON payload bytes 当作字段，和文档“不签 JSON”矛盾；现在签经过验证的固定语义 status（`alive`/`ok`）及 session/sequence。
- `String(describing: connection.endpoint)` 包含源端口，不能作为 per-peer 限速 key；共享地址策略现只返回解析后的数值 host，使重连换端口不能绕过 peer bucket。
- Apple 官方本地网络隐私说明确认：iOS 首次发起本地 TCP 会弹出用户授权，`NSLocalNetworkUsageDescription` 必须存在；`NWConnection` 可能先进入 waiting。本实现保持 10 秒 deadline、允许 waiting 后继续，并在文档中要求授权过慢时用新 token 重试。
- 独立终审最终确认 Apple M0 源码与收紧后的合同一致；剩余 iOS/Rust/ADB Blocked 都是当前主机工具链/真机证据边界，不是已知源码一致性缺陷。

## 2026-08-06 完整 Xcode 环境下的运行验证

- 主机环境已变化：完整 Xcode 26.6（iOS 26.5 SDK）+ `simctl` + 多台模拟器可用；`cargo`、`adb`、`xcodegen` 仍缺（xcodegen 本轮已装）。此前所有"因只有 Command Line Tools"导致的 Blocked 前提不再成立。
- `swift test` 首次真正执行 XCTest：4 项通过。此前 CLT 环境只能构建空测试模块并退出 0，从未运行过断言。
- **发现并修复一个此前从未被触发的生产缺陷**：`DiagnosticServer.start()` 在 loopback 模式下同时把显式端口写进 `parameters.requiredLocalEndpoint` 和 `NWListener(using:on:)`，`NWListener` 初始化直接抛 `POSIXErrorCode(22)` EINVAL。
- 影响范围：`QuadControlMacListener` 默认就是 loopback + 47100，且参数解析拒绝 port 0，所以**发布出来的 listener 二进制在默认调用下从来没能启动过**，只会打印 `listener failed: startup`。LAN 模式不设 `requiredLocalEndpoint`，不受影响。
- 51 项 self-test 全程没抓到，是因为 `LoopbackProbe` 用 `port: 0` → `.any`，两处端口都是 `.any` 时不冲突。教训：测试必须覆盖**发布二进制实际使用的参数**，而不是测试方便的参数。
- 用四组对照实测确认修复方向：`requiredLocalEndpoint(显式) + on:(显式)` = EINVAL；`requiredLocalEndpoint(显式)` 单独用可绑定但需 `newConnectionHandler` 才会 ready；`requiredLocalEndpoint(.any) + on:(显式)` 可绑定且实测 `lsof` 显示只监听 `127.0.0.1:47100`。采用最后一种。
- 新增回归断言 `explicit loopback port binds and is reported`，已验证它在还原修复后确实 FAIL、修复后 PASS，self-test 从 51 增至 52 项。
- 预留端口的辅助函数最初 flaky：`NWListener.cancel()` 是异步的，必须等到 `.cancelled` 状态才能重新绑定同一端口，否则第二个 listener 抢不到。
- 模拟器与 Mac 共用网络栈，因此 `127.0.0.1` 即可完成真实端到端验证，不需要真机或同网段 Wi-Fi。
- 实测行为闭环：handshake → authenticated → heartbeat 每 5 秒递增 → 用户 Disconnect → TTL 内复用同一 token 被拒为 `consumed` → 超过 180 秒复用被拒为 `expired`。listener 输出只含 8 位随机连接 ID，无 token、地址或消息内容，与安全合同一致。
- **文档与实际行为不符**：服务端拒绝握手时直接关连接，不发 `DiagnosticError` 帧，所以 iPhone 对所有服务端拒绝都只显示 `Connection failed (network)`。原 `CONNECT_AND_TEST.md` 声称用户会看到 `authentication`/`expired`/`consumed`，实际看不到。已改为如实描述，并把"不向未认证对端透露拒绝原因"记为显式决策而非缺陷。
- 自动化测试需要 listener 有真正的 controlling terminal 才能拿到 token（`/dev/tty` 守卫），用 `pty.fork()` 实现；这只用于本地测试脚本，没有改动生产代码的 token 显示约束。
- `apple/Sources/QuadControlDiagnosticClient/` 是重构后遗留的空目录，`ARCHITECTURE.md` 仍按旧名描述；已删除目录并改为实际的 `QuadControlDiagnosticTransport` / `QuadControlDiagnosticServer`。

## 2026-08-06 真机、简体中文与扫码配对

- 真机前置有两项只能由用户完成，不可脚本化：iPhone 开启 Developer Mode（`devicectl` 报 `developerModeStatus: disabled`），以及在 Xcode 登录 Apple ID 生成签名证书。登录属于凭据输入，助手不得代做。
- Xcode 登录后 keychain 仍为 0 个证书；证书是首次自动签名构建时按需生成的。Team ID 可从 `defaults read com.apple.dt.Xcode` 的 `IDEProvisioningTeamByIdentifier` 读到（本机 `TEAMIDREDACT`，免费个人 Team）。
- 免费个人 Team 装机后首次启动必须在手机上手动信任开发者描述文件，否则 `devicectl` 报 profile not trusted；设备锁屏时报的是另一个错（device not unlocked），两者要分清。
- **真机验证补上了模拟器证明不了的部分**：真实 Wi-Fi 局域网路由（非共享 loopback 栈）、iOS 本地网络权限弹窗、真机签名安装、相机。
- LAN 模式的 listener 绑定的是 `*:47100` 通配地址，不像 loopback 模式绑定到具体地址。防线在 accept 时的 `isPrivateOrLinkLocal` peer 校验和 `requiredInterfaceType`，即 bind 之后而非 bind 之时。符合代码合同，但比「`--interface wifi` 就只在 Wi-Fi 上监听」的直觉宽。
- 用户要求把 token 改成固定 `123456` 以便输入。已拒绝并说明：LAN 模式下同网段任何设备可连，固定值等于没有认证。
- 进一步分析否掉了「随机 6 位 PIN」这个看似合理的折中：在线猜测确实被限速挡住（每 peer 5 次/分、全局 20 次/分，token 仅活 180 秒 → 最多约 60 次尝试对 100 万组合无威胁），**但 client proof 是 `HMAC(token, transcript)` 且 transcript 中的 nonce/ID 明文传输，被动抓一次握手即可离线秒破 6 位数字**。限速对离线攻击完全无效。这是选择扫码而非缩短 token 的决定性理由。
- 按项目规则「所有跨平台消息必须先写入 .proto」，配对载荷先加进 `session.proto` 再实现 Swift adapter。
- 配对载荷刻意不用 URL 形式：URL 会诱导系统去「打开」它，且容易进入各类日志。改用 JSON，512 字节上限，扫描内容按不可信输入处理——地址仍走私网校验，token 仍走同一解码路径。已加测试覆盖公网地址、域名、任意 URL 文本和超长输入四种恶意二维码。
- 终端二维码用 CoreImage（macOS 自带，不引入第三方依赖）生成，用半块字符把两行模块压进一个字符格以保持近似正方形，并显式设置黑白 ANSI 背景色——终端主题不确定，深色主题会让二维码反色而 iOS 扫不出来。
- 为了让二维码带上具体地址，新增了接口地址解析（`NWPathMonitor` 拿接口名 + `getifaddrs` 取 IPv4），顺带修掉一个 UX 缺陷：LAN 模式原本只能显示「所选接口的私网地址」，用户还得自己去查 `ipconfig`。
- **SwiftPM 的 `.process()` 会把 `zh-Hans.lproj` 小写成 `zh-hans.lproj`**，Foundation 匹配不上 `zh-Hans-US`，所有字符串静默回退英文。改用逐个 `.copy("Resources/zh-Hans.lproj")` 保留大小写。
- **CLI 进程没有 app bundle，CFBundle 的 `Bundle.module` 字符串查找不走用户语言**，直接落到 development region。`Bundle.preferredLocalizations(from:forPreferences:)` 显式传入 `Locale.preferredLanguages` 才正确匹配；listener 因此自行解析最佳 `.lproj`。
- iOS 的 status 原本是硬编码英文 String，测试直接断言英文文案。改成 `ConnectionStatus` 枚举后文案与断言解耦，并加了「每个 case 都有非空且不等于原始 key 的译文」这条断言，漏翻会被测试抓到。
- pty 读到的日志行尾带 `\r`，直接 `grep` 取 token 会多一个字符导致长度 44、粘贴校验失败；自动化取值必须先 `tr -d '\r'`。

## 2026-08-06 macOS 蓝牙 HID 能力检测

- 关键区分：macOS 默认是 HID **host**（接受键盘鼠标），而从 Mac 控制 iPhone 需要的是 HID **peripheral**（把 Mac 伪装成键盘）。这是同一 profile 的相反两端，支持其一不能推出支持另一。控制器 `Supported services` 里的 `HID` 只证明射频支持该 profile。
- 外设角色需要三件事：发布带 HID service class 的 SDP 记录、接受 PSM 0x11（control）与 0x13（interrupt）的入站 L2CAP、用完撤销记录。
- 实测这三个 API 在 macOS 26.5 SDK 中都存在且**没有弃用标记**：`publishedServiceRecordWithDictionary:`、`removeServiceRecord`、`registerForChannelOpenNotifications:selector:withPSM:direction:`。
- 但符号存在远不足以下结论。read-only 测不出来的有：未签名/ad-hoc 签名进程是否被允许发布 HID SDP 记录、macOS 是否会把 HID PSM 交给第三方进程、iPhone 是否愿意配对、配对能否熬过睡眠与重连。因此 `can_act_as_hid_peripheral` 恒为 `unknown`，并加测试守护防止以后有人把它改成基于 API 存在性的 `yes`。
- 探测器加载 IOBluetooth 用 `dlopen` 而非链接框架，只做运行时 introspection，不调用任何 API，因此不触发 TCC 权限提示、不改动蓝牙状态。
- 框架未加载时 API 可用性报 `unknown` 而非 `no`——运行时答不上来不等于 API 不存在。这与 ADB probe 拒绝把「测不到」写成布尔值是同一条原则。
- `system_profiler SPBluetoothDataType -json` 的结构：`SPBluetoothDataType[0].controller_properties`，开关量用 `attrib_on`/`attrib_off`，支持服务是 `0x392039 < HFP AVRCP A2DP HID ... >` 这种带尖括号的字符串，需要单独解析。

## 2026-08-06 HID 外设角色主动实验（第一部分）

- 发布 SDP 记录会经 `IOBluetoothCoreBluetoothCoordinator`，被蓝牙 TCC 门控，且**不给错误码而是直接 SIGABRT 杀进程**（exit 134）。四组对照：纯 CLI 无 usage description → 杀；CLI 把 `NSBluetoothAlwaysUsageDescription` 嵌进 `__TEXT,__info_plist` 段 → 仍杀（该段不被采纳）；`.app` bundle 但直接 exec 内部二进制 → 仍杀；`.app` bundle 经 `open`/LaunchServices 启动 → 放行。
- 崩溃原因字符串始终是"Info.plist 必须含 NSBluetoothAlwaysUsageDescription"，但第二、三组里这个 key 明明存在 —— **该提示具有误导性，真正起决定作用的是有没有经 LaunchServices 启动**。因此任何需要蓝牙的组件必须打成 app bundle，不能沿用本仓库其余部分的 SwiftPM CLI 形态。
- `CBManager.authorization` 在实例化 `CBCentralManager` 之前恒为 `notDetermined`；实例化才触发权限弹窗。
- **一次重要的自我纠错**：首轮所有记录都返回 `nil`，看起来像策略拦截，实际是我的字典格式错了。对照 Apple 自己的 `OBEXOPPSDPRecord.plist` 才发现：UUID 是裸大端 `Data`（`Data([0x11,0x24])`）而非 `DataElementType: 3` 包装；`"0100 - ServiceName*"` 是纯 `String` 而非 data-element 字典；另有 `LocalAttributes` 块。`nil` 不带任何错误信息，格式错和被拒绝长得一模一样——遇到 nil 先拿 Apple 的 plist 对格式。
- 格式改对后：HID service class `0x1124` 发布成功（handle `0x4f491124`），SerialPort `0x1101` 对照组同样成功，两者 `remove()` 均返回 0。
- `registerForChannelOpenNotifications:` 对 HID 的两个 PSM（`0x0011` control、`0x0013` interrupt）**均注册成功**。
- 结论：macOS 并未对第三方进程保留 HID service class 或 HID PSM。本地 API 这一半的答案是「可以」。
- 仍未证明的是涉及手机的那一半，其中 **Class of Device 最可疑**：iPhone 按 CoD 过滤可配对配件，而 macOS 把自己报成 Computer，公共 API 中未找到修改入口。若改不了，SDP 记录再完整也可能不会被 iPhone 列为键盘。
- 实验后状态已还原：所有记录已 remove，蓝牙仍为 `State: On` / `Discoverable: Off`，与实验前一致。
