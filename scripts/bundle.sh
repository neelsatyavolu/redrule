#!/bin/bash
# Builds the Swift app into ./build. Usage: scripts/bundle.sh [debug|release]
# Signs with $APPLE_SIGNING_IDENTITY when it is set (see scripts/load-apple-creds.sh), otherwise ad hoc.
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

if [ -n "${APPLE_SIGNING_IDENTITY:-}" ]; then
    # Developer ID gives the app a stable identity, so Keychain and privacy permissions survive rebuilds.
    codesign --force --deep --timestamp --options runtime \
        ${MINUTES_SIGN_KEYCHAIN:+--keychain "$MINUTES_SIGN_KEYCHAIN"} \
        --sign "$APPLE_SIGNING_IDENTITY" --entitlements scripts/Minutes.entitlements "$APP"
else
    # Ad-hoc signature: runs locally, but macOS re-asks for Keychain and privacy permissions after each rebuild.
    codesign --force --deep --sign - --entitlements scripts/Minutes.entitlements "$APP"
fi
echo "Built $APP"
