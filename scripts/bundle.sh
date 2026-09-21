#!/bin/bash
# Builds Minutes.app into ./build. Usage: scripts/bundle.sh [debug|release]
set -euo pipefail

cd "$(dirname "$0")/.."
CONFIG="${1:-release}"
APP="build/Minutes.app"

swift build -c "$CONFIG"
BIN="$(swift build -c "$CONFIG" --show-bin-path)"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN/Minutes" "$APP/Contents/MacOS/Minutes"
cp scripts/Info.plist "$APP/Contents/Info.plist"

# SwiftPM resource bundles (FluidAudio ships some) must sit next to the executable's resources.
find "$BIN" -maxdepth 1 -name "*.bundle" -exec cp -R {} "$APP/Contents/Resources/" \;

ICONSET="build/AppIcon.iconset"
rm -rf "$ICONSET"
swift scripts/make-icon.swift "$ICONSET"
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
rm -rf "$ICONSET"

# Ad-hoc signature: enough to run locally. macOS re-asks for permissions after each rebuild.
codesign --force --deep --sign - --entitlements scripts/Minutes.entitlements "$APP"
echo "Built $APP"
