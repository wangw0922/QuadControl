# Architecture

Rust shared crates define future domain, protocol, transport, crypto, media,
pairing, diagnostics, and FFI boundaries. Cross-platform schemas live in
`proto/quadcontrol/v1`. Apple M0 additionally has one root SwiftPM package:
`QuadControlDiagnosticProtocol` owns wire/auth/limits, the reusable
`QuadControlDiagnosticTransport` owns Network.framework framing, address policy,
and the client session, `QuadControlDiagnosticServer` owns the listener and
admission control, and `QuadControlMacListener` is a visible local diagnostic
host. The iOS SwiftUI
root owns a single `ConnectionModel`; it reuses shared products rather than
copying frames, authentication, or limits.
