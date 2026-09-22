import Foundation

public enum TranscriptMerger {
    static let joinGap: TimeInterval = 2
    static let echoSimilarity = 0.6

    /// Orders segments by time, drops mic segments that merely echo the system audio,
    /// and joins neighbouring segments from the same speaker.
    public static func merge(_ segments: [TranscriptSegment]) -> [TranscriptSegment] {
        let cleaned = segments
            .map { TranscriptSegment(speaker: $0.speaker, start: $0.start, end: $0.end, text: $0.text.trimmingCharacters(in: .whitespacesAndNewlines), speakerID: $0.speakerID, speakerName: $0.speakerName) }
            .filter { !$0.text.isEmpty }
        let remote = cleaned.filter { $0.speaker == .them }
        let ordered = cleaned
            .filter { segment in segment.speaker == .them || !remote.contains { isEcho(segment, of: $0) } }
            .sorted { ($0.start, $0.speaker.rawValue) < ($1.start, $1.speaker.rawValue) }

        return ordered.reduce(into: [TranscriptSegment]()) { result, segment in
            if let last = result.last, last.speakerKey == segment.speakerKey, segment.start - last.end <= joinGap {
                result[result.count - 1] = TranscriptSegment(
                    speaker: last.speaker, start: last.start, end: max(last.end, segment.end), text: last.text + " " + segment.text, speakerID: last.speakerID, speakerName: last.speakerName
                )
            } else {
                result.append(segment)
            }
        }
    }

    public static func render(_ segments: [TranscriptSegment]) -> String {
        segments.map { "[\(timestamp($0.start))] \($0.speakerLabel): \($0.text)" }.joined(separator: "\n")
    }

    public static func timestamp(_ seconds: TimeInterval) -> String {
        let total = Int(seconds)
        let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60)
        return h > 0 ? String(format: "%d:%02d:%02d", h, m, s) : String(format: "%02d:%02d", m, s)
    }

    static func isEcho(_ mic: TranscriptSegment, of system: TranscriptSegment) -> Bool {
        guard mic.start < system.end, system.start < mic.end else { return false }
        let a = words(mic.text), b = words(system.text)
        guard !a.isEmpty, !b.isEmpty else { return false }
        return Double(a.intersection(b).count) / Double(a.union(b).count) >= echoSimilarity
    }

    private static func words(_ text: String) -> Set<String> {
        Set(text.lowercased().components(separatedBy: CharacterSet.alphanumerics.inverted).filter { !$0.isEmpty })
    }
}

public enum TranscriptChunker {
    /// Splits text on line boundaries into chunks of at most `budget` characters.
    /// A single line longer than the budget becomes its own chunk.
    public static func chunks(_ text: String, budget: Int) -> [String] {
        let lines = text.components(separatedBy: "\n")
        let grouped = lines.reduce(into: [[String]]()) { groups, line in
            let currentSize = groups.last.map { $0.reduce(0) { $0 + $1.count + 1 } } ?? 0
            if groups.isEmpty || currentSize + line.count > budget {
                groups.append([line])
            } else {
                groups[groups.count - 1].append(line)
            }
        }
        return grouped.map { $0.joined(separator: "\n") }
    }
}
