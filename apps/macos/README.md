# macOS control client boundary

Two responsibilities, with very different sizes.

## → Android (self-built)

The real work: ADB transport, screen capture, input injection, screen-off
control, built on the shared Rust crates. Not implemented.

## → iPhone (delegated to Apple)

QuadControl implements **no** control code here. Apple's iPhone Mirroring covers
this quadrant officially and is the only form Apple permits. The complete set of
permitted actions is:

- check that `/System/Applications/iPhone Mirroring.app` exists,
- check the system version,
- launch it with `open -b com.apple.ScreenContinuity`,
- explain the prerequisites in the UI and docs.

It exposes no URL scheme and no public API. Driving or automating its interface
would breach the no-private-API rule and is out of scope, not merely unbuilt.

## Existing code in this directory

`QuadControlMacListener` is a diagnostic listener from the earlier self-built
Mac↔iPhone link. That link is no longer on the product path. Run it with
`swift run QuadControlMacListener`; see [connect and test](../../docs/CONNECT_AND_TEST.md).
It is a SwiftPM executable, not an `.app` bundle, and it is not a product
capability.
