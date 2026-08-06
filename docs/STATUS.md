# Status

M0 foundation includes the workspace skeleton, source schema, documentation,
platform-boundary notes, passive ADB probe, and an Apple user-initiated
diagnostic listener/shared Swift package.

Verified locally with full Xcode 26.6 (iOS 26.5 SDK):

- `swift build` and `swift test` pass; XCTest executes 4 assertions. The earlier
  Command Line Tools environment could not run XCTest at all.
- `QuadControlSelfTest` passes 52 assertions over real TCP loopback.
- `xcodebuild test` on an iPhone 17 (iOS 26.5) Simulator passes the 2
  `ConnectionModelTests`.
- The shipped `QuadControlMacListener` binary was run end-to-end against the
  Simulator app: handshake, authentication, repeating 5-second heartbeats, user
  disconnect, in-TTL token reuse rejected as `consumed`, and post-TTL reuse
  rejected as `expired`. Listener output contained only short connection IDs —
  no token, address, or payload.

Rust remains **Blocked** locally where Cargo is unavailable; GitHub Actions
covers `cargo metadata`, Clippy, and workspace tests. Android real-device
behavior is **Blocked** without `adb` and a device. iOS **physical-device**
build, signing, and on-device validation remain **Blocked** — Simulator
evidence does not substitute for them.

No production UI, media stream, pairing, remote relay, device control, Android
screen-off, ReplayKit, Screen Curtain, HID, or iOS P0 result is implemented.
