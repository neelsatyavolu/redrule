import Foundation

public protocol SummaryProvider: Sendable {
    /// Sends one prompt and returns the model's text. `jsonSchema` is a hint for providers with structured output.
    func complete(system: String, user: String, jsonSchema: [String: any Sendable]?) async throws -> String
}

public struct Summarizer: Sendable {
    let provider: any SummaryProvider
    let chunkBudget: Int

    public init(provider: any SummaryProvider, chunkBudget: Int = SummaryPrompt.chunkBudget) {
        self.provider = provider
        self.chunkBudget = chunkBudget
    }

    public func summarize(meeting: Meeting, segments: [TranscriptSegment]) async throws -> MeetingNote {
        let transcript = TranscriptMerger.render(TranscriptMerger.merge(segments))
        guard !transcript.isEmpty else { throw SummaryError.emptyTranscript }

        let chunks = TranscriptChunker.chunks(transcript, budget: chunkBudget)
        let user: String
        if chunks.count == 1 {
            user = SummaryPrompt.user(transcript: transcript, app: meeting.app, startedAt: meeting.startedAt)
        } else {
            var digests: [String] = []
            for chunk in chunks {
                digests.append(try await provider.complete(system: SummaryPrompt.digestSystem, user: chunk, jsonSchema: nil))
            }
            user = SummaryPrompt.userFromDigests(digests, app: meeting.app, startedAt: meeting.startedAt)
        }

        let schema = SummaryPrompt.schema
        let first = try await provider.complete(system: SummaryPrompt.system, user: user, jsonSchema: schema)
        if let note = try? SummaryParser.parse(first) { return note }
        let second = try await provider.complete(system: SummaryPrompt.system, user: user, jsonSchema: schema)
        if let note = try? SummaryParser.parse(second) { return note }
        // Keep whatever the model wrote rather than losing the meeting.
        return MeetingNote(title: meeting.title, tldr: second.trimmingCharacters(in: .whitespacesAndNewlines), sections: [], decisions: [], actionItems: [])
    }
}
