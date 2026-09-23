#!/bin/bash
# Publishes a Redrule release. Bumps the version, builds the app, signs and notarizes it with the
# shared Developer ID from 1Password, then uploads the download and the auto-update files to the
# public releases repo. Installed copies pick the release up within six hours, or right away from
# "Check for Updates…". The website's download button always serves the newest Redrule.dmg.
# Usage: scripts/release.sh <version> ["what changed"]
set -euo pipefail

cd "$(dirname "$0")/.."
VERSION="${1:-}"
NOTES="${2:-}"
REPO="neelsatyavolu/redrule-releases"
ARCH="aarch64"
UPDATER_KEY="op://Private/Redrule Updater Signing Key"
BUNDLE="src-tauri/target/release/bundle"
APP="$BUNDLE/macos/Redrule.app"
TARBALL="$BUNDLE/macos/Redrule.app.tar.gz"
DMG="$BUNDLE/dmg/Redrule_${VERSION}_${ARCH}.dmg"

if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "usage: scripts/release.sh <version, e.g. 0.2.1> [\"what changed\"]" >&2
    exit 1
fi
if [ "$(uname -m)" != "arm64" ]; then
    echo "error: releases are built on Apple Silicon." >&2
    exit 1
fi
for tool in pnpm cargo gh jq op xcrun; do
    if ! command -v "$tool" >/dev/null; then
        echo "error: $tool was not found. Install it first." >&2
        exit 1
    fi
done
if gh release view "v$VERSION" -R "$REPO" >/dev/null 2>&1; then
    echo "error: v$VERSION is already published. Pick a higher version." >&2
    exit 1
fi

echo "Setting the version to $VERSION…"
perl -0pi -e "s/\"version\": \"[^\"]*\"/\"version\": \"$VERSION\"/" src-tauri/tauri.conf.json package.json
perl -0pi -e "s/(\[workspace\.package\]\nversion = )\"[^\"]*\"/\${1}\"$VERSION\"/" src-tauri/Cargo.toml

echo "Loading the Developer ID certificate, notary key and update key from 1Password…"
export OP_ACCOUNT="${OP_ACCOUNT:-YOUR_1PASSWORD_ACCOUNT}"
export AGMUX_APPLE_SIGNING_VAULT="${AGMUX_APPLE_SIGNING_VAULT:-Private}"
export AGMUX_APPLE_NOTARY_VAULT="${AGMUX_APPLE_NOTARY_VAULT:-Private}"
# shellcheck disable=SC1091
source scripts/load-apple-creds.sh
STAGE="$(mktemp -d)"
cleanup() {
    minutes_cleanup_apple_creds
    rm -f "$STAGE"/*
    rmdir "$STAGE"
}
trap cleanup EXIT
minutes_require_developer_id
if [ -z "${APPLE_API_KEY:-}" ] || [ -z "${APPLE_API_ISSUER:-}" ] || [ -z "${APPLE_API_KEY_PATH:-}" ]; then
    echo "error: the notary API key was not found in 1Password, so the release can't be notarized." >&2
    exit 1
fi
TAURI_SIGNING_PRIVATE_KEY="$(op read "$UPDATER_KEY/key")"
TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(op read "$UPDATER_KEY/password")"
export TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PASSWORD
# Crash reports are compiled in from the environment; a build without the DSN never sends any.
if [ -n "${REDRULE_SENTRY_DSN:-}" ]; then
    export REDRULE_SENTRY_DSN
else
    echo "warning: REDRULE_SENTRY_DSN is not set, so this build can't send crash reports." >&2
fi

echo "Building, signing and notarizing Redrule $VERSION (this takes several minutes)…"
rm -f "$TARBALL" "$TARBALL.sig" "$DMG"
pnpm install --frozen-lockfile >/dev/null
pnpm tauri build --bundles app,dmg

echo "Checking the app is signed, notarized and accepted by Gatekeeper…"
codesign --verify --deep --strict "$APP"
xcrun stapler validate "$APP"
spctl --assess --type execute "$APP"

echo "Notarizing the disk image…"
codesign --force --timestamp ${MINUTES_SIGN_KEYCHAIN:+--keychain "$MINUTES_SIGN_KEYCHAIN"} \
    --sign "$APPLE_SIGNING_IDENTITY" "$DMG"
xcrun notarytool submit "$DMG" --key "$APPLE_API_KEY_PATH" --key-id "$APPLE_API_KEY" \
    --issuer "$APPLE_API_ISSUER" --wait
xcrun stapler staple "$DMG"

echo "Writing the update feed…"
UPDATE_NAME="Redrule_${VERSION}_${ARCH}.app.tar.gz"
cp "$DMG" "$STAGE/Redrule_${VERSION}_${ARCH}.dmg"
cp "$DMG" "$STAGE/Redrule.dmg"
cp "$TARBALL" "$STAGE/$UPDATE_NAME"
jq -n \
    --arg version "$VERSION" \
    --arg notes "$NOTES" \
    --arg date "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --arg signature "$(<"$TARBALL.sig")" \
    --arg url "https://github.com/$REPO/releases/download/v$VERSION/$UPDATE_NAME" \
    '{version: $version, notes: $notes, pub_date: $date,
      platforms: {"darwin-aarch64": {signature: $signature, url: $url}}}' >"$STAGE/latest.json"

echo "Publishing v$VERSION to $REPO…"
gh release create "v$VERSION" -R "$REPO" --latest \
    --title "Redrule $VERSION" --notes "${NOTES:-Redrule $VERSION}" "$STAGE"/*

echo
echo "Published Redrule $VERSION: https://github.com/$REPO/releases/tag/v$VERSION"
echo "Commit the version bump: git commit -am \"chore: release $VERSION\""
