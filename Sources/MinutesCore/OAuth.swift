import CryptoKit
import Foundation

public enum ProviderID: String, Codable, Sendable, CaseIterable, Identifiable {
    case codex, grok

    public var id: String { rawValue }
    public var displayName: String { self == .codex ? "ChatGPT (Codex)" : "Grok" }
}

public enum OAuthError: Error, Equatable, LocalizedError {
    case emptyInput
    case providerError(String)
    case missingCode
    case stateMismatch
    case badTokenResponse

    public var errorDescription: String? {
        switch self {
        case .emptyInput: "Paste the code or the full redirect URL."
        case .providerError(let message): "Sign-in failed: \(message)"
        case .missingCode: "No authorization code was found in that text."
        case .stateMismatch: "The sign-in response did not match this request. Try connecting again."
        case .badTokenResponse: "The provider returned an unexpected token response."
        }
    }
}

public struct PKCE: Equatable, Sendable {
    public let verifier: String
    public let challenge: String
    public let state: String

    public init(verifier: String, challenge: String, state: String) {
        self.verifier = verifier
        self.challenge = challenge
        self.state = state
    }

    public static func generate() -> PKCE {
        let verifier = randomBase64URL(byteCount: 64)
        return PKCE(verifier: verifier, challenge: challenge(for: verifier), state: randomBase64URL(byteCount: 32))
    }

    public static func challenge(for verifier: String) -> String {
        Data(SHA256.hash(data: Data(verifier.utf8))).base64URLEncodedString()
    }

    private static func randomBase64URL(byteCount: Int) -> String {
        var generator = SystemRandomNumberGenerator()
        return Data((0..<byteCount).map { _ in UInt8.random(in: .min ... .max, using: &generator) }).base64URLEncodedString()
    }
}

/// OAuth client settings, mirroring apexline's `electron/main.cjs`.
public struct OAuthConfig: Sendable {
    public let provider: ProviderID
    public let clientID: String
    public let authorizeEndpoint: URL
    public let tokenEndpoint: URL
    public let redirectURI: String
    public let scope: String
    public let extraAuthorizeParameters: [String: String]
    /// Seconds subtracted from `expires_in` so tokens are treated as expired early.
    public let expirySkew: TimeInterval

    public static let codex = OAuthConfig(
        provider: .codex,
        clientID: "app_EMoamEEZ73f0CkXaXp7hrann",
        authorizeEndpoint: URL(string: "https://auth.openai.com/oauth/authorize")!,
        tokenEndpoint: URL(string: "https://auth.openai.com/oauth/token")!,
        redirectURI: "http://localhost:1455/auth/callback",
        scope: "openid profile email offline_access",
        extraAuthorizeParameters: [
            "id_token_add_organizations": "true",
            "codex_cli_simplified_flow": "true",
            "originator": "codex_cli_rs",
        ],
        expirySkew: 60
    )

    public static let grok = OAuthConfig(
        provider: .grok,
        clientID: "b1a00492-073a-47ea-816f-4c329264a828",
        authorizeEndpoint: URL(string: "https://auth.x.ai/oauth2/authorize")!,
        tokenEndpoint: URL(string: "https://auth.x.ai/oauth2/token")!,
        redirectURI: "http://127.0.0.1:56121/callback",
        scope: "openid profile email offline_access grok-cli:access api:access",
        extraAuthorizeParameters: [:],
        expirySkew: 120
    )

    public static func config(for provider: ProviderID) -> OAuthConfig {
        provider == .codex ? .codex : .grok
    }

    public var callbackPort: UInt16 { UInt16(URLComponents(string: redirectURI)?.port ?? 80) }
    public var callbackPath: String { URLComponents(string: redirectURI)?.path ?? "/" }

    public func authorizeURL(pkce: PKCE) -> URL {
        let base: [(String, String)] = [
            ("response_type", "code"), ("client_id", clientID), ("redirect_uri", redirectURI), ("scope", scope),
            ("code_challenge", pkce.challenge), ("code_challenge_method", "S256"), ("state", pkce.state),
        ]
        let extra = extraAuthorizeParameters.sorted { $0.key < $1.key }.map { ($0.key, $0.value) }
        var components = URLComponents(url: authorizeEndpoint, resolvingAgainstBaseURL: false)!
        components.queryItems = (base + extra).map { URLQueryItem(name: $0.0, value: $0.1) }
        return components.url!
    }

