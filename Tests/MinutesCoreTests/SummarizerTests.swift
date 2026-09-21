import Foundation
import Testing
@testable import MinutesCore

private actor FakeProvider: SummaryProvider {
    private var replies: [String]
    private(set) var calls: [(system: String, user: String)] = []

    init(replies: [String]) { self.replies = replies }

    func complete(system: String, user: String, jsonSchema: [String: any Sendable]?) async throws -> String {
        calls.append((system, user))
        return replies.isEmpty ? "" : replies.removeFirst()
    }
}

@Suite struct SummarizerTests {
    let good = #"{"title":"T","tldr":"S","sections":[],"decisions":[],"action_items":[]}"#
    let meeting = Meeting(id: UUID(), title: "Zoom meeting", app: .zoom, startedAt: Date(timeIntervalSince1970: 0), endedAt: nil, status: .summarizing, errorMessage: nil)
    let segments = [TranscriptSegment(speaker: .me, start: 0, end: 5, text: "let us ship on friday")]

    @Test func singlePassForShortTranscript() async throws {
        let provider = FakeProvider(replies: [good])
        let note = try await Summarizer(provider: provider).summarize(meeting: meeting, segments: segments)
        #expect(note.title == "T")
        let calls = await provider.calls
        #expect(calls.count == 1)
        #expect(calls[0].user.contains("[00:00] Me: let us ship on friday"))
    }

    @Test func emptyTranscriptThrowsWithoutCallingProvider() async {
        let provider = FakeProvider(replies: [good])
        await #expect(throws: SummaryError.emptyTranscript) {
            try await Summarizer(provider: provider).summarize(meeting: meeting, segments: [])
        }
        #expect(await provider.calls.isEmpty)
    }

    @Test func longTranscriptIsDigestedThenSummarized() async throws {
        let long = (0..<10).map { TranscriptSegment(speaker: $0 % 2 == 0 ? .me : .them, start: Double($0) * 10, end: Double($0) * 10 + 5, text: String(repeating: "word ", count: 10)) }
        let provider = FakeProvider(replies: ["digest one", "digest two", good])
        let summarizer = Summarizer(provider: provider, chunkBudget: 400)
        _ = try await summarizer.summarize(meeting: meeting, segments: long)
        let calls = await provider.calls
        #expect(calls.count == 3)
        #expect(calls.dropLast().allSatisfy { $0.system == SummaryPrompt.digestSystem })
        #expect(calls.last?.system == SummaryPrompt.system)
        #expect(calls.last?.user.contains("digest one") == true)
    }

    @Test func retriesOnceWhenReplyIsNotJson() async throws {
        let provider = FakeProvider(replies: ["sorry", good])
        let note = try await Summarizer(provider: provider).summarize(meeting: meeting, segments: segments)
        #expect(note.title == "T")
        #expect(await provider.calls.count == 2)
    }

    @Test func fallsBackToRawTextAfterSecondBadReply() async throws {
        let provider = FakeProvider(replies: ["first prose", "second prose"])
        let note = try await Summarizer(provider: provider).summarize(meeting: meeting, segments: segments)
        #expect(note.title == "Zoom meeting")
        #expect(note.tldr == "second prose")
    }
}
