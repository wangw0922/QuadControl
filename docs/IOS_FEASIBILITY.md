# iOS feasibility

The project no longer ships an iOS application. Neither iPhone quadrant needs
one — see [control architecture](CONTROL_ARCHITECTURE.md).

## macOS → iPhone

Apple's iPhone Mirroring covers this officially. Verified on the development
host (macOS 26.5.1, Apple M1 Pro): the app exists at
`/System/Applications/iPhone Mirroring.app`, its bundle identifier is
`com.apple.ScreenContinuity`, and its `LSMinimumSystemVersion` is `26.5`. It
exposes **no URL scheme and no public API**, so a third party can detect and
launch it and nothing more.

Apple's stated prerequisites — same Apple account, devices nearby, Wi-Fi and
Bluetooth on, iPhone locked — have **not been verified by this project**. If the
"iPhone stays locked with its screen off" prerequisite holds, this quadrant gets
screen-off control for free, which no self-built approach achieved.

## Windows → iPhone

go-ios plus WebDriverAgent. Public frameworks only, no jailbreak, and every step
requires explicit user authorization, so it stays inside the project's rules.

Honest limits, all of which must reach the user before they install anything:

- It is UI automation, not input injection. Ordinary taps, typing, and settings
  work; games, continuous drags, multi-touch, and high frame rates will not match
  scrcpy. Mechanism, not tuning.
- Configuration is heavy: trust the computer, Developer Mode, UI Automation,
  a WDA signed with a valid certificate, a device tunnel on iOS 17+, and a tunnel
  driver on Windows.
- Signing is the real barrier. A free personal team expires in seven days.
- It cannot be distributed through the App Store. This quadrant is a
  developer/technical-user capability, not a consumer product.
- **No screen-off control.** XCUITest cannot drive the UI or capture the screen
  while the display is off or locked.

The capability list above comes from the go-ios project's own description and has
**not been verified on hardware by this project**. The first task in this
quadrant is to confirm screenshot and tap on a real iPhone and record measured
results, the same way the ADB and Bluetooth probes were handled.

## What is closed

Lock-screen control by a third party remains impossible with public APIs and is
not a goal. ReplayKit, Screen Curtain, and Bluetooth HID were dropped when the
quadrants that would have used them were covered by other means; see
[macOS HID feasibility](MACOS_HID_FEASIBILITY.md) for what was actually
established before that work stopped.