    public func exchangeParameters(code: String, verifier: String) -> [String: String] {
        ["grant_type": "authorization_code", "code": code, "redirect_uri": redirectURI, "client_id": clientID, "code_verifier": verifier]
    }

    public func refreshParameters(refreshToken: String) -> [String: String] {
        ["grant_type": "refresh_token", "refresh_token": refreshToken, "client_id": clientID, "scope": scope]
    }
}

public struct OAuthCallback: Equatable, Sendable {
    public let code: String
    public let state: String?
}

public enum OAuthCallbackParser {
    /// Accepts a redirect URL, an HTTP request line, `code#state`, or a bare code.
    public static func parse(_ input: String) throws -> OAuthCallback {
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { throw OAuthError.emptyInput }

        if let queryStart = trimmed.firstIndex(of: "?") {
            let query = trimmed[trimmed.index(after: queryStart)...].prefix { $0 != " " && $0 != "#" }
            let items = URLComponents(string: "?\(query)")?.queryItems ?? []
            let value = { (name: String) in items.first { $0.name == name }?.value }
            if let error = value("error") { throw OAuthError.providerError(value("error_description") ?? error) }
            guard let code = value("code"), !code.isEmpty else { throw OAuthError.missingCode }
            return OAuthCallback(code: code, state: value("state"))
        }

        let parts = trimmed.split(separator: "#", maxSplits: 1).map(String.init)
        guard let code = parts.first, !code.isEmpty, !code.contains(" ") else { throw OAuthError.missingCode }
        return OAuthCallback(code: code, state: parts.count > 1 ? parts[1] : nil)
    }
}

public struct TokenBundle: Codable, Equatable, Sendable {
    public let accessToken: String
    public let refreshToken: String
    public let expiresAt: Date
    public let accountID: String?

    static let refreshWindow: TimeInterval = 30

    public init(accessToken: String, refreshToken: String, expiresAt: Date, accountID: String?) {
        self.accessToken = accessToken
        self.refreshToken = refreshToken
        self.expiresAt = expiresAt
        self.accountID = accountID
    }

    public func needsRefresh(now: Date) -> Bool {
        expiresAt.timeIntervalSince(now) < Self.refreshWindow
    }

    /// Builds a bundle from a token endpoint response. On refresh, missing fields fall back to `previous`.
    public static func from(tokenResponse data: Data, previous: TokenBundle?, now: Date, expirySkew: TimeInterval = 60) throws -> TokenBundle {
        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { throw OAuthError.badTokenResponse }
        if let error = json["error"] as? String {
            throw OAuthError.providerError(json["error_description"] as? String ?? error)
        }
        guard let access = json["access_token"] as? String,
              let refresh = json["refresh_token"] as? String ?? previous?.refreshToken
        else { throw OAuthError.badTokenResponse }
        let lifetime = max(30, ((json["expires_in"] as? Double) ?? 3600) - expirySkew)
        let account = (json["id_token"] as? String).flatMap(codexAccountID) ?? previous?.accountID
        return TokenBundle(accessToken: access, refreshToken: refresh, expiresAt: now.addingTimeInterval(lifetime), accountID: account)
    }

    /// The ChatGPT account id lives in the id token: the default organization, else `sub`.
    public static func codexAccountID(idToken: String) -> String? {
        let parts = idToken.split(separator: ".")
        guard parts.count >= 2,
              let payload = Data(base64URLEncoded: String(parts[1])),
              let claims = try? JSONSerialization.jsonObject(with: payload) as? [String: Any]
        else { return nil }
        let auth = claims["https://api.openai.com/auth"] as? [String: Any]
        if let orgs = auth?["organizations"] as? [[String: Any]], !orgs.isEmpty {
            let chosen = orgs.first { $0["is_default"] as? Bool == true } ?? orgs[0]
            return chosen["id"] as? String
        }
        return claims["sub"] as? String
    }
}

extension Data {
    func base64URLEncodedString() -> String {
        base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    init?(base64URLEncoded string: String) {
        let base64 = string.replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
        let padded = base64 + String(repeating: "=", count: (4 - base64.count % 4) % 4)
        self.init(base64Encoded: padded)
    }
}
