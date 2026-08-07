# Windows control client boundary

Future entry point: WinUI 3 desktop host calling Rust FFI. Prerequisites:
Windows 11, .NET SDK, Windows App SDK. Intended command: `dotnet build`.
No project or UI exists yet, and no Windows host has been verified.

This client owns **two** quadrants, and they are not equally mature.

## → Android (self-built)

ADB transport, screen capture, input injection, screen-off control, on the shared
Rust crates. Shares essentially all of its logic with the macOS client.

## → iPhone (go-ios + WebDriverAgent)

Drives `agents/ios-wda`. Unlike the Android path this is UI automation, so the
UI must not promise scrcpy-like responsiveness, and must state plainly that:

- there is no screen-off control,
- setup needs Developer Mode, UI Automation, and a signed WebDriverAgent,
- a free signing team expires after seven days,
- this quadrant cannot ship through a consumer store.

macOS has no equivalent obligation because Apple's iPhone Mirroring covers its
iPhone quadrant. Windows has no such option, which is why this path exists.
