#!/bin/sh
# Pushes the capability probe, runs it, and removes it again.
#
# The server is never left on the device: Mode B is a session-scoped tool, not
# something that installs or persists. Cleanup runs even when the probe fails.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
jar="$here/build/quadcontrol-shell.jar"
remote=/data/local/tmp/quadcontrol-shell.jar

if ! command -v adb >/dev/null 2>&1; then
    echo "SKIP: adb is not installed"
    exit 0
fi
if [ ! -f "$jar" ]; then
    echo "SKIP: $jar missing; run build.sh first"
    exit 0
fi
if [ -z "$(adb devices | awk 'NR>1 && $2=="device"')" ]; then
    echo "SKIP: no authorized device; check the USB-debugging prompt on the handset"
    exit 0
fi

cleanup() {
    adb shell rm -f "$remote" >/dev/null 2>&1 || true
}
trap cleanup EXIT INT TERM

adb push "$jar" "$remote" >/dev/null
adb shell CLASSPATH="$remote" app_process / com.quadcontrol.shell.CapabilityProbe
