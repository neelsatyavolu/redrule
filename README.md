# Minutes

A macOS app that notices when you are in a Zoom or Google Meet call, records the call and your microphone, transcribes both on your Mac, and writes the meeting up using your ChatGPT (Codex) or Grok account.

## Build and run

```bash
./install.sh --open        # builds and installs to /Applications, then opens it
scripts/bundle.sh          # or: build build/Minutes.app without installing
swift test                 # unit tests for MinutesCore
```

Requires macOS 15 or later on Apple Silicon, and Xcode's Swift 6 toolchain.

On first launch Minutes asks for two permissions and downloads the speech model (Parakeet TDT v3, several hundred MB, once).

- **Microphone**: your side of the conversation.
- **Screen & System Audio Recording**: macOS files call audio under this permission. Minutes captures sound only. Quit and reopen the app after granting it.

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
- Recording other people can require their consent. Tell them.
