import AppKit
import MinutesCore

extension AppModel {
    func share(for meeting: Meeting) -> MeetingShare? { try? store?.share(for: meeting.id) }

    func publishShare(for meeting: Meeting, includeTranscript: Bool) async -> URL? {
        guard let store, !sharingBusy else { return nil }
        sharingBusy = true
        defer { sharingBusy = false }
        do {
            guard let note = try store.note(for: meeting.id) else { return nil }
            let existing = try store.share(for: meeting.id)
            let id = existing?.id ?? (UUID().uuidString + UUID().uuidString).replacingOccurrences(of: "-", with: "").lowercased()
            let share = MeetingShare(id: id, includesTranscript: includeTranscript, url: ShareClient.baseURL.appendingPathComponent("s/\(id)"))
            // Retain the handle even if the connection drops after a successful upload.
            try store.saveShare(share, for: meeting.id)
            let transcript = TranscriptMerger.merge(try store.transcript(for: meeting.id))
            try await ShareClient().publish(share, note: note, transcript: transcript)
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(share.url.absoluteString, forType: .string)
            return share.url
        } catch { errorMessage = error.localizedDescription; return nil }
    }

    func revokeShare(for meeting: Meeting) async -> Bool {
        guard let store, !sharingBusy else { return false }
        sharingBusy = true
        defer { sharingBusy = false }
        do {
            if let share = try store.share(for: meeting.id) { try await ShareClient().revoke(share) }
            try store.removeShare(for: meeting.id)
            return true
        } catch { errorMessage = error.localizedDescription; return false }
    }

    func renameSpeaker(_ key: String, to name: String, in meeting: Meeting) {
        perform("The speaker name could not be saved.") { try $0.renameSpeaker(key, to: name, for: meeting.id) }
        if selection == meeting.id { loadSelection() }
    }
}
