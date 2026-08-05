# Threat model

Primary threats are unauthorized pairing, remote observation, injected input,
replay, token disclosure, and unsafe persistence. Apple M0 mitigates these with
a 32-byte 180-second one-time token, challenge-response, atomic consumption,
HKDF session MACs, strict heartbeat sequence, bounded framing/state, visible
user start/stop, loopback default, and explicit private-LAN opt-in. It is not
confidential session encryption and accepts only non-sensitive diagnostics.
Terminal scrollback/screenshots and non-zeroable memory are residual risks.
The ADB probe runs only a fixed parameterized whitelist with time/output limits;
it never infers host pairing from mDNS.
