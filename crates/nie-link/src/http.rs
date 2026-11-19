//! A small blocking HTTP/1.1 server: GET and HEAD, keep-alive, one thread per connection.
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

const MAX_HEAD: usize = 16 * 1024;

#[derive(Debug)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub query: String,
    headers: Vec<(String, String)>,
}

impl Request {
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// Value of `key` in the query string.
    #[must_use]
    pub fn param(&self, key: &str) -> Option<&str> {
        self.query
            .split('&')
            .filter_map(|kv| kv.split_once('='))
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v)
    }
}

pub enum Body {
    Bytes(Vec<u8>),
    Shared(Arc<Vec<u8>>),
}

impl Body {
    fn as_slice(&self) -> &[u8] {
        match self {
            Self::Bytes(b) => b,
            Self::Shared(b) => b,
        }
    }
}

pub struct Response {
    pub status: u16,
    pub content_type: &'static str,
    pub headers: Vec<(&'static str, String)>,
    pub body: Body,
}

impl Response {
    #[must_use]
    pub fn json(status: u16, value: &serde_json::Value) -> Self {
        Self {
            status,
            content_type: "application/json",
            headers: Vec::new(),
            body: Body::Bytes(serde_json::to_vec(value).unwrap_or_default()),
        }
    }

    #[must_use]
    pub fn error(status: u16, message: &str) -> Self {
        Self::json(status, &serde_json::json!({ "error": message }))
    }

    #[must_use]
    pub fn bytes(content_type: &'static str, body: Body) -> Self {
        Self { status: 200, content_type, headers: Vec::new(), body }
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Error",
    }
}

fn read_request(reader: &mut BufReader<TcpStream>) -> io::Result<Option<Request>> {
    let mut head = String::new();
    let mut line = String::new();
    loop {
        line.clear();
        if reader.by_ref().take(MAX_HEAD as u64).read_line(&mut line)? == 0 {
            return Ok(None);
        }
        if line.trim_end().is_empty() {
            if head.is_empty() {
                continue;
            }
            break;
        }
        head.push_str(&line);
        if head.len() > MAX_HEAD {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "request head too large"));
        }
    }
    let mut lines = head.lines();
    let first = lines.next().unwrap_or_default();
    let mut parts = first.split(' ');
    let (Some(method), Some(target)) = (parts.next(), parts.next()) else {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "bad request line"));
    };
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let headers = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect();
    Ok(Some(Request {
        method: method.to_owned(),
        path: path.to_owned(),
        query: query.to_owned(),
        headers,
    }))
}

fn write_response(out: &mut impl Write, head_only: bool, keep: bool, r: &Response) -> io::Result<()> {
    let body = r.body.as_slice();
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: {}\r\nX-Content-Type-Options: nosniff\r\n",
        r.status,
        reason(r.status),
        r.content_type,
        body.len(),
        if keep { "keep-alive" } else { "close" }
    );
    for (k, v) in &r.headers {
        head.push_str(k);
        head.push_str(": ");
        head.push_str(v);
        head.push_str("\r\n");
    }
    head.push_str("\r\n");
    out.write_all(head.as_bytes())?;
    if !head_only {
        out.write_all(body)?;
    }
    out.flush()
}

fn connection(stream: TcpStream, handler: &(dyn Fn(&Request) -> Response + Send + Sync)) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(120)));
    let Ok(mut out) = stream.try_clone() else { return };
    let mut reader = BufReader::new(stream);
    while let Ok(Some(req)) = read_request(&mut reader) {
        let keep = !req.header("connection").is_some_and(|v| v.eq_ignore_ascii_case("close"));
        let head_only = req.method == "HEAD";
        let resp = if req.method == "GET" || head_only {
            handler(&req)
        } else {
            Response::error(405, "method not allowed")
        };
        if write_response(&mut out, head_only, keep, &resp).is_err() || !keep {
            break;
        }
    }
}

/// Accept connections until the process ends; at most `max_connections` run at once.
pub fn serve(
    listener: &TcpListener,
    max_connections: usize,
    handler: Arc<dyn Fn(&Request) -> Response + Send + Sync>,
) {
    let live = Arc::new(AtomicUsize::new(0));
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        if live.load(Ordering::Relaxed) >= max_connections {
            drop(stream);
            continue;
        }
        live.fetch_add(1, Ordering::Relaxed);
        let (live, handler) = (live.clone(), handler.clone());
        std::thread::spawn(move || {
            connection(stream, handler.as_ref());
            live.fetch_sub(1, Ordering::Relaxed);
        });
    }
}

/// Constant-time bearer check: both sides are hashed, then compared.
#[must_use]
pub fn bearer_ok(req: &Request, token: &str) -> bool {
    let given = req
        .header("authorization")
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default();
    blake3::hash(given.as_bytes()) == blake3::hash(token.as_bytes())
}
