# Maintaining Redrule

Notes for the maintainer: releases, signing, hosting and the sharing service. Contributors don't need any of this; see [CONTRIBUTING.md](../CONTRIBUTING.md).

## Signed local builds

```bash
scripts/install-desktop.sh --open   # build, sign with the Developer ID, install to /Applications
```

Signing with a stable Developer ID keeps the Keychain's "Always Allow" and both privacy permissions across rebuilds. `scripts/load-apple-creds.sh` loads the certificate from 1Password through a shared loader script that lives outside this repo (set `AGMUX_APPLE_CREDS_LOADER` to its path). The 1Password account, vault names, signing identity and updater key reference come from `scripts/maintainer.local`, which git ignores; copy `scripts/maintainer.local.example` to create it. Without the loader, use `--adhoc`.

The Tauri app's bundle id is `co.neel.redrule`. On first launch it adopts data from the earlier copy: `~/Library/Application Support/Minutes` is renamed to `Redrule`, Keychain items under the service `Minutes` are copied to `Redrule`, and preferences are copied from the `co.nenu.minutes` domain. macOS asks for Microphone and Screen & System Audio Recording again, because the bundle id is new.

## Releases and updates

```bash
scripts/release.sh <version> "What changed"   # bump, build, sign, notarize, publish
git commit -am "chore: release <version>"
```

The script publishes `Redrule.dmg`, the signed update archive and `latest.json` to this repo's [GitHub Releases](https://github.com/neelsatyavolu/redrule/releases). Versions up to 0.3.0 were published to the separate `neelsatyavolu/redrule-releases` repo, which 0.3.0 and earlier still check for updates; 0.3.1 was published to both so those copies move over. Installed copies check `latest.json` a minute after launch and every six hours, install the update in the background and offer to restart (never during a recording). **Check for Updates…** is in the Redrule menu and the menu bar.

`release.sh` reads the Developer ID, the notary API key and the updater signing key from 1Password (`REDRULE_UPDATER_KEY` in `scripts/maintainer.local`). The updater key's public half is in `src-tauri/tauri.conf.json`. If the private key is lost, installed copies can't be updated, so never rotate it casually.

Crash reports are opt-in ("Send crash reports" in setup and Settings, off by default) and go to Sentry only from builds compiled with `REDRULE_SENTRY_DSN` set. The release script takes it from the environment and prints a warning when it is missing; the build still succeeds and never reports. Dev builds and forks have no DSN. For example, with the DSN stored in 1Password: `REDRULE_SENTRY_DSN="op://<vault>/<item>/dsn" op run -- scripts/release.sh 0.2.5 "What changed"`. Reports carry panics, uncaught webview errors, stack traces, the app version and the OS and Mac model, with home folder names and token-like strings removed (see `src-tauri/src/telemetry.rs`). Turn on "Prevent storing of IP addresses" in the Sentry project's security settings.

## Website and hosting

The landing page is `website/`. One Vercel project, connected to this repo, serves it together with the sharing API: `scripts/assemble-vercel-site.mjs` copies `website/` into `public/`, and `vercel.json` adds clean URLs (`/privacy`, `/terms`), the `/download` redirect to the newest `Redrule.dmg`, and the `/s/` and `/f/` rewrites for shared pages.

## Sharing service

The service lives in `sharing/` and stores shared meetings in private Vercel Blob storage. The app uses `https://redrule.vercel.app`.

Sharing and shared folders need no setup on any Mac. Each link gets its own random owner key, kept in this Mac's Keychain (service `Redrule`, account `share:<link id>`); the link's id is the SHA-256 of that key, so only the Mac that shared a link can update or remove it, and the service stores no key. Folders work the same way with their own owner and member keys.

`swift scripts/setup-sharing.swift` is only for the legacy `MINUTES_SHARE_KEY` service credential (Vercel environment secret plus Keychain account `sharing`). Links published by Redrule 0.2.4 and earlier have random ids and can only be updated or removed with that credential, so keep it on the Mac that made them; the service still accepts it for every link. Deploy service changes before releasing an app that depends on them: 0.2.4 keeps working against the new service, but a newer app can't publish against the old one.

The API limits each client IP in memory per function instance (creating or resetting a folder 20 per hour; publishing or removing links 60, folder writes 600 and folder pages 30 per 10 minutes) and caps each folder at 500 meetings and 200 MB. Those in-memory limits only stop bursts. For limits that hold across instances and regions, add Vercel WAF rules once from the repository root (`vercel link --cwd sharing` first; Hobby includes rate limiting). Start them in `log` mode, check **Firewall → Traffic**, then change the action to `rate_limit`:

```bash
vercel firewall rules add "Sharing writes" --cwd sharing \
  --condition '{"type":"path","op":"pre","value":"/api/"}' \
  --condition '{"type":"method","op":"inc","value":["POST","PUT","PATCH","DELETE"]}' \
  --action rate_limit --rate-limit-window 600 --rate-limit-requests 600 --rate-limit-keys ip --rate-limit-action log --yes
vercel firewall rules add "Folder pages" --cwd sharing \
  --condition '{"type":"path","op":"re","value":"^/f/[a-f0-9]{64}/?$"}' \
  --action rate_limit --rate-limit-window 600 --rate-limit-requests 60 --rate-limit-keys ip --rate-limit-action log --yes
vercel firewall diff --cwd sharing && vercel firewall publish --cwd sharing --yes
```

Checks:

```bash
cd sharing && npm ci && npm test && npm run build
python3 scripts/test-sharing.py   # after provisioning: checks the live service with synthetic content, then deletes it
```

Sharing behaviour: **Share** creates or updates a copyable link. The summary is shared by default; the transcript and speaker names are optional. Audio is never uploaded. Shared pages need no login. Edits stay local until **Update & copy link**. **Stop sharing** removes the hosted copy; deleting a shared meeting first revokes its link and needs a connection. Copies recipients already saved can't be recalled.

## Legacy Swift app

`Sources/`, `Tests/` and `Package.swift` hold the earlier Swift version of the app. It is no longer released; the Tauri app replaced it.

```bash
./install.sh --open        # builds and installs to /Applications/Minutes.app, then opens it
scripts/bundle.sh          # or: build build/Minutes.app without installing
swift test                 # unit tests for MinutesCore
```

Requires macOS 15 or later on Apple Silicon, and Xcode's Swift 6 toolchain. `install.sh` signs with the Developer ID like `install-desktop.sh`; `./install.sh --adhoc` and plain `scripts/bundle.sh` sign ad hoc.

| Piece | Where |
|---|---|
| Meeting detection rules, transcript merging, audio windowing, OAuth helpers, summary prompt and parsing, file store | `Sources/MinutesCore` (pure, unit tested) |
| Audio capture and the live transcription pipeline | `Sources/Minutes/Audio`, `Sources/Minutes/Transcription` |
| Call detection (processes, microphone users, browser window titles) | `Sources/Minutes/Detection` |
| Sign-in, Keychain storage, Codex and Grok clients | `Sources/Minutes/Providers` |
| App state | `Sources/Minutes/App` |
| Interface | `Sources/Minutes/UI` |

It stores meetings in `~/Library/Application Support/Minutes/meetings/<id>/` (`meeting.json`, `transcript.json`, `note.json`, `notes.md`), and uses FluidAudio's local diarizer to tell voices apart. The original design spec is in `docs/superpowers/specs/`.
