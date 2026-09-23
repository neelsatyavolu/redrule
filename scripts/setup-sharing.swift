#!/usr/bin/env swift
// Run from the repository root after `vercel link --cwd sharing`.
// Only the dedicated sharing credential crosses an in-memory pipe to Vercel.
import Foundation
import Security

func keychainQuery(_ service: String) -> [String: Any] {
    [kSecClass as String: kSecClassGenericPassword,
     kSecAttrService as String: service, kSecAttrAccount as String: "sharing"]
}
func readKey(_ service: String) -> Data? {
    var query = keychainQuery(service)
    query[kSecReturnData as String] = true
    var result: CFTypeRef?
    guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess else { return nil }
    return result as? Data
}
let base = keychainQuery("Redrule")
let key: Data
if let data = readKey("Redrule") ?? readKey("Minutes") {
    key = data
    if readKey("Redrule") == nil {
        var attributes = base
        attributes[kSecValueData as String] = key
        guard SecItemAdd(attributes as CFDictionary, nil) == errSecSuccess else { fatalError("Keychain save failed") }
    }
} else {
    var random = [UInt8](repeating: 0, count: 32)
    guard SecRandomCopyBytes(kSecRandomDefault, random.count, &random) == errSecSuccess else { fatalError("Random generation failed") }
    key = Data(random.map { String(format: "%02x", $0) }.joined().utf8)
    var attributes = base
    attributes[kSecValueData as String] = key
    guard SecItemAdd(attributes as CFDictionary, nil) == errSecSuccess else { fatalError("Keychain save failed") }
}

for environment in ["preview", "production"] {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
    process.arguments = ["vercel", "env", "add", "MINUTES_SHARE_KEY", environment, "--force", "--sensitive", "--cwd", "sharing"]
    let pipe = Pipe()
    process.standardInput = pipe
    try process.run()
    try pipe.fileHandleForWriting.write(contentsOf: key)
    try pipe.fileHandleForWriting.close()
    process.waitUntilExit()
    guard process.terminationStatus == 0 else { fatalError("Vercel credential provisioning failed") }
}
print("Sharing credential configured in Keychain and Vercel. Redeploy the sharing service to apply it.")
