# Status

The end-to-end roadmap from the current state to the finished product — system
boundaries and per-step acceptance criteria — is [MASTER_PLAN.md](MASTER_PLAN.md).

Updated 2026-09-13. Control directions are fixed — see
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
- **QuadControl scrcpy wrapper** (`crates/quadcontrol-android`, lib + CLI
  `quadcontrol-scrcpy`): adb unification via `ADB`, stable ≥4.1 pinning with
  prerelease rejection, semantic passthrough filtering, signal-ladder /
  Ctrl-Break lifecycle with layered cleanup contract. Smoke-tested on the real
  handset over Wi-Fi, including double-signal escalation and no-orphan checks.
  Windows/Linux real-device execution is not verified; macOS is the
  development host.

## GUI: M1 "usable on one machine" reached in code, not yet on hardware

- **Unified controller GUI** (Tauri v2, `crates/quadcontrol-gui`): G0 closed on
  2026-08-10 (shell builds on all three platforms in CI; Linux runtime spike
  passed on Ubuntu 22.04 arm64 — Chinese text renders, MJPEG held 14.55 fps
  for 5 minutes with no leak; Slint fallback not triggered). Merged on
  2026-09-13 (PR #24):
  - **P1 device list on real data** — `adb devices -l` plus go-ios `ios list`,
    status badges, 5-second polling, stable error codes translated in the UI.
    The go-ios output shape is taken from its documentation, not verified on
    hardware (no go-ios on the development host); P4 re-checks it.
  - **P1.5 session supervision library** — `try_wait`, `SessionHandle`,
    `spawn_supervised`, `shutdown_all` with a 7-second bounded budget; covered
    by unit tests (normal stop, natural exit, idempotent stop, no orphan).
  - **P2 one-click Android session** — start/stop, status card (running /
    exit code / stderr tail on non-zero exit), sound and screen-off options,
    exit interception with background cleanup. **Verified only with a fake
    adb and fake scrcpy on the Linux VM**; the 20-round real-handset
    acceptance in MASTER_PLAN P2 has not been run. Windows deliberately
    refuses to start sessions until Job Object cleanup lands (P6).
  - **P3 pairing wizard on real adb** — native Wireless-debugging QR payload,
    backend-driven 300-second countdown, cancel token wired to every exit
    path, manual pairing on the same state machine, pairing secret never
    derives `Debug`/`Clone`. **Not yet verified end-to-end on a real handset**
    through the GUI (the underlying `pair-qr` CLI path was).
- Still missing on the desktop side: the iPhone panel (P4), Windows/Linux
  real-device engine verification (P6), and packaging (P7). See
  [MASTER_PLAN.md](MASTER_PLAN.md).

## Not started

- **scrcpy on Windows/Linux**: expected to work (officially supported), not
  yet verified by this project.
- **go-ios on Windows/Linux**: same status — cross-platform by design,
  verified here only via macOS.
- macOS→iPhone: detection and guidance for Apple's iPhone Mirroring.

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
