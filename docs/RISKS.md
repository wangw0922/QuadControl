# Risks

- iOS Screen Curtain + ReplayKit and software HID are P0 experiments and blocked until physical-device tests.
- Android transport classification uses explicit USB metadata or recognizable TCP/mDNS serial forms and remains a diagnostic heuristic; it does not authorize control.
- Invoking adb can start its local server; M0 does not persist device serials or network addresses.
- Platform host builds require toolchains not bundled by this repository.
- Apple diagnostic token display may leak through terminal scrollback or screenshots;
  in-memory `Data` cannot guarantee physical zeroization. iOS build/Simulator/
  signing/device results remain Blocked.
