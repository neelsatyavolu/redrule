import Foundation
import MinutesCore
import Network

/// One-shot HTTP listener on 127.0.0.1 that waits for the OAuth redirect.
final class LoopbackServer: @unchecked Sendable {
    private let listener: NWListener
    private let path: String
    private let queue = DispatchQueue(label: "minutes.oauth-loopback")
    private var continuation: CheckedContinuation<OAuthCallback, Error>?

    init(port: UInt16, path: String) throws {
        let parameters = NWParameters.tcp
        parameters.allowLocalEndpointReuse = true
        parameters.requiredInterfaceType = .loopback
        listener = try NWListener(using: parameters, on: NWEndpoint.Port(rawValue: port)!)
        self.path = path
    }

    /// Resolves with the first request that carries a code or an error. Cancelling the task stops the listener.
    func waitForCallback() async throws -> OAuthCallback {
        try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                queue.async {
                    self.continuation = continuation
                    self.listener.newConnectionHandler = { [weak self] in self?.handle($0) }
                    self.listener.stateUpdateHandler = { [weak self] state in
                        if case .failed(let error) = state { self?.finish(.failure(error)) }
                    }
                    self.listener.start(queue: self.queue)
                }
            }
        } onCancel: {
            queue.async { self.finish(.failure(CancellationError())) }
        }
    }

    private func handle(_ connection: NWConnection) {
        connection.start(queue: queue)
        connection.receive(minimumIncompleteLength: 1, maximumLength: 16_384) { [weak self] data, _, _, _ in
            guard let self, let data, let requestLine = String(decoding: data, as: UTF8.self).components(separatedBy: "\r\n").first,
                  requestLine.contains(self.path)
            else {
                connection.cancel()
                return
            }
            let result = Result { try OAuthCallbackParser.parse(requestLine) }
            let succeeded = (try? result.get()) != nil
            self.respond(on: connection, succeeded: succeeded)
            self.finish(result)
        }
    }

    private func respond(on connection: NWConnection, succeeded: Bool) {
        let message = succeeded ? "Connected. You can close this tab and return to Minutes." : "Sign-in did not complete. Return to Minutes and try again."
        let html = """
        <!doctype html><meta charset="utf-8"><title>Minutes</title>
        <body style="font: 17px/1.5 'New York', Georgia, serif; background:#EFF3EA; color:#16211B; display:grid; place-items:center; height:100vh; margin:0">
        <p>\(message)</p></body>
        """
        let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: \(html.utf8.count)\r\nConnection: close\r\n\r\n\(html)"
        connection.send(content: Data(response.utf8), completion: .contentProcessed { _ in connection.cancel() })
    }

    private func finish(_ result: Result<OAuthCallback, Error>) {
        guard let continuation else { return }
        self.continuation = nil
        // Give the response a moment to flush before the listener goes away.
        queue.asyncAfter(deadline: .now() + 0.5) { self.listener.cancel() }
        continuation.resume(with: result)
    }
}
