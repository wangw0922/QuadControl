# QuadControl

M0 基础骨架包括 Rust 共享库、跨平台 schema、只读 Android ADB 能力探测器，以及 Apple 用户主动的连接诊断闭环。macOS SwiftPM listener 与共享库可在本机构建测试；iOS 已在模拟器与 **iPhone SE 3 真机**上完成构建、测试和真实 Wi-Fi 局域网连接验证，支持扫码配对。界面支持简体中文与英文，跟随系统语言。Rust/ADB 本机缺工具链，由 GitHub Actions 覆盖。

This is not a control product. The Apple diagnostic accepts only non-sensitive
connection heartbeats, never screen, input, clipboard, files, relay, Bonjour,
or unattended access. ReplayKit, Screen Curtain, HID, and all device control
remain Blocked. Android probe only calls `adb version`, `adb devices -l`, and
`adb mdns services`; it does not pair, connect, shell, or modify a device.

See [连接与测试说明](docs/CONNECT_AND_TEST.md).

## 验证

执行 `scripts/verify-all.sh` 可运行当前主机具备工具链的检查。本机（完整 Xcode 26.6 / iOS 26.5 SDK）已通过：`swift test` 4 项 XCTest、`QuadControlSelfTest` 62 项断言、iPhone 17 模拟器上 `xcodebuild test` 9 项，模拟器与 Mac listener 之间的真实握手/心跳/断开/token 复用拒绝，以及 iPhone SE 3 真机经 Wi-Fi 局域网的手输与扫码两种连接方式。Rust 与 ADB 因本机无工具链仍为 Blocked。Windows、Android app、投屏和控制功能尚未实现。
