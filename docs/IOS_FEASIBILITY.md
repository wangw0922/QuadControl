# iOS feasibility

The XcodeGen `project.yml` generates a project that builds and tests on an iOS
Simulator with full Xcode. `xcodebuild test` against an iPhone 17 (iOS 26.5)
Simulator passes, and the SwiftUI diagnostic client completes a real
authenticated session against the Mac listener over loopback.

Physical-device validation is **done** for the diagnostic client. An iPhone SE 3
(iPhone14,6, iOS 26.5) was signed with a free personal team, installed via
`devicectl`, and connected to the Mac listener over real Wi-Fi LAN. That covers
what the Simulator cannot: signing and install, LAN routing rather than the
shared loopback stack, the iOS local-network permission prompt, and the camera
used by QR pairing.

Free personal-team signing requires Developer Mode on the device and manual
trust of the developer profile; both are user actions that cannot be scripted.

iOS P0 remains **Blocked** pending real-device evidence. ReplayKit broadcasts
require user action. Screen Curtain (physical screen-off while active) is
distinct from locked state; whether ReplayKit remains useful under Screen
Curtain is unverified. Third-party public APIs do not justify a claim of
control after system lock, and no lock bypass is allowed.
