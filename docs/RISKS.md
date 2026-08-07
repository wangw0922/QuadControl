# Risks

- iOS Screen Curtain + ReplayKit and software HID are P0 experiments and blocked until physical-device tests.
- macOS carries the HID profile and the peripheral-role APIs, but the peripheral
  role itself is unproven. Do not plan Mac→iPhone HID control on API presence
  alone; it needs the active experiment in `MACOS_HID_FEASIBILITY.md`.
- **Windows→iPhone depends on WebDriverAgent, a developer tool.** Signing expires
  (seven days on a free team), Apple can change XCTest behavior, and the quadrant
  can never be distributed through the App Store. This caps that quadrant at
  developer/technical-user reach.
- **The go-ios capability list is unverified by this project.** Treat it as a
  claim to test on hardware, not as a specification. Today's HID work is the
  cautionary case: a restated capability list and a measured result are not the
  same thing.
- **macOS→iPhone depends entirely on an Apple feature we do not control.** If
  Apple changes or restricts iPhone Mirroring, that quadrant has no fallback of
  ours, because we deliberately built none.
- Android transport classification uses explicit USB metadata or recognizable TCP/mDNS serial forms and remains a diagnostic heuristic; it does not authorize control.
- Invoking adb can start its local server; M0 does not persist device serials or network addresses.
- Platform host builds require toolchains not bundled by this repository.
- Apple diagnostic token display may leak through terminal scrollback or screenshots;
  the QR pairing code carries the same token and has the same exposure. In-memory
  `Data` cannot guarantee physical zeroization.
- The listener's QR must be read from a real terminal. Redirected or re-rendered
  output loses the explicit background colors and can invert the code, which iOS
  will not decode.
- The listener closes rejected handshakes without disclosing the reason, so the
  client can only report `network`. Diagnosing a refused token requires the Mac
  listener log, which is a deliberate trade of client-side clarity for not
  telling an unauthenticated peer why its token failed.
- Listener binding was only ever exercised on an ephemeral port until an explicit
  port was tested; treat any Network.framework parameter change as needing a
  test at the port the shipped binary actually requests.
