# Product constraints

All control and capture require visible, user-initiated authorization. Hidden control, secret recording, lock-screen bypass, root, jailbreak, and default unattended access are prohibited.

## Android internal APIs — an explicit, narrow exception

Outside the explicitly bounded Android scrcpy exception below, no private or
internal APIs are used anywhere in the product.

**Android Mode B — the ADB-pushed shell server — uses Android internal APIs
through reflection.** Screen capture (`SurfaceControl`), input injection
(`InputManager.injectInputEvent`), and physical screen-off
(`setDisplayPowerMode`) have no public equivalent. This is inherent to the
approach the plan chose, and it is how scrcpy works. There is no version of Mode
B that avoids it.

What keeps this inside the product's ethics, and what each point does *not* claim:

- The user enables developer options and USB debugging themselves, and approves
  this specific computer on the device. Nothing is silent.
- It runs at shell UID. Not root, no jailbreak, no exploit, no privilege
  escalation — only what ADB already grants.
- The server is pushed for the session and removed afterwards. It does not
  persist or auto-start.
- **It is never distributed through an app store.** Store distribution is Mode A
  (MediaProjection plus AccessibilityService), which uses public APIs only and
  correspondingly cannot turn the physical screen off.

The honest cost: internal APIs are unstable across Android versions and OEM
builds. Mode B will break on devices we have not tested, and capability must be
probed on each device rather than assumed. Treat every internal API as
present-until-proven-absent on that specific handset.

Rules that remain absolute even in Mode B: no lock-screen bypass, no capture
without the user starting the session, no unattended default, and the device's
original brightness, rotation, and timeout are restored when the session ends.
**The device's display refresh rate is never modified** — an explicit user
rule (2026-08-08); any frame-rate limiting happens on the encoder side.

As of 2026-08-08 the Mode B implementation is **scrcpy** (Apache-2.0, adopted
as an external engine rather than self-built — decision and evidence in
CONTROL_ARCHITECTURE.md). scrcpy operates exactly within the Mode B bounds:
pushed over user-authorized ADB for the session, runs at shell UID via
app_process, removed on exit, never store-distributed. The wrapper pins scrcpy
≥ 4.1 and rejects prereleases — the version verified on hardware;
the wrapper does not fork scrcpy.

## iOS

No exception applies. iOS uses public frameworks only. Windows/Linux→iPhone goes
through WebDriverAgent, which is Apple's own XCTest machinery; macOS→iPhone uses
Apple's iPhone Mirroring and ships no control code of ours.
