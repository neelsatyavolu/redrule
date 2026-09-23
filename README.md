# Redrule

Redrule is a free macOS app that notices when you are in a Zoom or Google Meet call, records the call and your microphone, and transcribes both on your Mac. When the call ends, it writes the notes: a summary, decisions and action items. No bot joins the meeting, and audio never leaves your Mac.

**[Download for Mac](https://redrule.vercel.app/download)**. Requires macOS 15 or later on Apple Silicon.

## Features

- Offers to record when a Zoom or Google Meet call starts. Other calls can be recorded from the menu bar.
- Live transcription on your Mac with NVIDIA Parakeet. Your microphone is labelled **Me**, and other voices are told apart.
- Notes written on your Mac by a local Qwen3.5 model, or by the provider you choose.
- Ask questions about a meeting, check off action items and tag meetings.
- Share a meeting's notes, and optionally its transcript, with a link. **Stop sharing** removes the hosted copy.
- Updates install in the background, never during a recording.

## Privacy

Recording and transcription happen on your Mac. Meetings are plain files in `~/Library/Application Support/Redrule/meetings`, and audio isn't saved unless you turn on **Keep audio recordings**. The transcript leaves your Mac only if you choose a provider to write notes, or share a meeting. Redrule has no accounts, ads or analytics. The [privacy policy](https://redrule.vercel.app/privacy) has the details.

## Note-writing providers

- **On your Mac**: a local Qwen3.5 model. Nothing leaves your Mac, and it works offline.
- **API key**: your own OpenAI, Anthropic, Google Gemini or OpenAI-compatible key.
- **ChatGPT (Codex) or Grok sign-in**: these sign-ins reuse the OAuth clients of the providers' official command-line tools. They could stop working if a provider changes those clients. API keys and the local model are the stable options.

Recording other people can require their consent. Tell them.

## Building from source

```bash
pnpm install
pnpm tauri dev
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for prerequisites, tests and signing.

| Piece | Where |
|---|---|
| Pure logic: models, store, transcript merging, windowing, detection rules, OAuth, summary prompt and parsing | `src-tauri/crates/core` |
| Audio capture (ScreenCaptureKit, cpal), Parakeet v3 transcription and speaker recognition (sherpa-onnx), local notes (llama.cpp) | `src-tauri/crates/engine` |
| App state, commands, tray, menus, the call banner | `src-tauri/src` |
| macOS detection signals and permissions | `src-tauri/src/platform` |
| Accounts, Keychain, summary and sharing clients | `src-tauri/src/providers` |
| Interface | `src` (React, Tailwind) |
| Sharing service | `sharing` (Vercel functions, Vercel Blob) |
| Landing page, privacy policy and terms | `website` |

`Sources/` holds the earlier Swift version of the app, which is no longer released. Release, signing and hosting notes are in [docs/MAINTAINING.md](docs/MAINTAINING.md).

## License

[MIT](LICENSE)
