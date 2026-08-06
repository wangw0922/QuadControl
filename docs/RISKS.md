# Risks

- iOS Screen Curtain + ReplayKit and software HID are P0 experiments and blocked until physical-device tests.
- Android transport classification uses explicit USB metadata or recognizable TCP/mDNS serial forms and remains a diagnostic heuristic; it does not authorize control.
- Invoking adb can start its local server; M0 does not persist device serials or network addresses.
- Platform host builds require toolchains not bundled by this repository.
- Apple diagnostic token display may leak through terminal scrollback or screenshots;
  in-memory `Data` cannot guarantee physical zeroization. iOS Simulator build and
  test now pass, but signing and physical-device results remain Blocked.
- The listener closes rejected handshakes without disclosing the reason, so the
  client can only report `network`. Diagnosing a refused token requires the Mac
  listener log, which is a deliberate trade of client-side clarity for not
  telling an unauthenticated peer why its token failed.
- Listener binding was only ever exercised on an ephemeral port until an explicit
  port was tested; treat any Network.framework parameter change as needing a
  test at the port the shipped binary actually requests.
