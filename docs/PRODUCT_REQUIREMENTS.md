# Product requirements

M0 establishes a safe foundation only. The intended product is Windows/macOS control of user-authorized Android or iPhone sessions. Screen-off means the physical display is off while the device remains active; it is not system lock. No product feature may bypass a lock screen, password, Face ID, Touch ID, payment confirmation, or protected content.

## Localization

Every client UI — Windows, macOS, Android, iPhone — must support Simplified
Chinese (`zh-Hans`) alongside English, including system permission prompts that
quote app-supplied text. User-visible strings must not be hardcoded in a view;
they belong in a localized resource keyed by meaning, not by English wording.

Machine-readable output is exempt and stays English: listener log lines, CLI
usage text, error codes on the wire, and accessibility identifiers. The
troubleshooting docs and tests match on those strings, so translating them would
break both.

The iOS client and the macOS listener's one-time-token block already follow this.
Windows, macOS, and Android UIs must honor it when they are built.

## Pairing input

A user must never have to type a 43-character token by hand as the only option.
The Mac listener shows the same values as a scannable code, and the phone can
scan it. Any additional client must offer an equivalent low-friction path.
Shortening or weakening the token itself is not an acceptable substitute: the
client proof is an HMAC over a transcript that a passive listener on the same
network can observe, so a low-entropy token can be recovered offline regardless
of how well online guessing is rate limited.
