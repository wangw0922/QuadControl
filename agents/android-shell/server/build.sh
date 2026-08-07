#!/bin/sh
# Builds the device-side capability probe into a dex jar.
#
# No Gradle and no Android Gradle Plugin: this is not an APK. It is launched by
# `app_process`, so it needs no manifest, no resources, and no aapt2 — only
# `android.jar` to compile against and `d8` to dex.
#
# Set ANDROID_HOME, or let this find the usual macOS location. Reports SKIP rather
# than failing when the SDK is absent, matching scripts/verify-all.sh.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
out="$here/build"
sdk=${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}

if [ ! -d "$sdk" ]; then
    echo "SKIP: Android SDK not found at $sdk; set ANDROID_HOME"
    exit 0
fi

platform=$(ls -d "$sdk"/platforms/android-* 2>/dev/null | sort -V | tail -1 || true)
buildtools=$(ls -d "$sdk"/build-tools/* 2>/dev/null | sort -V | tail -1 || true)

if [ -z "$platform" ] || [ ! -f "$platform/android.jar" ]; then
    echo "SKIP: no android.jar under $sdk/platforms"
    exit 0
fi
if [ -z "$buildtools" ] || [ ! -x "$buildtools/d8" ]; then
    echo "SKIP: no d8 under $sdk/build-tools"
    exit 0
fi

rm -rf "$out"
mkdir -p "$out/classes"

# Android runs a subset of Java. Target 8 bytecode so d8 accepts it without desugaring
# surprises, and compile against android.jar alone so nothing from the host JDK leaks in.
javac \
    -source 8 -target 8 \
    -bootclasspath "$platform/android.jar" \
    -nowarn \
    -d "$out/classes" \
    "$here"/src/com/quadcontrol/shell/*.java

"$buildtools/d8" \
    --lib "$platform/android.jar" \
    --output "$out" \
    --min-api 24 \
    "$out"/classes/com/quadcontrol/shell/*.class

# app_process reads a jar, so wrap the dex. `classes.dex` must sit at the archive root.
(cd "$out" && jar cf quadcontrol-shell.jar classes.dex)

echo "PASS: built $out/quadcontrol-shell.jar"
echo "      platform: $(basename "$platform")  build-tools: $(basename "$buildtools")"
