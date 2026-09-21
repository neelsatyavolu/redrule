import Foundation
import MinutesCore
import Network

/// One-shot HTTP listener on 127.0.0.1 that waits for the OAuth redirect.
final class LoopbackServer: @unchecked Sendable {
    private let listener: NWListener
    private let path: String
    private let queue = DispatchQueue(label: "minutes.oauth-loopback")
    private var continuation: CheckedContinuation<OAuthCallback, Error>?
    private var cancelled = false

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
                    guard !self.cancelled else {
                        continuation.resume(throwing: CancellationError())
                        return
                    }
                    self.continuation = continuation
                    self.listener.newConnectionHandler = { [weak self] in self?.handle($0) }
                    self.listener.stateUpdateHandler = { [weak self] state in
                        if case .failed(let error) = state { self?.finish(.failure(error)) }
                    }
                    self.listener.start(queue: self.queue)
                }
            }
        } onCancel: {
            queue.async {
                self.cancelled = true
                self.listener.cancel()
                self.finish(.failure(CancellationError()))
            }
        }
    }

    private func handle(_ connection: NWConnection) {
        connection.start(queue: queue)
        receiveRequestLine(on: connection, buffered: Data())
    }

    /// The request line can arrive in pieces; keep reading until its line ending shows up.
    private func receiveRequestLine(on connection: NWConnection, buffered: Data) {
        connection.receive(minimumIncompleteLength: 1, maximumLength: 16_384) { [weak self] data, _, isComplete, error in
            guard let self else { return }
            let received = buffered + (data ?? Data())
            let text = String(decoding: received, as: UTF8.self)
            guard let requestLine = text.components(separatedBy: "\r\n").first, text.contains("\r\n") else {
                if isComplete || error != nil || received.count > 16_384 {
                    connection.cancel()
                } else {
                    self.receiveRequestLine(on: connection, buffered: received)
                }
                return
            }
            // Browsers also open speculative connections and ask for favicons; ignore those and keep listening.
            guard requestLine.contains(self.path) else {
                connection.cancel()
                return
            }
            let result = Result { try OAuthCallbackParser.parse(requestLine) }
            self.respond(on: connection, succeeded: (try? result.get()) != nil)
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
