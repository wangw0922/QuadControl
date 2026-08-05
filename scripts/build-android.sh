#!/usr/bin/env sh
set -u
if [ ! -x "./gradlew" ]; then echo "SKIP: no Gradle wrapper/project exists in M0; Android host is not built."; exit 0; fi
./gradlew assembleDebug
