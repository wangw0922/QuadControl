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
- A rejected handshake is closed without sending `DiagnosticError`, so an
  unauthenticated peer cannot distinguish a wrong, expired, or already-consumed
  token. The precise reason stays in the Mac listener log only.
- In loopback mode the listening port is specified once, through
  `NWListener(using:on:)`; `requiredLocalEndpoint` constrains the host only.
  Pinning an explicit port in both places is rejected with EINVAL.
- Solve token-entry friction with a scannable code, never by shortening the
  token. A low-entropy token is recoverable offline from one captured handshake,
  which online rate limiting cannot mitigate.
- The QR payload is JSON defined in the `.proto`, not a URL. A URL would invite
  the system to open it and would put the token in a place that gets logged.
- The macOS listener resolves and prints its own numeric private address instead
  of telling the user to run `ipconfig`; the QR needs a concrete host anyway.
- UI strings are keyed by meaning (`status.connected`), not by their English
  text, so translations cannot drift from test assertions.
- SwiftPM lowercases `.lproj` directories under `.process()`, which breaks
  language matching; each `.lproj` is declared with `.copy` instead.
- A CLI has no application bundle, so `Bundle.module` string lookup ignores the
  user's languages. The listener resolves the best-matching `.lproj` explicitly.
