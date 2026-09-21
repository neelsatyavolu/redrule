import Foundation
import MinutesCore

/// OAuth PKCE sign-in, token storage and refresh for Codex and Grok.
actor OAuthService {
    private static let redirectTimeout: Duration = .seconds(300)

    private let keychain = TokenKeychain()
    private var pending: [ProviderID: PKCE] = [:]
    private var cache: [ProviderID: TokenBundle] = [:]

    func isConnected(_ provider: ProviderID) -> Bool {
        (try? storedTokens(provider)) != nil
    }

    /// Starts a sign-in and returns the URL to open in the browser.
    func begin(_ provider: ProviderID) -> URL {
        let pkce = PKCE.generate()
        pending[provider] = pkce
        return OAuthConfig.config(for: provider).authorizeURL(pkce: pkce)
    }

    /// Waits for the browser redirect on the provider's loopback port.
    nonisolated func awaitRedirect(_ provider: ProviderID) async throws -> OAuthCallback {
        let config = OAuthConfig.config(for: provider)
        let server = try LoopbackServer(port: config.callbackPort, path: config.callbackPath)
        return try await withThrowingTaskGroup(of: OAuthCallback.self) { group in
            group.addTask { try await server.waitForCallback() }
            group.addTask {
                try await Task.sleep(for: Self.redirectTimeout)
                throw OAuthError.providerError("Timed out waiting for the browser.")
            }
            defer { group.cancelAll() }
            return try await group.next()!
        }
    }

    /// Exchanges the authorization code for tokens and stores them.
    func complete(_ provider: ProviderID, callback: OAuthCallback) async throws {
        guard let pkce = pending[provider] else { throw OAuthError.providerError("Start connecting first.") }
        if let state = callback.state, state != pkce.state { throw OAuthError.stateMismatch }
        let config = OAuthConfig.config(for: provider)
        let data = try await postForm(config.tokenEndpoint, config.exchangeParameters(code: callback.code, verifier: pkce.verifier))
        let bundle = try TokenBundle.from(tokenResponse: data, previous: nil, now: Date(), expirySkew: config.expirySkew)
        try store(bundle, for: provider)
        pending[provider] = nil
    }

    func disconnect(_ provider: ProviderID) throws {
        cache[provider] = nil
        pending[provider] = nil
        try keychain.delete(provider)
    }

    /// A valid token bundle, refreshed if it is about to expire.
    func activeTokens(_ provider: ProviderID) async throws -> TokenBundle {
        guard let tokens = try storedTokens(provider) else { throw SummaryError.noProviderConnected }
        guard tokens.needsRefresh(now: Date()) else { return tokens }
        let config = OAuthConfig.config(for: provider)
        do {
            let data = try await postForm(config.tokenEndpoint, config.refreshParameters(refreshToken: tokens.refreshToken))
            let refreshed = try TokenBundle.from(tokenResponse: data, previous: tokens, now: Date(), expirySkew: config.expirySkew)
            try store(refreshed, for: provider)
            return refreshed
        } catch let error as OAuthError {
            // The refresh token was rejected: the user has to sign in again.
            try? disconnect(provider)
            throw SummaryError.provider("\(provider.displayName) needs to be reconnected in Settings. \(error.localizedDescription)")
        }
    }

    private func storedTokens(_ provider: ProviderID) throws -> TokenBundle? {
        if let cached = cache[provider] { return cached }
        let loaded = try keychain.load(provider)
        cache[provider] = loaded
        return loaded
    }

    private func store(_ bundle: TokenBundle, for provider: ProviderID) throws {
        try keychain.save(bundle, for: provider)
        cache[provider] = bundle
    }

    private func postForm(_ url: URL, _ parameters: [String: String]) async throws -> Data {
        var components = URLComponents()
        components.queryItems = parameters.map { URLQueryItem(name: $0.key, value: $0.value) }
        var request = URLRequest(url: url, timeoutInterval: 30)
        request.httpMethod = "POST"
        request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        // URLComponents leaves "+" unescaped, which form decoding would read as a space.
        request.httpBody = Data((components.percentEncodedQuery ?? "").replacingOccurrences(of: "+", with: "%2B").utf8)
        let (data, _) = try await URLSession.shared.data(for: request)
        return data
    }
}
