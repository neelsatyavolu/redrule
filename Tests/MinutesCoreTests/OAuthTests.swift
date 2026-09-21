import Foundation
import Testing
@testable import MinutesCore

@Suite struct OAuthTests {
    @Test func pkceChallengeIsSha256OfVerifier() {
        // RFC 7636 appendix B test vector.
        let challenge = PKCE.challenge(for: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk")
        #expect(challenge == "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM")
    }

    @Test func generatedPkceIsUrlSafeAndUnique() {
        let a = PKCE.generate(), b = PKCE.generate()
        #expect(a.verifier != b.verifier)
        #expect(a.verifier.count >= 43)
        #expect(a.verifier.allSatisfy { $0.isLetter || $0.isNumber || $0 == "-" || $0 == "_" })
        #expect(a.challenge == PKCE.challenge(for: a.verifier))
    }

    @Test func codexAuthorizeUrlCarriesCliParameters() throws {
        let pkce = PKCE(verifier: "v", challenge: "c", state: "s")
        let url = OAuthConfig.codex.authorizeURL(pkce: pkce)
        let items = try #require(URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems)
        let query = Dictionary(uniqueKeysWithValues: items.map { ($0.name, $0.value ?? "") })
        #expect(url.host == "auth.openai.com")
        #expect(query["code_challenge"] == "c")
        #expect(query["state"] == "s")
        #expect(query["code_challenge_method"] == "S256")
        #expect(query["redirect_uri"] == "http://localhost:1455/auth/callback")
        #expect(query["originator"] == "codex_cli_rs")
    }

    @Test func grokConfigUsesLoopbackPort() {
        #expect(OAuthConfig.grok.callbackPort == 56121)
        #expect(OAuthConfig.grok.callbackPath == "/callback")
        #expect(OAuthConfig.codex.callbackPort == 1455)
    }

    @Test func parsesCallbackUrlAndBareCode() throws {
        let full = try OAuthCallbackParser.parse("http://localhost:1455/auth/callback?code=abc&state=xyz")
        #expect(full == OAuthCallback(code: "abc", state: "xyz"))
        #expect(try OAuthCallbackParser.parse("  abc123 ") == OAuthCallback(code: "abc123", state: nil))
        #expect(try OAuthCallbackParser.parse("abc#xyz") == OAuthCallback(code: "abc", state: "xyz"))
        #expect(try OAuthCallbackParser.parse("GET /callback?code=q&state=r HTTP/1.1") == OAuthCallback(code: "q", state: "r"))
    }

    @Test func callbackErrorsAreSurfaced() {
        #expect(throws: OAuthError.self) { try OAuthCallbackParser.parse("http://x/cb?error=access_denied") }
        #expect(throws: OAuthError.self) { try OAuthCallbackParser.parse("   ") }
    }

    @Test func tokenBundleRefreshWindow() {
        let now = Date(timeIntervalSince1970: 1000)
        let fresh = TokenBundle(accessToken: "a", refreshToken: "r", expiresAt: now.addingTimeInterval(600), accountID: nil)
        let stale = TokenBundle(accessToken: "a", refreshToken: "r", expiresAt: now.addingTimeInterval(10), accountID: nil)
        #expect(!fresh.needsRefresh(now: now))
        #expect(stale.needsRefresh(now: now))
    }

    @Test func tokenResponseBecomesBundle() throws {
        let json = Data(#"{"access_token":"a","refresh_token":"r","expires_in":3600}"#.utf8)
        let now = Date(timeIntervalSince1970: 0)
        let bundle = try TokenBundle.from(tokenResponse: json, previous: nil, now: now)
        #expect(bundle.accessToken == "a")
        #expect(bundle.expiresAt == now.addingTimeInterval(3540))
    }

    @Test func refreshKeepsOldRefreshTokenAndAccount() throws {
        let previous = TokenBundle(accessToken: "old", refreshToken: "keep", expiresAt: .distantPast, accountID: "acct")
        let json = Data(#"{"access_token":"new","expires_in":100}"#.utf8)
        let bundle = try TokenBundle.from(tokenResponse: json, previous: previous, now: Date())
        #expect(bundle.refreshToken == "keep")
        #expect(bundle.accountID == "acct")
    }

    @Test func tokenErrorResponseThrows() {
        let json = Data(#"{"error":"invalid_grant","error_description":"expired"}"#.utf8)
        #expect(throws: OAuthError.self) { try TokenBundle.from(tokenResponse: json, previous: nil, now: Date()) }
    }

    @Test func decodesDefaultOrganizationFromIdToken() {
        let claims = #"{"sub":"user","https://api.openai.com/auth":{"organizations":[{"id":"org-a"},{"id":"org-b","is_default":true}]}}"#
        let payload = Data(claims.utf8).base64EncodedString()
            .replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
        #expect(TokenBundle.codexAccountID(idToken: "h.\(payload).s") == "org-b")
        #expect(TokenBundle.codexAccountID(idToken: "garbage") == nil)
    }
}
