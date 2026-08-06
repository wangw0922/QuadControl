# Status

M0 foundation includes the workspace skeleton, source schema, documentation,
platform-boundary notes, passive ADB probe, and an Apple user-initiated
diagnostic listener/shared Swift package.

Verified locally with full Xcode 26.6 (iOS 26.5 SDK):

- `swift build` and `swift test` pass; XCTest executes 4 assertions.
- `QuadControlSelfTest` passes 62 assertions over real TCP loopback.
- `xcodebuild test` on an iPhone 17 (iOS 26.5) Simulator passes 9
  `ConnectionModelTests`.
- Simulator end-to-end: handshake, authentication, 5-second heartbeats, user
  disconnect, in-TTL token reuse rejected as `consumed`, post-TTL reuse rejected
  as `expired`.
- **Physical iPhone SE 3 (iPhone14,6, iOS 26.5)**: signed with a free personal
  team, installed, and connected over real Wi-Fi LAN — not loopback. The iOS
  local-network permission prompt was accepted and the session authenticated and
  heartbeated. Verified twice: once with a typed token, once by scanning the
  listener's QR pairing code.

Listener output contains only short connection IDs — no token, address, or
payload.

UI is localized for Simplified Chinese and English: the iOS app and the macOS
listener's one-time-token block follow the system language, while stderr log
lines stay English because docs and tests match on them.

`QuadControlMacHIDProbe` covers the M0 "macOS Bluetooth HID" item as far as
read-only observation allows: the controller supports the HID profile and all
three peripheral-role APIs exist, but whether macOS will let a process act as a
HID *peripheral* is reported `unknown` and stays **Blocked** pending an active,
user-present experiment. See [macOS HID feasibility](MACOS_HID_FEASIBILITY.md).

Rust remains **Blocked** locally where Cargo is unavailable; GitHub Actions
covers `cargo metadata`, Clippy, and workspace tests. Android real-device
behavior is **Blocked** without `adb` and a device.

No production UI, media stream, pairing, remote relay, device control, Android
screen-off, ReplayKit, Screen Curtain, HID, or iOS P0 result is implemented.
