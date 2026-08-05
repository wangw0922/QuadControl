#!/usr/bin/env sh
set -u

status=0
if command -v cargo >/dev/null 2>&1; then
  for step in "cargo metadata --no-deps --format-version 1" "cargo fmt --all --check" "cargo clippy --workspace --all-targets -- -D warnings" "cargo test --workspace"; do if sh -c "$step"; then echo "PASS: $step"; else echo "FAIL: $step" >&2; status=1; fi; done
else echo "BLOCKED: cargo is not installed; Rust checks did not run."; fi
if command -v swift >/dev/null 2>&1; then
  if swift package --disable-sandbox describe >/dev/null; then echo "PASS: swift package --disable-sandbox describe"; else echo "FAIL: swift package --disable-sandbox describe" >&2; status=1; fi
  if swift build --disable-sandbox; then echo "PASS: swift build --disable-sandbox"; else echo "FAIL: swift build --disable-sandbox" >&2; status=1; fi
  if swift run --disable-sandbox QuadControlSelfTest; then echo "PASS: QuadControlSelfTest"; else echo "FAIL: QuadControlSelfTest" >&2; status=1; fi
  if command -v xcrun >/dev/null 2>&1 && xcrun --find xctest >/dev/null 2>&1; then
    if swift test --disable-sandbox; then echo "PASS: swift test --disable-sandbox"; else echo "FAIL: swift test --disable-sandbox" >&2; status=1; fi
  else
    echo "BLOCKED: XCTest is unavailable; XCTest targets did not run (self-test above is authoritative here)."
  fi
else echo "BLOCKED: swift is not installed; Apple checks did not run."; fi
if command -v xcodegen >/dev/null 2>&1 && command -v xcodebuild >/dev/null 2>&1; then
  if scripts/build-ios.sh; then echo "PASS: iOS generic Simulator build"; else echo "FAIL: iOS generic Simulator build" >&2; status=1; fi
else
  scripts/build-ios.sh
fi
exit "$status"
