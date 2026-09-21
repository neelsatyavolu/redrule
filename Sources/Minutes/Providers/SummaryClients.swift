import Foundation
import MinutesCore

/// A provider and model choice, stored as "codex:gpt-6-astra".
struct ModelChoice: Hashable, Identifiable {
    let provider: ProviderID
    let model: String
    let label: String

    var id: String { "\(provider.rawValue):\(model)" }

    /// Newest first within each provider; the first entry is that provider's default.
    static let all: [ModelChoice] = [
        ModelChoice(provider: .codex, model: "gpt-6-astra", label: "GPT-6 Astra"),
        ModelChoice(provider: .codex, model: "gpt-5.6-sol", label: "GPT-5.6 Sol"),
        ModelChoice(provider: .codex, model: "gpt-5.6-terra", label: "GPT-5.6 Terra"),
        ModelChoice(provider: .codex, model: "gpt-5.6-luna", label: "GPT-5.6 Luna"),
        ModelChoice(provider: .grok, model: "grok-4.7", label: "Grok 4.7"),
        ModelChoice(provider: .grok, model: "grok-4.6", label: "Grok 4.6"),
        ModelChoice(provider: .grok, model: "grok-4.5", label: "Grok 4.5"),
    ]

    /// A saved choice that is no longer offered (a retired model) falls back to the same provider's default.
    static func resolve(_ id: String?) -> ModelChoice {
        if let match = all.first(where: { $0.id == id }) { return match }
        let provider = id.flatMap { ProviderID(rawValue: String($0.prefix { $0 != ":" })) } ?? .codex
        return defaultChoice(for: provider)
    }

    static func defaultChoice(for provider: ProviderID) -> ModelChoice {
        all.first { $0.provider == provider }!
    }
}

private enum Endpoint {
    static let codex = URL(string: "https://chatgpt.com/backend-api/codex/responses")!
    static let grok = URL(string: "https://api.x.ai/v1/chat/completions")!
    static let timeout: TimeInterval = 240
}

private func send(_ request: URLRequest, provider: ProviderID) async throws -> Data {
    let (data, response) = try await URLSession.shared.data(for: request)
    guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
        let detail = String(decoding: data.prefix(300), as: UTF8.self)
        throw SummaryError.provider("\(provider.displayName) returned an error (\(status)). \(detail)")
    }
    return data
}

struct CodexClient: SummaryProvider {
    let oauth: OAuthService
    let model: String

    func complete(system: String, user: String, jsonSchema: [String: any Sendable]?) async throws -> String {
        let tokens = try await oauth.activeTokens(.codex)
        var body: [String: Any] = [
            "model": model,
            "instructions": system,
            "input": [["role": "user", "content": [["type": "input_text", "text": user]]]],
            "reasoning": ["effort": "low"],
            "store": false,
            "stream": true,
        ]
        if let jsonSchema {
            body["text"] = ["format": ["type": "json_schema", "name": "meeting_note", "strict": true, "schema": jsonSchema]]
        }

        var request = URLRequest(url: Endpoint.codex, timeoutInterval: Endpoint.timeout)
        request.httpMethod = "POST"
        request.httpBody = try JSONSerialization.data(withJSONObject: body)
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("text/event-stream, application/json", forHTTPHeaderField: "Accept")
        request.setValue("Bearer \(tokens.accessToken)", forHTTPHeaderField: "Authorization")
        request.setValue("codex_cli_rs", forHTTPHeaderField: "originator")
        request.setValue("responses=v1", forHTTPHeaderField: "OpenAI-Beta")
        if let account = tokens.accountID { request.setValue(account, forHTTPHeaderField: "chatgpt-account-id") }

        let data = try await send(request, provider: .codex)
        return try CodexStreamParser.outputText(from: String(decoding: data, as: UTF8.self))
    }
}

struct GrokClient: SummaryProvider {
    let oauth: OAuthService
    let model: String

    func complete(system: String, user: String, jsonSchema: [String: any Sendable]?) async throws -> String {
        let tokens = try await oauth.activeTokens(.grok)
        let body: [String: Any] = [
            "model": model,
            "messages": [["role": "system", "content": system], ["role": "user", "content": user]],
            "temperature": 0.3,
        ]
        var request = URLRequest(url: Endpoint.grok, timeoutInterval: Endpoint.timeout)
        request.httpMethod = "POST"
        request.httpBody = try JSONSerialization.data(withJSONObject: body)
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(tokens.accessToken)", forHTTPHeaderField: "Authorization")

        let data = try await send(request, provider: .grok)
        let json = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        let message = (json?["choices"] as? [[String: Any]])?.first?["message"] as? [String: Any]
        guard let text = message?["content"] as? String else {
            throw SummaryError.provider("Grok returned a reply with no text.")
        }
        return text
    }
}
