# Status

Updated 2026-08-08. Control directions are fixed — see
[control architecture](CONTROL_ARCHITECTURE.md): every desktop
(Windows/macOS/Linux)→Android rides **scrcpy** as the engine;
Windows/Linux→iPhone goes through go-ios and WebDriverAgent; macOS→iPhone uses
Apple's iPhone Mirroring rather than anything of ours.

## Working today (verified on hardware)

- **macOS→Android via scrcpy 4.1**: full mirror and control at native
  resolution over Wi-Fi, latency the user describes as "gone" — on the same
  Samsung SM-S9180 whose vendor-modified internal APIs originally pushed us
  away from a scrcpy-style self-build. Adopting the engine instead of
  rebuilding it is the recorded decision.
- **macOS→Android fallback path (self-built, official adb commands only)**:
  live H.264 mirror (AnnexB→VideoToolbox), mouse/keyboard input, rotation
  follow, Wi-Fi pairing, orphan-encoder cleanup, link-measured auto bitrate.
  Measured floor: 0.56 s trigger→bytes, of which ≥0.26 s is inside the device
  before bytes leave it — the reason the fallback stays a fallback. Kept
  working, no further investment.
- **iPhone control plumbing (validated via macOS host)**: go-ios userspace
  tunnel, WDA signed and installed on an iPhone SE 3, tap / text / home /
  screenshot verified on hardware. This is the same stack Windows and Linux
  will drive.
- **Apple connection-diagnostic loop** (off the product path, kept as a
  diagnostic tool): end-to-end verified on hardware, including QR pairing.
- Rust workspace with cross-platform schema; passive Android ADB capability
  probe (27 tests, verified against the real handset); `scripts/verify-all.sh`
  fully green; Simplified Chinese + English localization for all shipped UI.

## Not started

- **Unified controller GUI** on each desktop (Windows, macOS, Linux): device
  discovery, pairing, engine launch/config/cleanup, iPhone-side integration.
  Today macOS has a working self-built viewer plus raw scrcpy; nothing is
  unified yet, and Windows/Linux have no client at all.
- **scrcpy on Windows/Linux**: expected to work (officially supported), not
  yet verified by this project.
- **go-ios on Windows/Linux**: same status — cross-platform by design,
  verified here only via macOS.
- macOS→iPhone: detection and guidance for Apple's iPhone Mirroring.
- Wrapping scrcpy (launch flags, exit cleanup, version pinning) inside
  QuadControl instead of asking the user to run it by hand.

## Dropped

Bluetooth HID, ReplayKit broadcast extension, Screen Curtain, the self-built
Mac↔iPhone protocol, and the self-built Android internal-API capture layer
(superseded by adopting scrcpy). Reasons and honest evidence state per item in
[control architecture](CONTROL_ARCHITECTURE.md) and
[macOS HID feasibility](MACOS_HID_FEASIBILITY.md).

## Screen-off support is asymmetric

All desktops→Android: reachable via scrcpy `--turn-screen-off` — verified on
macOS 2026-08-08 (panel off, mirror and control unaffected). macOS→iPhone: expected covered by Apple's solution,
unverified by us. Windows/Linux→iPhone: **not reachable**, decided by how
XCUITest works.
