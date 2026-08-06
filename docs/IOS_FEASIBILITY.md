# iOS feasibility

The XcodeGen `project.yml` generates a project that builds and tests on an iOS
Simulator with full Xcode. `xcodebuild test` against an iPhone 17 (iOS 26.5)
Simulator passes, and the SwiftUI diagnostic client completes a real
authenticated session against the Mac listener over loopback.

iOS **physical-device** work is still **Blocked**: signing with a real Team,
device provisioning, Developer Mode, and on-device validation have not been
performed here. Simulator results are not device evidence — the Simulator shares
the Mac's network stack, so it does not exercise Wi-Fi LAN routing or the iOS
local-network permission prompt that a real iPhone shows on first connect.

iOS P0 remains **Blocked** pending real-device evidence. ReplayKit broadcasts
require user action. Screen Curtain (physical screen-off while active) is
distinct from locked state; whether ReplayKit remains useful under Screen
Curtain is unverified. Third-party public APIs do not justify a claim of
control after system lock, and no lock bypass is allowed.
