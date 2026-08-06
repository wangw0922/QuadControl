# Risks

- iOS Screen Curtain + ReplayKit and software HID are P0 experiments and blocked until physical-device tests.
- macOS carries the HID profile and the peripheral-role APIs, but the peripheral
  role itself is unproven. Do not plan Mac→iPhone HID control on API presence
  alone; it needs the active experiment in `MACOS_HID_FEASIBILITY.md`.
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
