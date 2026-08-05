# iOS diagnostic source

`project.yml` is an XcodeGen source manifest that references the root Swift
package. It has not generated an `.xcodeproj`, and no Xcode/Simulator/signing/
device build is claimed. The SwiftUI app is manual host/port/token entry only;
it persists no token and has no Bonjour declaration, ReplayKit, screen sharing,
control, clipboard, or files. It accepts only numeric loopback/private/link-local
hosts and disconnects when the app leaves the active scene. Generate and test
it using `docs/CONNECT_AND_TEST.md`; full Xcode and user-owned signing are
required.
