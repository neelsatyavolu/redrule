import FluidAudio
import Foundation
import MinutesCore

struct Transcription: Sendable {
    let text: String
    let words: [TranscriptWord]
}

protocol Transcriber: Sendable {
    /// Downloads (first run) and loads the model. Safe to call repeatedly.
    func prepare() async throws
    func transcribe(_ window: AudioWindow) async throws -> Transcription
}

/// Parakeet TDT v3 running on the Neural Engine through FluidAudio.
actor ParakeetTranscriber: Transcriber {
    private var loading: Task<AsrManager, Error>?

    func prepare() async throws {
        _ = try await manager()
    }

    func transcribe(_ window: AudioWindow) async throws -> Transcription {
        let manager = try await manager()
        // Each window is independent speech, so it gets a fresh decoder state.
        var state = TdtDecoderState.make()
        let result = try await manager.transcribe(window.samples, decoderState: &state)
        return Transcription(text: result.text.trimmingCharacters(in: .whitespacesAndNewlines),
                             words: buildWordTimings(from: result.tokenTimings ?? []).map {
                                 TranscriptWord(text: $0.word, start: $0.startTime, end: $0.endTime)
                             })
    }

    private func manager() async throws -> AsrManager {
        if let loading { return try await loading.value }
        let task = Task {
            let models = try await AsrModels.downloadAndLoad(version: .v3)
            let manager = AsrManager(config: .default)
            try await manager.loadModels(models)
            return manager
        }
        loading = task
        do {
            return try await task.value
        } catch {
            loading = nil // allow a retry after a failed download
            throw error
        }
    }
}
