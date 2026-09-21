#!/bin/bash
# Builds Minutes and installs it to /Applications. Usage: ./install.sh [--open] [--adhoc]
# Signs with the shared Developer ID certificate from 1Password (see ~/Documents/GitHub/APPLE_SIGNING.md).
# --adhoc skips that; macOS then re-asks for Keychain and privacy permissions after every build.
set -euo pipefail

cd "$(dirname "$0")"
SOURCE="build/Minutes.app"
DEST="/Applications/Minutes.app"

if ! command -v swift >/dev/null; then
    echo "error: Swift was not found. Install Xcode or the Command Line Tools first." >&2
    exit 1
fi

OPEN=0
ADHOC=0
for arg in "$@"; do
    case "$arg" in
        --open) OPEN=1 ;;
        --adhoc) ADHOC=1 ;;
        *) echo "error: unknown option $arg" >&2; exit 1 ;;
    esac
done

if [ "$ADHOC" = 0 ]; then
    echo "Loading the Developer ID certificate from 1Password…"
    # The shared loader does not pick an account. This is the user ID of the account that
    # holds the certificate: two accounts share my.1password.com, and the email does not select one.
    export OP_ACCOUNT="${OP_ACCOUNT:-YOUR_1PASSWORD_ACCOUNT}"
    # In that account the default vault is named "Private", not "Personal".
    export AGMUX_APPLE_SIGNING_VAULT="${AGMUX_APPLE_SIGNING_VAULT:-Private}"
    export AGMUX_APPLE_NOTARY_VAULT="${AGMUX_APPLE_NOTARY_VAULT:-Private}"
    # shellcheck disable=SC1091
    source scripts/load-apple-creds.sh
    trap minutes_cleanup_apple_creds EXIT
    minutes_disable_notarization_env
    minutes_require_developer_id
fi

echo "Building Minutes (release)…"
scripts/bundle.sh release >/dev/null

if [ ! -x "$SOURCE/Contents/MacOS/Minutes" ]; then
    echo "error: the build did not produce $SOURCE" >&2
    exit 1
fi

if pgrep -x Minutes >/dev/null; then
    echo "Quitting the running copy of Minutes…"
    osascript -e 'tell application "Minutes" to quit' >/dev/null 2>&1 || true
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        pgrep -x Minutes >/dev/null || break
        sleep 0.5
    done
    if pgrep -x Minutes >/dev/null; then
        echo "error: Minutes is still running (a recording may be in progress). Quit it and run this again." >&2
        exit 1
    fi
fi

echo "Installing to $DEST…"
if [ -d "$DEST" ]; then
    # Only ever removes the previous install of this app.
    rm -rf "$DEST"
fi
ditto "$SOURCE" "$DEST"

# Refresh the icon in Finder and the Dock after an update.
touch "$DEST"

echo "Installed Minutes $(defaults read "$DEST/Contents/Info" CFBundleShortVersionString)."
codesign -dvv "$DEST" 2>&1 | sed -n '/^Authority=Developer ID Application/p;/^Signature=adhoc/p' | head -1

if [ "$OPEN" = 1 ]; then
    open "$DEST"
fi
