//! One-shot HTTP listener on the loopback interface that waits for the OAuth redirect.
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use crate::core::oauth::{OAuthCallback, parse_callback};
use crate::core::{Error, Result};

const MAX_REQUEST_LINE: usize = 16_384;
const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Resolves with the first request to `path` that carries a code or an error.
/// Dropping the future closes the listener.
pub async fn wait_for_callback(port: u16, path: &str) -> Result<OAuthCallback> {
    let ipv4 = TcpListener::bind(("127.0.0.1", port)).await.map_err(|error| {
        Error::message(format!(
            "Port {port} is in use, so the browser cannot hand the sign-in back. Paste the redirect address instead. ({error})"
        ))
    })?;
    // Codex redirects to "localhost", which browsers may resolve to the IPv6 loopback first.
    let ipv6 = TcpListener::bind(("::1", port)).await.ok();
    // Each connection is read in its own task: browsers open speculative connections that may never send a request.
    let (found, mut requests) = mpsc::channel::<(TcpStream, String)>(4);
    let mut readers = JoinSet::new();
    loop {
        let accepted = tokio::select! {
            accepted = ipv4.accept() => accepted,
            accepted = accept_optional(ipv6.as_ref()) => accepted,
            Some((mut stream, line)) = requests.recv() => {
                let result = parse_callback(&line);
                respond(&mut stream, result.is_ok()).await;
                return result;
            }
        };
        let (mut stream, _) = accepted?;
        let (found, path) = (found.clone(), path.to_string());
        readers.spawn(async move {
            let line = tokio::time::timeout(READ_TIMEOUT, read_request_line(&mut stream)).await.ok().flatten();
            // Favicon requests and the like are ignored; only the callback path counts.
            if let Some(line) = line.filter(|line| line.contains(&path)) {
                let _ = found.send((stream, line)).await;
            }
        });
    }
}

async fn accept_optional(listener: Option<&TcpListener>) -> std::io::Result<(TcpStream, std::net::SocketAddr)> {
    match listener {
        Some(listener) => listener.accept().await,
        None => std::future::pending().await,
    }
}

/// The request line can arrive in pieces; keep reading until its line ending shows up.
async fn read_request_line(stream: &mut TcpStream) -> Option<String> {
    let mut received = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        let count = stream.read(&mut buffer).await.ok()?;
        if count == 0 {
            return None;
        }
        received.extend_from_slice(&buffer[..count]);
        let text = String::from_utf8_lossy(&received);
        if let Some(end) = text.find("\r\n") {
            return Some(text[..end].to_string());
        }
        if received.len() > MAX_REQUEST_LINE {
            return None;
        }
    }
}

async fn respond(stream: &mut TcpStream, succeeded: bool) {
    let message = if succeeded {
        "Connected. You can close this tab and return to Redrule."
    } else {
        "Sign-in did not complete. Return to Redrule and try again."
    };
    let html = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>Redrule</title>\
         <body style=\"font:16px/1.5 -apple-system,system-ui,sans-serif;background:#f6f7f4;color:#1b1f1c;display:grid;place-items:center;height:100vh;margin:0\">\
         <p>{message}</p></body>"
    );
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",
        html.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ignores_other_paths_then_returns_the_callback() {
        let port = 48_231;
        let server = tokio::spawn(wait_for_callback(port, "/callback"));
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let _idle = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let mut favicon = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        favicon.write_all(b"GET /favicon.ico HTTP/1.1\r\n\r\n").await.unwrap();

        let mut callback = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        callback.write_all(b"GET /callback?code=abc&st").await.unwrap();
        callback.write_all(b"ate=xyz HTTP/1.1\r\nHost: x\r\n\r\n").await.unwrap();
        let mut reply = String::new();
        callback.read_to_string(&mut reply).await.unwrap();
        assert!(reply.contains("Connected."));

        let result = server.await.unwrap().unwrap();
        assert_eq!(result, OAuthCallback { code: "abc".into(), state: Some("xyz".into()) });
    }
}
