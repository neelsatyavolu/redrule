import Foundation
import MinutesCore
import Security

struct ShareClient {
    static let baseURL = URL(string: "https://redrule.n3el.dev")!

    private struct Payload: Encodable {
        struct Segment: Encodable {
            let speaker: String
            let time: String
            let text: String
        }
        let note: MeetingNote
        let transcript: [Segment]?
    }

    func publish(_ share: MeetingShare, note: MeetingNote, transcript: [TranscriptSegment]) async throws {
        let payload = Payload(note: note, transcript: share.includesTranscript ? transcript.map {
            Payload.Segment(speaker: $0.speakerLabel, time: TranscriptMerger.timestamp($0.start), text: $0.text)
        } : nil)
        let data = try JSONEncoder().encode(payload)
        guard data.count <= 2_000_000 else { throw ShareError.message("This meeting is too large to share.") }
        try await request(share: share, method: "PUT", body: data)
    }

    func revoke(_ share: MeetingShare) async throws {
        try await request(share: share, method: "DELETE", body: nil)
    }

    private func request(share: MeetingShare, method: String, body: Data?) async throws {
        // Always use our configured host; stored links cannot redirect upload credentials.
        var components = URLComponents(url: Self.baseURL.appendingPathComponent("api/share"), resolvingAgainstBaseURL: false)!
        components.queryItems = [URLQueryItem(name: "id", value: share.id)]
        var request = URLRequest(url: components.url!)
        request.httpMethod = method
        request.timeoutInterval = 45
        request.setValue("Bearer \(try credential())", forHTTPHeaderField: "Authorization")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = body
        let (data, response) = try await URLSession.shared.data(for: request)
        guard let response = response as? HTTPURLResponse, (200..<300).contains(response.statusCode) else {
            let error = try? JSONDecoder().decode(ServiceError.self, from: data)
            throw ShareError.message(error?.error ?? "Sharing could not complete. Check your connection and try again.")
        }
    }

    private struct ServiceError: Decodable { let error: String }

    private func credential() throws -> String {
        for service in ["Redrule", "Minutes"] {
            let query: [String: Any] = [kSecClass as String: kSecClassGenericPassword,
                                       kSecAttrService as String: service, kSecAttrAccount as String: "sharing",
                                       kSecReturnData as String: true, kSecMatchLimit as String: kSecMatchLimitOne]
            var result: CFTypeRef?
            let status = SecItemCopyMatching(query as CFDictionary, &result)
            if status == errSecItemNotFound { continue }
            guard status == errSecSuccess, let data = result as? Data, let key = String(data: data, encoding: .utf8) else {
                throw KeychainError.status(status)
            }
            return key
        }
        throw ShareError.message("Sharing is not configured on this Mac. Run the sharing setup script from the Redrule project.")
    }
}

enum ShareError: LocalizedError {
    case message(String)
    var errorDescription: String? { if case .message(let message) = self { return message }; return nil }
}
