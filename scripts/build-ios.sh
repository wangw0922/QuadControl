#!/usr/bin/env sh
set -eu
if ! command -v xcodegen >/dev/null 2>&1 || ! command -v xcodebuild >/dev/null 2>&1; then echo "BLOCKED: full Xcode and XcodeGen are required; apps/ios/project.yml is source-only and no iOS build ran."; exit 0; fi
cd "$(dirname "$0")/../apps/ios"
xcodegen generate --spec project.yml
xcodebuild \
  -project QuadControlIOS.xcodeproj \
  -scheme QuadControlIOS \
  -sdk iphonesimulator \
  -destination 'generic/platform=iOS Simulator' \
  CODE_SIGNING_ALLOWED=NO \
  build
