#!/bin/bash
# Builds the Tauri version of Minutes and installs it to /Applications. Usage: scripts/install-desktop.sh [--open] [--adhoc] [--build-only]
# Signs with the shared Developer ID certificate from 1Password, like install.sh, so the Keychain's
# "Always Allow" and both privacy permissions survive rebuilds. --adhoc skips that.
set -euo pipefail

cd "$(dirname "$0")/.."
SOURCE="src-tauri/target/release/bundle/macos/Minutes.app"
DEST="/Applications/Minutes.app"

OPEN=0
ADHOC=0
INSTALL=1
for arg in "$@"; do
    case "$arg" in
        --open) OPEN=1 ;;
        --adhoc) ADHOC=1 ;;
        --build-only) INSTALL=0 ;;
        *) echo "error: unknown option $arg" >&2; exit 1 ;;
    esac
done

for tool in pnpm cargo; do
    if ! command -v "$tool" >/dev/null; then
        echo "error: $tool was not found. Install it first." >&2
        exit 1
    fi
done

if [ "$ADHOC" = 0 ]; then
    echo "Loading the Developer ID certificate from 1Password…"
    export OP_ACCOUNT="${OP_ACCOUNT:-YOUR_1PASSWORD_ACCOUNT}"
    export AGMUX_APPLE_SIGNING_VAULT="${AGMUX_APPLE_SIGNING_VAULT:-Private}"
    export AGMUX_APPLE_NOTARY_VAULT="${AGMUX_APPLE_NOTARY_VAULT:-Private}"
    # shellcheck disable=SC1091
    source scripts/load-apple-creds.sh
    trap minutes_cleanup_apple_creds EXIT
    minutes_disable_notarization_env
    minutes_require_developer_id
fi

echo "Building Minutes (release)…"
pnpm install --frozen-lockfile >/dev/null
# Sign below instead, so the identity's temporary keychain can be named explicitly.
env -u APPLE_SIGNING_IDENTITY pnpm tauri build --bundles app

if [ -n "${APPLE_SIGNING_IDENTITY:-}" ]; then
    codesign --force --deep --timestamp --options runtime \
        ${MINUTES_SIGN_KEYCHAIN:+--keychain "$MINUTES_SIGN_KEYCHAIN"} \
        --sign "$APPLE_SIGNING_IDENTITY" --entitlements src-tauri/Entitlements.plist "$SOURCE"
else
    codesign --force --deep --sign - --entitlements src-tauri/Entitlements.plist "$SOURCE"
fi
echo "Built $SOURCE"

if [ "$INSTALL" = 0 ]; then
    exit 0
fi

if pgrep -x Minutes >/dev/null || pgrep -x minutes >/dev/null; then
    echo "Quitting the running copy of Minutes…"
    osascript -e 'tell application "Minutes" to quit' >/dev/null 2>&1 || true
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        pgrep -x Minutes >/dev/null || pgrep -x minutes >/dev/null || break
        sleep 0.5
    done
    if pgrep -x Minutes >/dev/null || pgrep -x minutes >/dev/null; then
        echo "error: Minutes is still running (a recording may be in progress). Quit it and run this again." >&2
        exit 1
    fi
fi

echo "Installing to ${DEST}…"
if [ -d "$DEST" ]; then
    # Only ever removes the previous install of this app.
    rm -rf "$DEST"
fi
ditto "$SOURCE" "$DEST"
touch "$DEST"

echo "Installed Minutes $(defaults read "$DEST/Contents/Info" CFBundleShortVersionString)."
codesign -dvv "$DEST" 2>&1 | sed -n '/^Authority=Developer ID Application/p;/^Signature=adhoc/p' | head -1

if [ "$OPEN" = 1 ]; then
    open "$DEST"
fi
