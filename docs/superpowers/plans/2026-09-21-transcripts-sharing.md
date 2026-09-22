# Transcripts and Sharing Implementation Plan

**Goal:** Make completed notes and transcripts equally accessible, identify speakers locally, and share selected content through revocable web links.

**Approved design:** Notes / Transcript tabs; local numbered speakers with editable names; optional transcript sharing; anyone with a link can read; stop sharing revokes it. Old recordings without audio retain source labels.

**Architecture:** Preserve the audio-source enum and add optional speaker identity to transcript segments. Run a meeting-scoped local diarizer and align ASR words to its timeline. Persist speaker names with segments. Host a minimal read-only notes renderer and upload/revoke API on Vercel with private Blob storage. Authenticate writes with a locally provisioned Keychain credential; do not embed service credentials in source or the app binary. Store per-meeting share handles before network writes so interruptions remain recoverable.

- [x] Extend transcript model, merge/render, and tests for legacy decoding, distinct speakers, and renaming.
- [x] Integrate FluidAudio speaker detection and word timing; preserve transcription on detection failure.
- [x] Add visible Notes / Transcript tabs and speaker editing.
- [x] Implement and test sharing validation, safe rendering, authenticated writes and revocation.
- [x] Service deployed and share UI connected. User approved dedicated credential provisioning; Keychain and sensitive Vercel variables configured, service redeployed, and synthetic live sharing lifecycle verified.
- [x] Run Swift tests/build and web tests; document operational limitations. 59 Swift tests and 5 web tests passed; release app bundled. Live authenticated publish, anonymous read, update, transcript removal, and revocation verified using synthetic content (deleted afterward). Real-audio model quality remains untested.

Existing unrelated changes in MinutesApp.swift, SettingsView.swift, and install.sh are preserved.
