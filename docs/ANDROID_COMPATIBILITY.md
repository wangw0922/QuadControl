# Android compatibility

M0 supports only passive ADB diagnostics. `adb version`, `adb devices -l`, and `adb mdns services` may be launched without a shell and have bounded output and timeouts. The reported mDNS pairing/connect services are advertisements, not proof that this host is paired or connected. `wireless_connected` requires a wireless entry in `adb devices -l` whose state is `device`. OEM and real-device behavior remain unverified.
