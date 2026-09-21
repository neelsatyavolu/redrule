#!/bin/bash
# Builds Minutes and installs it to /Applications. Usage: ./install.sh [--open]
set -euo pipefail

cd "$(dirname "$0")"
SOURCE="build/Minutes.app"
DEST="/Applications/Minutes.app"

if ! command -v swift >/dev/null; then
    echo "error: Swift was not found. Install Xcode or the Command Line Tools first." >&2
    exit 1
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
echo "macOS asks for Microphone and Screen & System Audio Recording again after each new build."

if [ "${1:-}" = "--open" ]; then
    open "$DEST"
fi
