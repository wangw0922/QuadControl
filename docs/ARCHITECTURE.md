# Architecture

Control directions and their mechanisms are defined in
[control architecture](CONTROL_ARCHITECTURE.md). This file describes the code.

Shared logic lives in the Rust workspace under `crates/`: `core`, `protocol`,
`transport`, `crypto`, `media`, `pairing`, `diagnostics`, `ffi`. Both desktop
control clients build on it so that Windows and macOS do not each grow their own
copy of session state, framing, or input encoding.

Per-quadrant boundaries:

- **→ Android** is self-built and is where the shared crates carry the most
  weight: ADB transport, screen capture, input injection, screen-off control.
  `agents/android-shell` holds the read-only ADB probe and, later, the device-side
  helper.
- **Windows → iPhone** drives `go-ios` and WebDriverAgent as external processes,
  the same way the ADB probe drives `adb`: a whitelisted, bounded subprocess with
  parsing kept separate from execution so it can be tested without hardware.
  WDA speaks its own HTTP protocol, so our `protocol` crate does not apply.
- **macOS → iPhone** contains no control code at all. The only permitted actions
  are detecting `/System/Applications/iPhone Mirroring.app`, checking the system
  version, and launching it with `open -b com.apple.ScreenContinuity`. There is
  no public API, and driving its UI would breach the no-private-API rule.

`apple/Sources/QuadControlDiagnostic*`, `QuadControlMacListener`, and `apps/ios`
implement a self-built Mac↔iPhone diagnostic link. That link is **not on the
product path** after the architecture change. The code is retained because it is
the only component in the repository with real-device end-to-end verification,
but it must not be presented as a product capability.
