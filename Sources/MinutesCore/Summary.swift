import Foundation

public enum SummaryError: Error, Equatable, LocalizedError {
    case notJSON
    case provider(String)
    case emptyTranscript
    case noProviderConnected

    public var errorDescription: String? {
        switch self {
        case .notJSON: "The model did not return notes in the expected format."
        case .provider(let message): message
        case .emptyTranscript: "Nothing was transcribed, so there is nothing to summarise."
        case .noProviderConnected: "Connect ChatGPT or Grok in Settings to generate notes."
        }
    }
}

public enum SummaryPrompt {
    /// Transcripts longer than this are digested chunk by chunk before the final note is written.
    public static let chunkBudget = 60_000

    public static let system = """
    You write meeting notes from a transcript, in the style of a sharp chief of staff.
    "Me" is the person who recorded the meeting. Numbered speaker labels are estimated voice identities, not real names. "Them" is unassigned call audio, possibly several people. Named labels were supplied by the user. Do not infer real names from numbered labels.
    Rules:
    - Report only what was said. Never invent names, numbers, dates or commitments.
    - The transcript comes from speech recognition: silently fix obvious mis-hearings, ignore filler and small talk.
    - title: 3-7 words naming the meeting's subject, no date.
    - tldr: two or three sentences a colleague who missed the meeting could act on.
    - sections: 2-6 topic sections in the order discussed, each with concise, specific bullets.
    - decisions: things actually agreed. Empty list if none.
    - action_items: concrete follow-ups. owner is a name if one was stated, "Me" for the recorder, otherwise an empty string.
    Reply with a single JSON object and nothing else, with exactly these keys:
    {"title": string, "tldr": string, "sections": [{"heading": string, "bullets": [string]}], "decisions": [string], "action_items": [{"owner": string, "task": string}]}
    """

    public static let digestSystem = """
    You are condensing one part of a long meeting transcript so that notes can be written later from your digest.
    Keep every decision, number, date, name, commitment and open question. Drop filler. Keep the order of discussion.
    Reply with plain text bullets only.
    """

    public static func user(transcript: String, app: MeetingApp, startedAt: Date) -> String {
        "Meeting on \(app.displayName), started \(startedAt.formatted(date: .abbreviated, time: .shortened)).\n\nTranscript:\n\(transcript)"
    }

    public static func userFromDigests(_ digests: [String], app: MeetingApp, startedAt: Date) -> String {
        let body = digests.enumerated().map { "Part \($0.offset + 1):\n\($0.element)" }.joined(separator: "\n\n")
        return "Meeting on \(app.displayName), started \(startedAt.formatted(date: .abbreviated, time: .shortened)).\n\nDigests of the transcript, in order:\n\(body)"
    }

    /// Strict JSON schema for providers that support structured output.
    public static var schema: [String: any Sendable] {
        let string: [String: any Sendable] = ["type": "string"]
        let strings: [String: any Sendable] = ["type": "array", "items": string]
        func object(_ properties: [String: any Sendable]) -> [String: any Sendable] {
            ["type": "object", "properties": properties, "required": properties.keys.sorted(), "additionalProperties": false]
        }
        func array(of item: [String: any Sendable]) -> [String: any Sendable] {
            ["type": "array", "items": item]
        }
        return object([
            "title": string,
            "tldr": string,
            "sections": array(of: object(["heading": string, "bullets": strings])),
            "decisions": strings,
            "action_items": array(of: object(["owner": string, "task": string])),
        ])
    }
}

public enum SummaryParser {
    private struct Payload: Decodable {
        struct Item: Decodable {
            let owner: String?
            let task: String
        }
        let title: String
        let tldr: String
        let sections: [NoteSection]?
        let decisions: [String]?
        let action_items: [Item]?
    }

    /// Parses the model reply, tolerating code fences or prose around the JSON object.
    public static func parse(_ reply: String) throws -> MeetingNote {
        guard let start = reply.firstIndex(of: "{"), let end = reply.lastIndex(of: "}"), start < end,
              let payload = try? JSONDecoder().decode(Payload.self, from: Data(reply[start...end].utf8))
        else { throw SummaryError.notJSON }

        let items = (payload.action_items ?? []).map { item in
            let owner = item.owner?.trimmingCharacters(in: .whitespaces)
            return ActionItem(owner: owner?.isEmpty == false ? owner : nil, task: item.task)
        }
        return MeetingNote(
            title: payload.title, tldr: payload.tldr, sections: payload.sections ?? [],
            decisions: payload.decisions ?? [], actionItems: items
        )
    }
}

public enum CodexStreamParser {
    /// Extracts the output text from a buffered Responses API server-sent-event body.
    public static func outputText(from body: String) throws -> String {
        var text = ""
        for line in body.components(separatedBy: .newlines) where line.hasPrefix("data: ") {
            let data = line.dropFirst(6).trimmingCharacters(in: .whitespaces)
            guard data != "[DONE]", let event = try? JSONSerialization.jsonObject(with: Data(data.utf8)) as? [String: Any] else { continue }
            let response = event["response"] as? [String: Any]
            if let error = (event["error"] as? [String: Any] ?? response?["error"] as? [String: Any])?["message"] as? String {
                throw SummaryError.provider(error)
            }
            if let delta = event["delta"] as? String { text += delta }
            if event["type"] as? String == "response.output_text.done", let done = event["text"] as? String { text = done }
        }
        return text
    }
}
