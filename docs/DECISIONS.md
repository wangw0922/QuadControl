# Decisions

- Use a std-only Rust workspace at M0 to avoid toolchain/network dependency expansion.
- Preserve Protocol Buffers as source schemas without requiring `protoc`.
- Treat unsupported `adb mdns services` as partial diagnostic success; missing adb and failed core commands are errors.
- Report `paired_with_this_host` as `Unknown` because read-only discovery cannot observe it reliably.
- Keep Apple M0 as a visible, non-sensitive diagnostic connection only: SwiftPM
  owns the one schema adapter/auth implementation and the iOS manifest reuses it.
- Do not generate an Xcode project or claim iOS build evidence without XcodeGen,
  full Xcode, signing, Simulator, and physical-device authorization.
- Derive `wireless_connected` only from a wireless `adb devices -l` entry in `device` state; an mDNS connect advertisement is discovery evidence, not an active connection.
