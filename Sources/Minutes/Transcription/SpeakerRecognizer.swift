import FluidAudio
import Foundation
import MinutesCore

/// One recognizer per recording: voice identities are never carried across meetings.
actor SpeakerRecognizer {
    private var manager: DiarizerManager?
    private var labels: [String: String] = [:]
    private var failed = false

    func turns(in window: AudioWindow) async throws -> [SpeakerTurn] {
        guard !failed else { return [] }
        do {
            if manager == nil {
                let models = try await DiarizerModels.downloadIfNeeded()
                let manager = DiarizerManager()
                manager.initialize(models: models)
                self.manager = manager
            }
            guard let manager else { return [] }
            let result = try manager.performCompleteDiarization(window.samples, atTime: window.start)
            return result.segments.map { segment in
                if labels[segment.speakerId] == nil { labels[segment.speakerId] = String(labels.count + 1) }
                return SpeakerTurn(id: labels[segment.speakerId]!,
                                   start: Double(segment.startTimeSeconds) - window.start,
                                   end: Double(segment.endTimeSeconds) - window.start)
            }
        } catch {
            failed = true
            throw error
        }
    }
}
