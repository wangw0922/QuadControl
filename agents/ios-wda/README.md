# Windows → iPhone agent boundary

This quadrant drives [go-ios](https://github.com/danielpaulus/go-ios) and
WebDriverAgent as external processes, the same way `agents/android-shell` drives
`adb`: a whitelisted, bounded subprocess with parsing separated from execution so
it can be tested without a device.

Nothing is implemented yet.

## First task is verification, not implementation

go-ios advertises screenshot streaming, tap/swipe/longpress, text input, and file
transfer. **This project has not confirmed any of it on hardware.** The first
task here is to run `go-ios` against a real iPhone, confirm screenshot and tap,
and record measured results — not to restate the project's own capability list.

The Bluetooth HID work is the cautionary precedent: a capability list said the
peripheral-role APIs existed, they did exist, and the direction still went
nowhere useful. Symbol presence and a working product are different claims.

## Known limits, to be surfaced to users before install

- UI automation, not input injection. Games, continuous drags, multi-touch, and
  high frame rates will not match scrcpy.
- Setup requires trusting the computer, Developer Mode, UI Automation, a signed
  WebDriverAgent, a device tunnel on iOS 17+, and a tunnel driver on Windows.
- A free personal signing team expires after seven days.
- Cannot be distributed through the App Store.
- No screen-off control — XCUITest cannot drive a locked or dark display.

## Rules this agent inherits

Public frameworks only, no jailbreak, no lock-screen bypass, no unattended
default. Every subprocess needs a timeout and an output bound. Device serial
numbers and addresses are not persisted.
