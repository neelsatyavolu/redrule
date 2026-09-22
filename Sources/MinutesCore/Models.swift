import Foundation

public enum Speaker: String, Codable, Sendable {
    case me, them

    public var label: String { self == .me ? "Me" : "Them" }
}

public struct TranscriptSegment: Codable, Equatable, Sendable, Identifiable {
    public let speaker: Speaker
    public let start: TimeInterval
    public let end: TimeInterval
    public let text: String

    public let speakerID: String?
    public let speakerName: String?

    public var speakerKey: String { "\(speaker.rawValue):\(speakerID ?? "source")" }
    public var speakerLabel: String { speakerName ?? speakerID.map { "Speaker \($0)" } ?? speaker.label }
    public var id: String { "\(speakerKey)-\(start)" }

    public init(speaker: Speaker, start: TimeInterval, end: TimeInterval, text: String, speakerID: String? = nil, speakerName: String? = nil) {
        self.speaker = speaker
        self.start = start
        self.end = end
        self.text = text
        self.speakerID = speakerID
        self.speakerName = speakerName
    }
}

public enum MeetingApp: String, Codable, Sendable {
    case zoom, googleMeet, manual

    public var displayName: String {
        switch self {
        case .zoom: "Zoom"
        case .googleMeet: "Google Meet"
        case .manual: "Recording"
        }
    }
}

public enum MeetingStatus: String, Codable, Sendable {
    case recording, transcribing, summarizing, done, failed
}

public struct Meeting: Codable, Equatable, Sendable, Identifiable {
    public let id: UUID
    public let title: String
    public let app: MeetingApp
    public let startedAt: Date
    public let endedAt: Date?
    public let status: MeetingStatus
    public let errorMessage: String?
    public let archivedAt: Date?

    public var isArchived: Bool { archivedAt != nil }

    public init(id: UUID, title: String, app: MeetingApp, startedAt: Date, endedAt: Date?, status: MeetingStatus, errorMessage: String?, archivedAt: Date? = nil) {
        self.id = id
        self.title = title
        self.app = app
        self.startedAt = startedAt
        self.endedAt = endedAt
        self.status = status
        self.errorMessage = errorMessage
        self.archivedAt = archivedAt
    }

    /// Returns a copy with the given fields replaced. `errorMessage` is cleared unless passed.
    public func with(title: String? = nil, endedAt: Date? = nil, status: MeetingStatus? = nil, errorMessage: String? = nil, isArchived: Bool? = nil) -> Meeting {
        Meeting(
            id: id, title: title ?? self.title, app: app, startedAt: startedAt,
            endedAt: endedAt ?? self.endedAt, status: status ?? self.status, errorMessage: errorMessage,
            archivedAt: isArchived == false ? nil : (isArchived == true ? (archivedAt ?? Date()) : archivedAt)
        )
    }

    public var duration: TimeInterval? { endedAt.map { $0.timeIntervalSince(startedAt) } }
}

public struct ActionItem: Codable, Equatable, Sendable {
    public let owner: String?
    public let task: String

    public init(owner: String?, task: String) {
        self.owner = owner
        self.task = task
    }
}

public struct NoteSection: Codable, Equatable, Sendable {
    public let heading: String
    public let bullets: [String]

    public init(heading: String, bullets: [String]) {
        self.heading = heading
        self.bullets = bullets
    }
}

public struct MeetingNote: Codable, Equatable, Sendable {
    public let title: String
    public let tldr: String
    public let sections: [NoteSection]
    public let decisions: [String]
    public let actionItems: [ActionItem]

    public init(title: String, tldr: String, sections: [NoteSection], decisions: [String], actionItems: [ActionItem]) {
        self.title = title
        self.tldr = tldr
        self.sections = sections
        self.decisions = decisions
        self.actionItems = actionItems
    }

    public var markdown: String {
        var blocks = ["# \(title)", tldr]
        blocks += sections.map { section in
            (["## \(section.heading)"] + section.bullets.map { "- \($0)" }).joined(separator: "\n")
        }
        if !decisions.isEmpty {
            blocks.append((["## Decisions"] + decisions.map { "- \($0)" }).joined(separator: "\n"))
        }
        if !actionItems.isEmpty {
            let lines = actionItems.map { item in
                item.owner.map { "- [ ] **\($0)** — \(item.task)" } ?? "- [ ] \(item.task)"
            }
            blocks.append((["## Action items"] + lines).joined(separator: "\n"))
        }
        return blocks.joined(separator: "\n\n") + "\n"
    }
}
