import Foundation

/// File-backed storage: one folder per meeting holding `meeting.json`, `transcript.json`, `note.json`, `notes.md`.
public struct MeetingStore: Sendable {
    public let root: URL

    public init(root: URL) throws {
        self.root = root
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    }

    public static func defaultRoot() throws -> URL {
        try FileManager.default
            .url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
            .appendingPathComponent("Minutes/meetings", isDirectory: true)
    }

    public func folder(for id: UUID) -> URL {
        root.appendingPathComponent(id.uuidString, isDirectory: true)
    }

    /// All readable meetings, newest first. Folders that fail to decode are skipped.
    public func list() throws -> [Meeting] {
        try FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: nil)
            .compactMap { try? read(Meeting.self, from: $0.appendingPathComponent("meeting.json")) }
            .sorted { $0.startedAt > $1.startedAt }
    }

    public func save(_ meeting: Meeting) throws {
        try FileManager.default.createDirectory(at: folder(for: meeting.id), withIntermediateDirectories: true)
        try write(meeting, to: folder(for: meeting.id).appendingPathComponent("meeting.json"))
    }

    public func transcript(for id: UUID) throws -> [TranscriptSegment] {
        let url = folder(for: id).appendingPathComponent("transcript.json")
        guard FileManager.default.fileExists(atPath: url.path) else { return [] }
        return try read([TranscriptSegment].self, from: url)
    }

    public func saveTranscript(_ segments: [TranscriptSegment], for id: UUID) throws {
        try write(segments, to: folder(for: id).appendingPathComponent("transcript.json"))
    }

    public func note(for id: UUID) throws -> MeetingNote? {
        let url = folder(for: id).appendingPathComponent("note.json")
        guard FileManager.default.fileExists(atPath: url.path) else { return nil }
        return try read(MeetingNote.self, from: url)
    }

    public func saveNote(_ note: MeetingNote, for id: UUID) throws {
        try write(note, to: folder(for: id).appendingPathComponent("note.json"))
        try Data(note.markdown.utf8).write(to: folder(for: id).appendingPathComponent("notes.md"), options: .atomic)
    }

    public func delete(_ id: UUID) throws {
        try FileManager.default.removeItem(at: folder(for: id))
    }

    /// Meetings left mid-flight by a crash or quit can never finish; mark them so the UI offers a retry.
    public func recoverInterrupted() throws {
        for meeting in try list() where meeting.status != .done && meeting.status != .failed {
            try save(meeting.with(endedAt: meeting.endedAt ?? meeting.startedAt, status: .failed, errorMessage: "Minutes quit before this meeting was finished."))
        }
    }

    private func read<T: Decodable>(_ type: T.Type, from url: URL) throws -> T {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return try decoder.decode(type, from: Data(contentsOf: url))
    }

    private func write<T: Encodable>(_ value: T, to url: URL) throws {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        try encoder.encode(value).write(to: url, options: .atomic)
    }
}
