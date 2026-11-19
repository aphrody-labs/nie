//! A minimal HTTP/1.1 GET over `std::net`, for health probes of loopback services. Plain HTTP
//! only, no redirects, bounded body: it exists so images and the deploy tool need neither `curl`
//! nor a shell.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

/// An answer: status code and body (at most [`MAX_BODY`] bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// HTTP status code.
    pub status: u16,
    /// Response body, lossily decoded.
    pub body: String,
}

/// Bytes of body kept.
pub const MAX_BODY: usize = 64 * 1024;

/// Split `http://host:port/path` into the socket address and the request target.
pub fn parse_url(url: &str) -> Result<(String, String), String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| format!("{url}: only http:// is supported"))?;
    let (authority, path) = rest.split_once('/').map_or((rest, "/".to_owned()), |(a, p)| {
        (a, format!("/{p}"))
    });
    if authority.is_empty() {
        return Err(format!("{url}: empty host"));
    }
    let authority = if authority.contains(':') {
        authority.to_owned()
    } else {
        format!("{authority}:80")
    };
    Ok((authority, path))
}

/// `GET url` with a timeout for connect, write and read.
pub fn get(url: &str, timeout: Duration) -> Result<Answer, String> {
    let (authority, path) = parse_url(url)?;
    let addr: SocketAddr = authority
        .to_socket_addrs()
        .map_err(|e| format!("{authority}: {e}"))?
        .next()
        .ok_or_else(|| format!("{authority}: no address"))?;
    let mut stream =
        TcpStream::connect_timeout(&addr, timeout).map_err(|e| format!("{authority}: {e}"))?;
    stream.set_read_timeout(Some(timeout)).map_err(|e| e.to_string())?;
    stream.set_write_timeout(Some(timeout)).map_err(|e| e.to_string())?;
    let host = authority.rsplit_once(':').map_or(authority.as_str(), |(h, _)| h);
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: nie-health\r\nAccept: */*\r\nConnection: close\r\n\r\n"
    )
    .map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    let mut chunk = [0u8; 8192];
    while raw.len() < MAX_BODY + 16 * 1024 {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => raw.extend_from_slice(&chunk[..n]),
            Err(e)
                if !raw.is_empty()
                    && matches!(
                        e.kind(),
                        std::io::ErrorKind::TimedOut
                            | std::io::ErrorKind::WouldBlock
                            | std::io::ErrorKind::ConnectionReset
                    ) =>
            {
                break;
            }
            Err(e) => return Err(format!("{authority}: {e}")),
        }
    }
    parse_response(&raw)
}

/// Parse a raw HTTP/1.1 response; decodes `Transfer-Encoding: chunked`.
pub fn parse_response(raw: &[u8]) -> Result<Answer, String> {
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("truncated response")?;
    let head = String::from_utf8_lossy(&raw[..split]);
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or("bad status line")?;
    let chunked = lines.any(|l| {
        let l = l.to_ascii_lowercase();
        l.starts_with("transfer-encoding:") && l.contains("chunked")
    });
    let body = &raw[split + 4..];
    let body = if chunked { dechunk(body) } else { body.to_vec() };
    let body = &body[..body.len().min(MAX_BODY)];
    Ok(Answer {
        status,
        body: String::from_utf8_lossy(body).into_owned(),
    })
}

fn dechunk(mut data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(eol) = data.windows(2).position(|w| w == b"\r\n") {
        let size = std::str::from_utf8(&data[..eol])
            .ok()
            .and_then(|s| usize::from_str_radix(s.split(';').next().unwrap_or("").trim(), 16).ok())
            .unwrap_or(0);
        data = &data[eol + 2..];
        if size == 0 || data.len() < size {
            out.extend_from_slice(&data[..data.len().min(size)]);
            break;
        }
        out.extend_from_slice(&data[..size]);
        data = data.get(size + 2..).unwrap_or(&[]);
    }
    out
}

/// A probe passes on status 200 and, when given, a body that contains `needle`.
#[must_use]
pub fn passes(answer: &Answer, needle: Option<&str>) -> bool {
    answer.status == 200 && needle.is_none_or(|n| answer.body.contains(n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn serve_once(response: &'static [u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut seen = Vec::new();
            let mut buf = [0u8; 1024];
            while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = s.read(&mut buf).unwrap();
                seen.extend_from_slice(&buf[..n]);
            }
            s.write_all(response).unwrap();
        });
        format!("http://{addr}/health")
    }

    #[test]
    fn plain_and_chunked_bodies() {
        let url = serve_once(b"HTTP/1.1 200 OK\r\nContent-Length: 17\r\n\r\n{\"vfsReady\":true}");
        let a = get(&url, Duration::from_secs(2)).unwrap();
        assert!(passes(&a, Some("\"vfsReady\":true")));
        let url = serve_once(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n",
        );
        assert_eq!(get(&url, Duration::from_secs(2)).unwrap().body, "hello world");
    }

    #[test]
    fn status_and_needle_decide() {
        let url = serve_once(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 2\r\n\r\nno");
        let a = get(&url, Duration::from_secs(2)).unwrap();
        assert_eq!(a.status, 503);
        assert!(!passes(&a, None));
        let ok = Answer { status: 200, body: "{\"vfsReady\":false}".into() };
        assert!(!passes(&ok, Some("\"vfsReady\":true")));
    }

    #[test]
    fn rejects_non_http_and_dead_ports() {
        assert!(parse_url("https://x/").is_err());
        assert_eq!(
            parse_url("http://127.0.0.1:8790/health").unwrap(),
            ("127.0.0.1:8790".into(), "/health".into())
        );
        assert!(get("http://127.0.0.1:1/", Duration::from_millis(300)).is_err());
    }
}
