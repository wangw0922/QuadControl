# Status

M0. No screen mirroring or device control is implemented in any quadrant.

Control directions are fixed — see [control architecture](CONTROL_ARCHITECTURE.md):
Windows/macOS→Android is self-built, Windows→iPhone goes through go-ios and
WebDriverAgent, and macOS→iPhone uses Apple's iPhone Mirroring rather than
anything of ours.

## Done

- Rust workspace, cross-platform schema, repository skeleton, core documents.
- Passive Android ADB capability probe, 27 tests, **verified against a real
  Samsung handset** in both `unauthorized` and `device` states. Real output
  carries fields the hand-written fixtures lacked, including a `device:` field
  that collides with the `device` state keyword; regression tests now pin it.
- `scripts/verify-all.sh` is **fully green for the first time** — every Rust and
  Apple check passes locally with nothing BLOCKED or SKIPPED. The `cargo fmt`
  gate, dropped from CI in August when the repository did not satisfy stable
  rustfmt, has been restored.
- An Apple connection-diagnostic loop, verified end to end on a physical
  iPhone SE 3 over Wi-Fi LAN, including QR pairing. **This is no longer on the
  product path** — it was built to validate a self-built Mac↔iPhone link, which
  the architecture change removed. It is kept as a diagnostic tool.
- `QuadControlMacHIDProbe`, a read-only Bluetooth capability probe. The HID
  direction it was built for has since been dropped.
- Simplified Chinese and English localization for every UI string shipped so far.

## Not started

- Windows control client.
- macOS control client.
- Android target: screen capture, input injection, screen-off control.
- Windows→iPhone: go-ios integration and desktop GUI. **The go-ios capability
  list has not been verified on hardware by this project** — the first task is to
  confirm screenshot and tap on a real device rather than restate its docs.
- macOS→iPhone: detection and guidance for Apple's iPhone Mirroring.

## Dropped

Bluetooth HID, ReplayKit broadcast extension, and Screen Curtain. Removed for
loss of purpose, not because they were disproven — see
[macOS HID feasibility](MACOS_HID_FEASIBILITY.md) for the honest state of what
was and was not established.

## Screen-off support is asymmetric

Windows/macOS→Android: reachable. macOS→iPhone: expected to be covered by
Apple's solution, unverified by us. Windows→iPhone: **not reachable**, decided
by how XCUITest works.
