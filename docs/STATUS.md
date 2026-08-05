# Status

M0 foundation includes the workspace skeleton, source schema, documentation,
platform-boundary notes, passive ADB probe, and an Apple user-initiated
diagnostic listener/shared Swift package. `swift build --disable-sandbox` and
the real TCP loopback `QuadControlSelfTest` pass locally with 51 assertions.
The listener's non-interactive token-display refusal also passes. Rust remains
**Blocked** where Cargo is unavailable. iOS source, tests, Info.plist, and an
XcodeGen manifest exist, while iOS build, Simulator, signing, and real-device
validation remain **Blocked** because full Xcode is absent. No production UI,
media stream, pairing, remote relay, device control, Android screen-off,
ReplayKit, Screen Curtain, HID, or iOS P0 result is implemented.
