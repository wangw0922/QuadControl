# Android compatibility

M0 supports only passive ADB diagnostics. `adb version`, `adb devices -l`, and `adb mdns services` may be launched without a shell and have bounded output and timeouts. The reported mDNS pairing/connect services are advertisements, not proof that this host is paired or connected. `wireless_connected` requires a wireless entry in `adb devices -l` whose state is `device`.

## Verified on hardware

Samsung SM_S9180 over USB, platform-tools 37.0.1, adb bridge 1.0.41. The probe
was exercised in both the `unauthorized` state (before the on-device prompt was
approved) and `device` state, and reported each correctly, including
`transport: usb` and `wireless_connected: false`.

Two things real output has that the hand-written fixtures never did:

- extra `model:` and `device:` fields, and **`device:dm3q` collides with the
  `device` state keyword**. Parsing is positional, so the collision is harmless —
  there are now regression tests pinning that.
- `adb version` prints four lines. Only the first carries the bridge version; the
  platform-tools build (`Version 37.0.1-15733141`) is deliberately not reported,
  though it is often the more useful number when diagnosing OEM issues.

`paired_with_this_host` stayed `unknown` against a real device, which is the
intended answer — read-only probing cannot establish it.

Other OEMs, wireless debugging, and any screen capture or input path remain
unverified. Nothing beyond passive discovery is implemented.
