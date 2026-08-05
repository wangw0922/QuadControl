# iOS feasibility

iOS P0 is **Blocked** pending real-device evidence. The repository contains a
SwiftUI diagnostic source and XcodeGen `project.yml`, not a generated project
or verified app. Build, Simulator, signing, and physical-device validation are
all Blocked until full Xcode and user-authorized hardware are available.
ReplayKit broadcasts require user action. Screen Curtain (physical screen-off
while active) is distinct from locked state; whether ReplayKit remains useful
under Screen Curtain is unverified. Third-party public APIs do not justify a
claim of control after system lock, and no lock bypass is allowed.
