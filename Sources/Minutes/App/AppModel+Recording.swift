import Foundation
import MinutesCore

extension AppModel {
    var isRecording: Bool { recording != nil }

    func startRecording(app: MeetingApp) {
        guard recording == nil, let store else { return }
        banner = nil
        refreshPermissions()
        guard permissions.allGranted else {
            errorMessage = "Minutes needs Microphone and Screen & System Audio Recording access before it can record. Grant them in Settings, under Permissions."
            return
        }

        let title = app == .manual ? "New meeting" : "\(app.displayName) meeting"
        let meeting = persist(Meeting(id: UUID(), title: title, app: app, startedAt: Date(), endedAt: nil, status: .recording, errorMessage: nil))
        recording = meeting
        liveSegments = []
        selection = meeting.id

        let pipeline = RecordingPipeline(transcriber: transcriber, audioFolder: keepAudio ? store.folder(for: meeting.id) : nil)
        self.pipeline = pipeline
        rawSegments = []
        startTask = Task {
            do {
                try await pipeline.start(
                    onSegment: { [weak self] segment in Task { @MainActor in self?.append(segment, to: meeting.id) } },
                    onError: { [weak self] error in Task { @MainActor in self?.errorMessage = "Part of the audio could not be processed. \(error.localizedDescription)" } }
                )
                return true
            } catch {
                self.pipeline = nil
                recording = nil
                persist(meeting.with(endedAt: Date(), status: .failed, errorMessage: "Recording could not start. \(error.localizedDescription)"))
                return false
            }
        }
    }

    func stopRecording() {
        guard let meeting = recording, let pipeline, let startTask else { return }
        banner = nil
        recording = nil
        self.pipeline = nil
        self.startTask = nil
        let ended = persist(meeting.with(endedAt: Date(), status: .transcribing))
        Task {
            // Stopping before capture has finished starting would leave it running with no owner.
            guard await startTask.value else { return }
            // The pipeline's own list is authoritative: live updates reach the main actor asynchronously.
            let segments = await pipeline.stop()
            perform("The transcript could not be saved.") { try $0.saveTranscript(segments, for: ended.id) }
            await generateNotes(for: ended)
        }
    }

    /// Writes notes from the stored transcript. Also used to retry after a failure.
    func generateNotes(for meeting: Meeting) async {
        guard let store else { return }
        let working = persist(meeting.with(status: .summarizing))
        do {
            let segments = try store.transcript(for: meeting.id)
            let provider = try await summaryProvider()
            let note = try await Summarizer(provider: provider).summarize(meeting: working, segments: segments)
            try store.saveNote(note, for: meeting.id)
            persist(working.with(title: note.title, status: .done))
        } catch {
            persist(working.with(status: .failed, errorMessage: error.localizedDescription))
        }
        if selection == meeting.id { loadSelection() }
    }

    private func append(_ segment: TranscriptSegment, to id: UUID) {
        // After stop, the pipeline's returned list is saved instead; a late update must not overwrite it.
        guard recording?.id == id else { return }
        liveSegments = TranscriptMerger.merge(liveSegments + [segment])
        // Saved as the meeting runs so a crash keeps what was said so far.
        rawSegments = rawSegments + [segment]
        let snapshot = rawSegments
        perform("The transcript could not be saved.") { try $0.saveTranscript(snapshot, for: id) }
    }

    private func summaryProvider() async throws -> any SummaryProvider {
        await refreshConnections()
        let preferred = ModelChoice.all.first { $0.id == modelChoiceID }
        let choice = preferred.flatMap { connected.contains($0.provider) ? $0 : nil }
            ?? connected.sorted { $0.rawValue < $1.rawValue }.first.map(ModelChoice.defaultChoice)
        guard let choice else { throw SummaryError.noProviderConnected }
        switch choice.provider {
        case .codex: return CodexClient(oauth: oauth, model: choice.model)
        case .grok: return GrokClient(oauth: oauth, model: choice.model)
        }
    }
}
