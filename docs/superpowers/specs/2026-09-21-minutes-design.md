# Minutes — design

A native macOS app that detects meetings, records system audio and the microphone, transcribes locally, and produces a Granola-style meeting note using the user's Codex (ChatGPT) or Grok account.

## Decisions

| Topic | Decision |
|---|---|
| Stack | Swift 6, SwiftUI, macOS 15+, SwiftPM executable bundled into `Minutes.app` by `scripts/bundle.sh`. Not sandboxed. |
| Notes flow | Fully automatic. No note editor during the meeting. |
| Transcription | Parakeet TDT v3 through FluidAudio (CoreML, Neural Engine). Hidden behind a `Transcriber` protocol. |
| Speakers | Mic stream is labelled "Me", system audio is labelled "Them". No diarization in v1. |
| Summaries | Codex (Responses endpoint) or Grok (chat completions), OAuth PKCE ported from apexline `electron/main.cjs`. |
| Credentials | macOS Keychain, one JSON token bundle per provider, service `Minutes`. |
| Storage | One folder per meeting under `~/Library/Application Support/Minutes/meetings/<id>/`. No database. |
| Audio retention | Deleted after transcription by default. Setting to keep it. |

## Components

Each unit has one job and is reachable through a small interface.

### MinutesCore (library, unit tested)

- `Meeting`, `TranscriptSegment`, `MeetingNote` — value types, `Codable`, immutable updates.
- `MeetingStore` — `list()`, `load(id)`, `save(meeting)`, `saveTranscript`, `saveNote`, `delete(id)`. Files: `meeting.json`, `transcript.json`, `notes.md`, `note.json`.
- `MeetingDetectorLogic` — pure state machine. Input: a `DetectionSnapshot` (mic in use, running process names, window titles). Output: `.idle`, `.detected(app)`, `.ended`. Debounced so a brief mic blip does not fire.
- `TranscriptMerger` — interleaves mic and system segments by time, merges adjacent segments from the same speaker, renders the prompt text.
- `TranscriptChunker` — splits a long transcript into chunks under a character budget on segment boundaries.
- `PKCE`, `OAuthConfig`, `OAuthCallbackParser`, `TokenBundle` — PKCE generation, authorize URL building, parsing a pasted code or callback URL, expiry checks.
- `SummaryPrompt`, `SummaryParser` — builds the prompt and JSON schema; parses the model's JSON into `MeetingNote`; renders `MeetingNote` as Markdown.
- `CodexStreamParser` — concatenates `delta` text out of the Responses SSE body.

### Minutes (app target)

- `AudioCapture` — `SystemAudioCapture` (ScreenCaptureKit, audio only) and `MicCapture` (AVAudioEngine). Both deliver 16 kHz mono `Float` buffers to a callback.
- `ParakeetTranscriber` — conforms to `Transcriber`. Buffers ~15 s windows per stream, transcribes, emits `TranscriptSegment`s.
- `MeetingDetector` — every 3 s builds a `DetectionSnapshot` (CoreAudio `kAudioDevicePropertyDeviceIsRunningSomewhere`, `NSWorkspace` running apps, `CGWindowListCopyWindowInfo` titles) and feeds `MeetingDetectorLogic`.
- `RecordingSession` — coordinates capture → transcriber → store → summarizer. Owns the state shown in the UI: `idle`, `recording`, `transcribing`, `summarizing`, `done`, `failed(message)`.
- `OAuthService` — browser open, loopback listener (`Network.framework`), token exchange, refresh 30 s before expiry, Keychain persistence.
- `CodexClient`, `GrokClient` — conform to `SummaryProvider.complete(system:user:schema:)`.
- `Summarizer` — single pass under the chunk budget, otherwise map (per-chunk digest) then reduce (final note).
- UI: `MainWindow` (sidebar, note reader, transcript drawer), `DetectionPanel` (floating `NSPanel`), `MenuBarExtra`, `OnboardingView`, `SettingsView`.

## Detection rules

- Zoom: process `CptHost` is running (it only exists during a Zoom meeting), and the mic is in use.
- Google Meet: mic in use and a window owned by a browser has a title starting with `Meet` or containing `Google Meet`.
- Detected after two consecutive positive snapshots. Ended after three consecutive negative snapshots.
- A dismissed meeting does not prompt again until it has ended.

## Data flow

1. Detector fires → `DetectionPanel` appears top-right with Record / Dismiss. Recording can also be started by hand from the menu bar or main window.
2. Record → both captures start; buffers go to the transcriber; segments append to `transcript.json` as they arrive.
3. Stop (user, or the user accepts the "meeting ended" prompt) → captures stop, remaining audio is flushed, transcript is merged.
4. Summarizer calls the preferred connected provider → `note.json` + `notes.md` saved → the note opens in the main window.
5. Audio files are removed unless retention is on.

## Error handling

- Missing permission: the onboarding and the recording button explain which permission is missing and deep-link to the right System Settings pane.
- Model not downloaded: recording still captures audio; transcription runs when the model is ready.
- No provider connected or summary fails: the meeting keeps its transcript and shows a "Generate notes" retry button with the error text.
- Token refresh failure: provider is marked disconnected and the user is asked to reconnect.
- Model JSON that does not parse: one retry, then the raw text is saved as the note body.

## Design direction

Editorial and calm. Warm paper and ink in light and dark, New York serif for note headings, SF for interface text, one amber accent reserved for recording state. Native sidebar vibrancy, generous line height in the reader, tabular numerals for timers.

## Testing

Unit tests for everything in MinutesCore, written before the implementation. Audio capture, permissions, OAuth against the live providers, and the detection popup are checked by hand in the built app.

## Out of scope for v1

Calendar integration, chat about a meeting, sharing or sync, Teams/Slack/FaceTime detection, note editing, diarization of remote speakers.

## Known risks

- The Codex connection presents itself as the Codex CLI's OAuth client, as apexline does. Fine for personal use; not suitable for distribution.
- No code-signing identity on this machine, so builds are ad-hoc signed and macOS asks for Screen Recording and Microphone permission again after each rebuild.
- Recording other people has consent implications. The app always shows a visible recording state.
