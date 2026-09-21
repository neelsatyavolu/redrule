import AppKit
import MinutesCore

extension AppModel {
    func refreshConnections() async {
        var found: Set<ProviderID> = []
        for provider in ProviderID.allCases where await oauth.isConnected(provider) {
            found.insert(provider)
        }
        connected = found
    }

    /// Opens the provider's sign-in page and waits for the browser to redirect back.
    func connect(_ provider: ProviderID) {
        connectTask?.cancel()
        connectionError = nil
        connecting = provider
        connectTask = Task {
            do {
                NSWorkspace.shared.open(await oauth.begin(provider))
                let callback = try await oauth.awaitRedirect(provider)
                try await oauth.complete(provider, callback: callback)
                await finishConnecting(provider, error: nil)
            } catch is CancellationError {
                // Replaced by a pasted code or another attempt.
            } catch {
                await finishConnecting(provider, error: error)
            }
        }
    }

    /// Fallback when the browser cannot reach the loopback port: the user pastes the code or redirect URL.
    func submitPastedCode(_ text: String) {
        guard let provider = connecting else { return }
        connectTask?.cancel()
        connectTask = Task {
            do {
                try await oauth.complete(provider, callback: OAuthCallbackParser.parse(text))
                await finishConnecting(provider, error: nil)
            } catch {
                connectionError = error.localizedDescription
            }
        }
    }

    func cancelConnecting() {
        connectTask?.cancel()
        connecting = nil
        connectionError = nil
    }

    func disconnect(_ provider: ProviderID) {
        Task {
            do {
                try await oauth.disconnect(provider)
            } catch {
                connectionError = error.localizedDescription
            }
            await refreshConnections()
        }
    }

    private func finishConnecting(_ provider: ProviderID, error: Error?) async {
        connecting = nil
        connectionError = error?.localizedDescription
        await refreshConnections()
        if error == nil, !connected.isEmpty, !connected.contains(ModelChoice.all.first { $0.id == modelChoiceID }?.provider ?? provider) {
            modelChoiceID = ModelChoice.defaultChoice(for: provider).id
        }
    }
}
