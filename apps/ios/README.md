# iOS diagnostic source — not on the product path

**The project no longer ships an iOS application.** Neither iPhone quadrant needs
one: Windows→iPhone installs WebDriverAgent via go-ios, and macOS→iPhone uses
Apple's iPhone Mirroring. See [control architecture](../../docs/CONTROL_ARCHITECTURE.md).

What remains here is the diagnostic client built to validate a self-built
Mac↔iPhone link before that link was removed from the architecture. It is kept
because it is the only component in this repository with real-device end-to-end
verification — signed and installed on an iPhone SE 3, connected over Wi-Fi LAN,
with QR pairing — and it is useful for diagnosing LAN connectivity.

It must not be described as a product capability.

`project.yml` is an XcodeGen source manifest referencing the root Swift package;
the generated `.xcodeproj` is not committed. Real-device builds take the
development team on the command line rather than storing personal signing
information in the repository.

If the diagnostic loop stops earning its keep, this directory and the
`QuadControlDiagnostic*` targets can be removed together.
