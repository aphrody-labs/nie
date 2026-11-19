//! @name@: HTTP service on the nie framework, std only.

#![forbid(unsafe_code)]

use std::io::{BufRead as _, BufReader, Write as _};
use std::net::TcpListener;

@plugin@
/// Status, content type and body of the answer to a request line such as `GET /health HTTP/1.1`.
#[must_use]
pub fn route(request_line: &str) -> (u16, &'static str, String) {
    let mut parts = request_line.split_whitespace();
    match (parts.next(), parts.next()) {
        (Some("GET"), Some("/health")) => (
            200,
            "application/json",
            format!("{{\"ok\":true,\"service\":\"{}\"}}", env!("CARGO_PKG_NAME")),
        ),
        (Some("GET"), Some(_)) => (404, "text/plain", "not found\n".to_owned()),
        _ => (405, "text/plain", "method not allowed\n".to_owned()),
    }
}

/// Serves connections one thread each until the listener fails.
///
/// # Errors
/// When the listener cannot accept.
pub fn serve(listener: &TcpListener) -> std::io::Result<()> {
    for stream in listener.incoming() {
        let mut stream = stream?;
        std::thread::spawn(move || {
            let mut line = String::new();
            let Ok(read) = stream.try_clone() else { return };
            if BufReader::new(read).read_line(&mut line).is_err() {
                return;
            }
            let (status, kind, body) = route(line.trim());
            let _ = write!(
                stream,
                "HTTP/1.1 {status} X\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        });
    }
    Ok(())
}
