# QuadControl

从 Windows、macOS 和 Linux 控制 Android 与 iPhone。控制方向按被控端分别采用
不同引擎，权威依据见 [控制架构决策](docs/CONTROL_ARCHITECTURE.md)。

| 控制端 | 被控端 | 引擎 | 本项目负责 |
|---|---|---|---|
| Windows / macOS / Linux | Android | scrcpy（外部引擎） | 封装：发现、配对、启动、清理、统一 GUI |
| Windows / Linux | iPhone | go-ios + WebDriverAgent | 客户端与桌面 GUI，亮屏，UI 自动化级别 |
| macOS | iPhone | Apple iPhone Mirroring | **不自研**，仅检测与引导 |

两处"不自研"的理由一致：官方或社区已有维护良好的实现（Apple 的 iPhone
Mirroring、Apache-2.0 的 scrcpy），自研只会得到更差且更难维护的替代品。
scrcpy 的采纳证据链（含本项目真机上的对照测量）见架构文档。

## 当前状态

详见 [Status](docs/STATUS.md)。已在真机验证：macOS→Android 经 scrcpy 4.1
低延迟镜像与控制；自建官方命令回退路径（镜像+输入+旋转跟随+Wi-Fi）；
iPhone 侧 go-ios/WDA 控制链路（iPhone SE 3）。未开始：三平台统一控制端
GUI、Windows/Linux 上的引擎验证。

蓝牙 HID、ReplayKit 广播扩展、Screen Curtain 三项已移出范围。理由是失去用途而非
被证伪：Mac→iPhone 由 Apple 方案覆盖、Windows/Linux→iPhone 由 WDA 覆盖之后，HID 不再
服务于任何象限。它的实测状态如实记录在
[macOS HID 可行性](docs/MACOS_HID_FEASIBILITY.md)——本地 API 可用，手机侧未验证。

界面支持简体中文与英文，跟随系统语言。

## 安全边界

除 Android scrcpy 这一已明确约束的例外外，不使用私有 API；不越狱、不 Root、
不绕过锁屏、不隐藏控制、不默认无人值守。
所有连接必须由用户主动发起、可见、可断开。

Android 探测器只执行 `adb version`、`adb devices -l`、`adb mdns services` 三条
只读命令，不配对、不连接、不执行 shell、不修改设备。

## 验证

`scripts/verify-all.sh` 运行当前主机具备工具链的检查。本机（完整 Xcode 26.6 /
iOS 26.5 SDK）已通过 Swift 构建、45 项 XCTest、62 项自检断言，以及 iPhone SE 3
真机经 Wi-Fi 局域网的连接验证。Rust 与 ADB 因本机缺工具链由 GitHub Actions 覆盖。

## scrcpy 封装

Rust 封装位于 [`crates/quadcontrol-android`](crates/quadcontrol-android)。它只
使用 `adb devices -l` 做设备选择，`--adb`、`ADB` 环境变量、PATH 的优先级固定为
参数 > 环境变量 > PATH，并把最终路径以 `ADB` 传给 scrcpy。封装只接受稳定版
scrcpy 4.1 或更新版本，预发布版本一律拒绝。

封装拥有 `--serial`、窗口标题、视频码率（默认 8M）和熄屏参数。透传参数会语义
解析，既检查 `--option=value`，也检查空格分隔形式和短参数组合；`--` 之后才是
不解析的原样参数。以下参数会被拒绝：`-s/--serial`、`-d/--select-usb`、
`-e/--select-tcpip`、`--window-title`、`-b/--video-bit-rate` 会与封装配置冲突；
`--no-cleanup` 会破坏清理契约，`--tcpip` 会改变连接状态；`-w/--stay-awake`、
`-t/--show-touches`、`--screen-off-timeout`、`--power-off-on-close` 会改变设备
设置；`--kill-adb-on-close` 会影响其他 adb 用户；`--no-window`、`-r/--record`、
`-N/--no-playback`、`--no-video-playback`、`--no-video`、`--otg` 会绕过可见镜像
边界。完整解析规则见命令的 `--help`。

实际使用示例：开发时可通过 Cargo 启动并指定设备和熄屏：
`cargo run -p quadcontrol-android --bin quadcontrol-scrcpy -- --serial <S> --screen-off`。
安装后的二进制使用相同参数：`quadcontrol-scrcpy --serial <S> --screen-off`。

清理契约分层如下：正常退出和库级 `stop` 由 scrcpy 自身清理设备端 server 与
forward；Unix stop 使用 SIGINT→3 秒→SIGTERM→2 秒→SIGKILL，Windows 有 console
使用 Ctrl-Break 后再降级到 TerminateProcess。强杀时设备端依靠 server socket 断开
自杀，但本机 adb forward/reverse 映射可能遗留，不作保证。Windows 无 console 的
路径直接 TerminateProcess，不保证子进程树完全清理（本阶段不引入 Job Object）。
wrapper 崩溃或断电同样不作保证；`/data/local/tmp/scrcpy-server.jar` 无害且下次
启动会覆盖。
