# QuadControl

M0 基础骨架包括 Rust 共享库、跨平台 schema、只读 Android ADB 能力探测器，以及 Apple 用户主动的连接诊断闭环。macOS SwiftPM listener/shared libraries can be built and tested locally; iOS source and its XcodeGen manifest are present, but iOS build, Simulator, signing, and physical-device evidence are **Blocked** here.

This is not a control product. The Apple diagnostic accepts only non-sensitive
connection heartbeats, never screen, input, clipboard, files, relay, Bonjour,
or unattended access. ReplayKit, Screen Curtain, HID, and all device control
remain Blocked. Android probe only calls `adb version`, `adb devices -l`, and
`adb mdns services`; it does not pair, connect, shell, or modify a device.

See [连接与测试说明](docs/CONNECT_AND_TEST.md).

## 验证

执行 `scripts/verify-all.sh` 可运行当前主机具备工具链的检查。macOS Swift build 与 51 项 self-test 断言已在本机通过；Rust/ADB 与 iOS 真机因缺少工具链仍为 Blocked。Windows、Android app、投屏和控制功能尚未实现。
