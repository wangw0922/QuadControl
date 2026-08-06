# QuadControl

M0 基础骨架包括 Rust 共享库、跨平台 schema、只读 Android ADB 能力探测器，以及 Apple 用户主动的连接诊断闭环。macOS SwiftPM listener 与共享库可在本机构建测试；iOS 工程可在模拟器构建、测试并完成真实连接，**真机签名与 on-device 验证仍未做**。Rust/ADB 本机缺工具链，由 GitHub Actions 覆盖。

This is not a control product. The Apple diagnostic accepts only non-sensitive
connection heartbeats, never screen, input, clipboard, files, relay, Bonjour,
or unattended access. ReplayKit, Screen Curtain, HID, and all device control
remain Blocked. Android probe only calls `adb version`, `adb devices -l`, and
`adb mdns services`; it does not pair, connect, shell, or modify a device.

See [连接与测试说明](docs/CONNECT_AND_TEST.md).

## 验证

执行 `scripts/verify-all.sh` 可运行当前主机具备工具链的检查。本机（完整 Xcode 26.6 / iOS 26.5 SDK）已通过：`swift test` 4 项 XCTest、`QuadControlSelfTest` 52 项断言、iPhone 17 模拟器上 `xcodebuild test` 2 项，以及模拟器与 Mac listener 之间的真实握手/心跳/断开/token 复用拒绝。Rust 与 ADB 因本机无工具链仍为 Blocked。Windows、Android app、投屏和控制功能尚未实现。
