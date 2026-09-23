# Redrule

Redrule is a macOS app that notices when you are in a Zoom or Google Meet call, records the call and your microphone, transcribes both on your Mac, and writes the meeting up using your ChatGPT (Codex) or Grok account.

## Desktop app (Tauri)

The Tauri app (React UI, Rust backend), bundle id `co.neel.redrule`. On first launch it adopts data from the previous copy: `~/Library/Application Support/Minutes` is renamed to `Redrule`, Keychain items under the service `Minutes` are copied to `Redrule`, and preferences are copied from the `co.nenu.minutes` domain. macOS asks for Microphone and Screen & System Audio Recording again, because the bundle id is new.

```bash
pnpm install
pnpm tauri dev                    # run with hot reload
scripts/install-desktop.sh --open # build, sign with the shared Developer ID, install to /Applications
cd src-tauri && cargo test --workspace && cd .. && pnpm test
```

| Piece | Where |
|---|---|
| Pure logic: models, store, transcript merging, windowing, detection rules, OAuth, summary prompt and parsing | `src-tauri/crates/core` |
| Audio capture (ScreenCaptureKit, cpal), Parakeet v3 transcription and speaker recognition (sherpa-onnx) | `src-tauri/crates/engine` |
| App state, commands, tray, menus, the call banner | `src-tauri/src` |
| macOS detection signals and permissions | `src-tauri/src/platform` |
| Accounts, Keychain, summary and sharing clients | `src-tauri/src/providers` |
| Interface | `src` (React, Tailwind) |

### Releases and updates

```bash
scripts/release.sh 0.2.2 "What changed"   # bump, build, sign, notarize, publish
git commit -am "chore: release 0.2.2"
```

The script publishes `Redrule.dmg`, the signed update archive and `latest.json` to the public repo [neelsatyavolu/redrule-releases](https://github.com/neelsatyavolu/redrule-releases). Installed copies check `latest.json` a minute after launch and every six hours, install the update in the background and offer to restart (never during a recording). **Check for Updates…** is in the Redrule menu and the menu bar. The key that signs updates is the 1Password item "Redrule Updater Signing Key" (Private vault). If it is lost, installed copies can't be updated, so never rotate it casually.

The landing page is `website/`, served with the sharing API by the Vercel project `redrule` (Git-connected to this repo). `/download` redirects to the newest `Redrule.dmg`.

`preview.html` renders the interface in a browser with sample data (`pnpm dev`, then open `/preview.html?view=notes`), for design work without the backend.

## Swift app

## Build and run

```bash
./install.sh --open        # builds and installs to /Applications, then opens it
scripts/bundle.sh          # or: build build/Minutes.app without installing
swift test                 # unit tests for MinutesCore
```

Requires macOS 15 or later on Apple Silicon, and Xcode's Swift 6 toolchain.

On first launch the app asks for two permissions and downloads the speech model (Parakeet TDT v3, several hundred MB, once).

- **Microphone**: your side of the conversation.
- **Screen & System Audio Recording**: macOS files call audio under this permission. The app captures sound only. Quit and reopen it after granting it.

`install.sh` signs with the shared Developer ID certificate from 1Password (see `~/Documents/GitHub/APPLE_SIGNING.md`; `scripts/load-apple-creds.sh` mirrors the Strix loader). That gives the app a stable identity, so the Keychain's "Always Allow" and both permissions survive rebuilds. `./install.sh --adhoc` and plain `scripts/bundle.sh` sign ad hoc instead, and macOS then asks again after every build.

## How it works

| Piece | Where |
|---|---|
| Meeting detection rules, transcript merging, audio windowing, OAuth helpers, summary prompt and parsing, file store | `Sources/MinutesCore` (pure, unit tested) |
| Audio capture and the live transcription pipeline | `Sources/Minutes/Audio`, `Sources/Minutes/Transcription` |
| Call detection (processes, microphone users, browser window titles) | `Sources/Minutes/Detection` |
| Sign-in, Keychain storage, Codex and Grok clients | `Sources/Minutes/Providers` |
| App state | `Sources/Minutes/App` |
| Interface | `Sources/Minutes/UI` |

Meetings are stored as plain files in `~/Library/Application Support/Minutes/meetings/<id>/`: `meeting.json`, `transcript.json`, `note.json` and `notes.md`. Audio is not written to disk unless "Keep audio recordings" is on.

The design spec is in `docs/superpowers/specs/`.

## Notes

- The Codex and Grok sign-ins reuse the OAuth clients of their official command-line tools, the same approach as apexline. Suitable for personal use only.
- API keys (OpenAI, Anthropic, Gemini, or any OpenAI-compatible server) are the alternative that does not depend on those clients. Keys are checked by listing the provider's models, then kept in the Keychain (service `Redrule`, account `apikey:<provider>`). A custom server's base URL and model name are preferences (`compatibleURL`, `compatibleModel`); plain http is accepted only for localhost.
- Recording other people can require their consent. Tell them.

## Notes, transcripts, and sharing

Completed meetings have **Notes** and **Transcript** tabs. The transcript includes timestamps and editable speaker labels. New recordings use FluidAudio's local diarizer to distinguish voices in the call audio; the microphone remains **Me**. Models download on first use. Speaker labels are estimates, particularly for short turns, similar voices and overlapping speech. Unknown or overlapping speakers retain **Them**. Detection failures preserve the transcript. Existing transcripts retain their original labels; there is no automatic reprocessing of old audio.

Choose **Share** to create or update a copyable link. The summary is shared by default; the transcript and speaker names are optional. Audio is never uploaded. Shared pages require no recipient login. Edits remain local until **Update & copy link**. **Stop sharing** removes the hosted copy; deleting a shared meeting first revokes its link and requires connectivity. Copies already saved by recipients cannot be recalled.

The service lives in `sharing/` and uses private Vercel Blob storage. The marketing site and the sharing API are one Vercel project, `redrule`, connected to this repo. The app uses `https://redrule.vercel.app`. Uploads and revocation require a dedicated `MINUTES_SHARE_KEY` environment secret and the same credential in this Mac's Keychain (service `Redrule`, account `sharing`); no service credential is embedded in the app. After connecting the private Blob store, run `swift scripts/setup-sharing.swift` from the repository root to provision this Mac, then redeploy. This setup is for a personal installation; other Macs must be provisioned with the same service credential rather than independently rotating it.

Web checks: `cd sharing && npm ci && npm test && npm run build`. After provisioning, `python3 scripts/test-sharing.py` (from the repository root) checks the live service with synthetic content and deletes the test share afterward.
