# QuadControl

从 Windows 和 macOS 控制 Android 与 iPhone。控制方向按被控端分别采用不同方案，
权威依据见 [控制架构决策](docs/CONTROL_ARCHITECTURE.md)。

| 控制端 | 被控端 | 方案 | 本项目负责 |
|---|---|---|---|
| Windows | Android | 自研（ADB） | 全部，含熄屏后继续控制 |
| macOS | Android | 自研（ADB） | 全部，含熄屏后继续控制 |
| Windows | iPhone | go-ios + WebDriverAgent | 客户端与桌面 GUI，亮屏，UI 自动化级别 |
| macOS | iPhone | Apple iPhone Mirroring | **不自研**，仅检测与引导 |

Mac→iPhone 不自研，是因为 Apple 已经提供官方实现，自研只会得到更差、更脆弱且
无法上架的替代品。

## 当前状态

M0 阶段。尚未实现任何投屏或控制能力。已完成的是：Rust workspace 与跨平台
schema、只读 Android ADB 能力探测器、以及一套经真机验证的 Apple 连接诊断闭环
（现已不在产品路径上，保留为诊断工具）。

蓝牙 HID、ReplayKit 广播扩展、Screen Curtain 三项已移出范围。理由是失去用途而非
被证伪：Mac→iPhone 由 Apple 方案覆盖、Windows→iPhone 由 WDA 覆盖之后，HID 不再
服务于任何象限。它的实测状态如实记录在
[macOS HID 可行性](docs/MACOS_HID_FEASIBILITY.md)——本地 API 可用，手机侧未验证。

界面支持简体中文与英文，跟随系统语言。

## 安全边界

不使用私有 API、不越狱、不 Root、不绕过锁屏、不隐藏控制、不默认无人值守。
所有连接必须由用户主动发起、可见、可断开。

Android 探测器只执行 `adb version`、`adb devices -l`、`adb mdns services` 三条
只读命令，不配对、不连接、不执行 shell、不修改设备。

## 验证

`scripts/verify-all.sh` 运行当前主机具备工具链的检查。本机（完整 Xcode 26.6 /
iOS 26.5 SDK）已通过 Swift 构建、19 项 XCTest、62 项自检断言，以及 iPhone SE 3
真机经 Wi-Fi 局域网的连接验证。Rust 与 ADB 因本机缺工具链由 GitHub Actions 覆盖。
