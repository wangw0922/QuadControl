#!/usr/bin/env sh
set -eu
if ! command -v swift >/dev/null 2>&1; then echo "BLOCKED: swift is not installed; macOS SwiftPM host was not built."; exit 0; fi
exec swift build --disable-sandbox
