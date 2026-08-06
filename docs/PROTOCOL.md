# Protocol

`proto/quadcontrol/v1/session.proto` is the schema-first M0 diagnostic contract.
Wire payloads are Codable envelopes framed by a 4-byte big-endian length and
are at most 4096 bytes. Version 1 accepts only hello, challenge, client proof,
server finished, heartbeat, ack, and error. Every nonce is 32 bytes; server,
challenge, and session IDs are 16 bytes; token/proof/MAC values are 32 bytes.

HMAC never covers JSON bytes. It covers a canonical binary transcript composed
of the ASCII domain `QuadControlDiagnostic`, a big-endian UInt32 version,
a length-prefixed operation label, and ordered length-prefixed fixed fields.
The 32-byte temporary token lasts 180 seconds;
the 10-second handshake derives a session-MAC key with HKDF-SHA256. Client
proof and independent server-finished authenticate both directions. Afterward,
heartbeats use strict increasing sequence numbers and a session MAC every five
seconds; 15 seconds without a valid heartbeat disconnects. Unknown version/type,
bad length, wrong state, duplicate/backward sequence, and bad MAC close. The
HMAC and HKDF encodings are pinned by golden vectors in `QuadControlSelfTest`.

Canonical encoding is `domain || be32(version) || lp(label) || lp(field1) ...`,
where `lp(x)` is `be32(byteCount(x)) || x`. Operations are exact:

- client proof: HMAC-SHA256 key = token, label `client-proof`, fields = client nonce, server nonce, challenge ID, server ID;
- session key: HKDF-SHA256 IKM = token, salt = client nonce concatenated with server nonce, info = canonical label `session-key` with the same four fields, output = 32 bytes;
- server finished: HMAC-SHA256 key = session key, label `server-finished`, fields = the same four handshake fields followed by session ID;
- client heartbeat: HMAC-SHA256 key = session key, label `client-heartbeat`, fields = session ID, 8-byte big-endian sequence, UTF-8 `alive`;
- server ack: HMAC-SHA256 key = session key, label `server-ack`, fields = session ID, 8-byte big-endian sequence, UTF-8 `ok`.

The heartbeat/ack status is decoded and validated before its semantic UTF-8
value enters the canonical transcript; raw JSON serialization is never signed.

`DiagnosticPairingPayload` is a separate, out-of-band message: the JSON the Mac
listener renders as a QR code so the phone does not have to type the token. It
carries only `protocol_version`, a numeric private host, a port, and the same
43-character token string. It is bounded to 512 bytes, is never a URL, and is
never opened by the system, persisted, or placed on a pasteboard. Scanning is
input, not authentication — the scanned host is still checked against the
client's private-address policy and the token still goes through the same decode,
so a hostile code cannot redirect the client or bypass any validation.
