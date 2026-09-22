import Foundation

public struct MeetingShare: Codable, Equatable, Sendable {
    public let id: String
    public let includesTranscript: Bool
    public let url: URL

    public init(id: String, includesTranscript: Bool, url: URL) {
        self.id = id; self.includesTranscript = includesTranscript; self.url = url
    }
}

extension MeetingStore {
    public func share(for id: UUID) throws -> MeetingShare? {
        let file = folder(for: id).appendingPathComponent("share.json")
        guard FileManager.default.fileExists(atPath: file.path) else { return nil }
        return try JSONDecoder().decode(MeetingShare.self, from: Data(contentsOf: file))
    }

    public func saveShare(_ share: MeetingShare, for id: UUID) throws {
        try JSONEncoder().encode(share).write(to: folder(for: id).appendingPathComponent("share.json"), options: .atomic)
    }

    public func removeShare(for id: UUID) throws {
        let file = folder(for: id).appendingPathComponent("share.json")
        if FileManager.default.fileExists(atPath: file.path) { try FileManager.default.removeItem(at: file) }
    }

    public func renameSpeaker(_ key: String, to name: String, for id: UUID) throws {
        let name = String(name.trimmingCharacters(in: .whitespacesAndNewlines).prefix(100))
        let segments = try transcript(for: id).map { segment in
            TranscriptSegment(speaker: segment.speaker, start: segment.start, end: segment.end, text: segment.text,
                              speakerID: segment.speakerID,
                              speakerName: segment.speakerKey == key ? (name.isEmpty ? nil : name) : segment.speakerName)
        }
        try saveTranscript(segments, for: id)
    }
}
