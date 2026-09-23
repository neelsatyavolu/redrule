#!/bin/bash
# Builds the Swift app and installs it to /Applications. Usage: ./install.sh [--open] [--adhoc]
# Signs with the maintainer's Developer ID certificate from 1Password (see docs/MAINTAINING.md).
# --adhoc skips that and needs no certificate; macOS then re-asks for Keychain and privacy permissions
# after every build.
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
    # Maintainer settings (1Password account, vaults, signing identity) live in an untracked file.
    # shellcheck disable=SC1091
    [ -f scripts/maintainer.local ] && source scripts/maintainer.local
    # shellcheck disable=SC1091
    source scripts/load-apple-creds.sh
    trap minutes_cleanup_apple_creds EXIT
    minutes_disable_notarization_env
    minutes_require_developer_id
fi

echo "Building the Swift app (release)…"
scripts/bundle.sh release >/dev/null

if [ ! -x "$SOURCE/Contents/MacOS/Minutes" ]; then
    echo "error: the build did not produce $SOURCE" >&2
    exit 1
fi

if pgrep -x Minutes >/dev/null; then
    echo "Quitting the running copy…"
    osascript -e 'tell application "Redrule" to quit' >/dev/null 2>&1 || true
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        pgrep -x Minutes >/dev/null || break
        sleep 0.5
    done
    if pgrep -x Minutes >/dev/null; then
        echo "error: the app is still running (a recording may be in progress). Quit it and run this again." >&2
        exit 1
    fi
fi

echo "Installing to ${DEST}…"
if [ -d "$DEST" ]; then
    # Only ever removes the previous install of this app.
    rm -rf "$DEST"
fi
ditto "$SOURCE" "$DEST"

# Refresh the icon in Finder and the Dock after an update.
touch "$DEST"

echo "Installed $(defaults read "$DEST/Contents/Info" CFBundleShortVersionString)."
codesign -dvv "$DEST" 2>&1 | sed -n '/^Authority=Developer ID Application/p;/^Signature=adhoc/p' | head -1

if [ "$OPEN" = 1 ]; then
    open "$DEST"
fi
