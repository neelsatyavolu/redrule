import Foundation
import MinutesCore
import Security

enum KeychainError: LocalizedError {
    case status(OSStatus)

    var errorDescription: String? {
        guard case .status(let status) = self else { return nil }
        let message = SecCopyErrorMessageString(status, nil) as String? ?? "code \(status)"
        return "Keychain error: \(message)"
    }
}

/// Stores one token bundle per provider as a generic password in the login keychain.
struct TokenKeychain {
    private static let service = "Minutes"

    func load(_ provider: ProviderID) throws -> TokenBundle? {
        var query = baseQuery(provider)
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = item as? Data else { throw KeychainError.status(status) }
        return try? JSONDecoder().decode(TokenBundle.self, from: data)
    }

    func save(_ bundle: TokenBundle, for provider: ProviderID) throws {
        let data = try JSONEncoder().encode(bundle)
        let update = SecItemUpdate(baseQuery(provider) as CFDictionary, [kSecValueData as String: data] as CFDictionary)
        if update == errSecSuccess { return }
        guard update == errSecItemNotFound else { throw KeychainError.status(update) }
        var attributes = baseQuery(provider)
        attributes[kSecValueData as String] = data
        let add = SecItemAdd(attributes as CFDictionary, nil)
        guard add == errSecSuccess else { throw KeychainError.status(add) }
    }

    func delete(_ provider: ProviderID) throws {
        let status = SecItemDelete(baseQuery(provider) as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else { throw KeychainError.status(status) }
    }

    private func baseQuery(_ provider: ProviderID) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: Self.service, kSecAttrAccount as String: provider.rawValue]
    }
}
