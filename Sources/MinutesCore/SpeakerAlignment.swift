import Foundation

public struct TranscriptWord: Sendable {
    public let text: String
    public let start: TimeInterval
    public let end: TimeInterval

    public init(text: String, start: TimeInterval, end: TimeInterval) {
        self.text = text; self.start = start; self.end = end
    }
}

public struct SpeakerTurn: Sendable {
    public let id: String
    public let start: TimeInterval
    public let end: TimeInterval

    public init(id: String, start: TimeInterval, end: TimeInterval) {
        self.id = id; self.start = start; self.end = end
    }
}

public enum SpeakerAlignment {
    /// Assign by the midpoint of each word. Overlap and uncovered speech remain unassigned.
    public static func segments(words: [TranscriptWord], turns: [SpeakerTurn], offset: TimeInterval) -> [TranscriptSegment] {
        let segments = words.map { word in
            let midpoint = (word.start + word.end) / 2
            let speakers = Set(turns.filter { $0.start <= midpoint && midpoint < $0.end }.map(\.id))
            return TranscriptSegment(speaker: .them, start: offset + word.start, end: offset + word.end,
                                     text: word.text, speakerID: speakers.count == 1 ? speakers.first : nil)
        }
        return TranscriptMerger.merge(segments)
    }
}
