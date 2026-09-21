import FluidAudio
import Foundation
import MinutesCore

protocol Transcriber: Sendable {
    /// Downloads (first run) and loads the model. Safe to call repeatedly.
    func prepare() async throws
    func transcribe(_ window: AudioWindow) async throws -> String
}

/// Parakeet TDT v3 running on the Neural Engine through FluidAudio.
actor ParakeetTranscriber: Transcriber {
    private var loading: Task<AsrManager, Error>?

    func prepare() async throws {
        _ = try await manager()
    }

    func transcribe(_ window: AudioWindow) async throws -> String {
        let manager = try await manager()
        // Each window is independent speech, so it gets a fresh decoder state.
        var state = TdtDecoderState.make()
        let result = try await manager.transcribe(window.samples, decoderState: &state)
        return result.text.trimmingCharacters(in: .whitespacesAndNewlines)
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
