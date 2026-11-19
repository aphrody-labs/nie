//! A minimal Chrome DevTools Protocol client: blocking JSON messages over a plain `ws://127.0.0.1` socket.
//!
//! `call` sends a command and waits for the response with the same id; every event that arrives meanwhile
//! is queued for the caller. `pump` collects events for a fixed time (the wait between page steps).

use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use tungstenite::{Message, WebSocket};

pub struct Cdp {
    ws: WebSocket<TcpStream>,
    next_id: u64,
    events: Vec<Value>,
}

impl Cdp {
    /// Connects to a `ws://host:port/path` debugger URL.
    pub fn connect(url: &str) -> Result<Self> {
        let host = url
            .strip_prefix("ws://")
            .and_then(|r| r.split('/').next())
            .context("only ws:// URLs are supported")?;
        let stream = TcpStream::connect(host).with_context(|| format!("connect {host}"))?;
        stream.set_nodelay(true)?;
        let (ws, _) = tungstenite::client(url, stream)
            .map_err(|e| anyhow::anyhow!("websocket handshake: {e}"))?;
        Ok(Self {
            ws,
            next_id: 0,
            events: Vec::new(),
        })
    }

    fn set_timeout(&self, timeout: Option<Duration>) -> Result<()> {
        self.ws.get_ref().set_read_timeout(timeout)?;
        Ok(())
    }

    /// Reads one JSON message; `None` when the read timed out.
    fn read(&mut self) -> Result<Option<Value>> {
        loop {
            match self.ws.read() {
                Ok(Message::Text(text)) => return Ok(Some(serde_json::from_str(text.as_str())?)),
                Ok(Message::Binary(bytes)) => return Ok(Some(serde_json::from_slice(&bytes)?)),
                Ok(Message::Close(_)) => bail!("the debugger closed the connection"),
                Ok(_) => {}
                Err(tungstenite::Error::Io(e))
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    return Ok(None);
                }
                Err(e) => bail!("websocket: {e}"),
            }
        }
    }

    fn file(&mut self, message: Value) {
        if message.get("method").is_some() {
            self.events.push(message);
        }
    }

    /// Sends a command and returns its `result`; a protocol error becomes an `Err`.
    pub fn call(&mut self, method: &str, params: &Value) -> Result<Value> {
        self.next_id += 1;
        let id = self.next_id;
        self.ws
            .send(Message::text(
                json!({ "id": id, "method": method, "params": params }).to_string(),
            ))
            .map_err(|e| anyhow::anyhow!("websocket send: {e}"))?;
        self.set_timeout(Some(Duration::from_secs(60)))?;
        loop {
            let Some(message) = self.read()? else {
                bail!("timed out waiting for {method}")
            };
            if message.get("id").and_then(Value::as_u64) == Some(id) {
                if let Some(error) = message.get("error") {
                    bail!("{method}: {error}");
                }
                return Ok(message.get("result").cloned().unwrap_or(Value::Null));
            }
            self.file(message);
        }
    }

    /// Collects events for `duration`.
    pub fn pump(&mut self, duration: Duration) -> Result<()> {
        let end = Instant::now() + duration;
        loop {
            let left = end.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(());
            }
            self.set_timeout(Some(left))?;
            if let Some(message) = self.read()? {
                self.file(message);
            }
        }
    }

    /// Takes the events queued so far.
    pub fn take_events(&mut self) -> Vec<Value> {
        std::mem::take(&mut self.events)
    }

    pub fn close(&mut self) {
        let _ = self.ws.close(None);
    }
}

/// `GET http://host:port/path` over a bare socket, returning the response body.
pub fn http_get(host_port: &str, path: &str) -> Result<String> {
    let mut stream = TcpStream::connect(host_port)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {host_port}\r\nConnection: close\r\n\r\n"
    )?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    Ok(text
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_owned())
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    use super::*;

    /// A debugger stand-in: for every command it first sends an event and a stale response, then the answer.
    fn fake_debugger() -> (String, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("ws://{}/devtools/page/1", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut ws = tungstenite::accept(stream).unwrap();
            let mut seen = Vec::new();
            loop {
                let Ok(Message::Text(text)) = ws.read() else {
                    return seen;
                };
                let command: Value = serde_json::from_str(text.as_str()).unwrap();
                let id = command["id"].as_u64().unwrap();
                let method = command["method"].as_str().unwrap().to_owned();
                seen.push(method.clone());
                ws.send(Message::text(
                    json!({ "method": "Log.entryAdded", "params": { "n": id } }).to_string(),
                ))
                .unwrap();
                ws.send(Message::text(
                    json!({ "id": id + 100, "result": {} }).to_string(),
                ))
                .unwrap();
                let reply = if method == "Bad.method" {
                    json!({ "id": id, "error": { "code": -32601, "message": "nope" } })
                } else {
                    json!({ "id": id, "result": { "echo": command["params"] } })
                };
                ws.send(Message::text(reply.to_string())).unwrap();
            }
        });
        (url, handle)
    }

    #[test]
    fn responses_match_by_id_and_events_are_queued() {
        let (url, server) = fake_debugger();
        let mut cdp = Cdp::connect(&url).unwrap();
        let first = cdp.call("Page.enable", &json!({ "a": 1 })).unwrap();
        assert_eq!(first["echo"]["a"], 1);
        let second = cdp
            .call("Runtime.evaluate", &json!({ "expression": "1" }))
            .unwrap();
        assert_eq!(second["echo"]["expression"], "1");
        let events = cdp.take_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["method"], "Log.entryAdded");
        assert_eq!(events[1]["params"]["n"], 2);
        assert!(cdp.take_events().is_empty());
        cdp.close();
        drop(cdp);
        assert_eq!(server.join().unwrap(), ["Page.enable", "Runtime.evaluate"]);
    }

    #[test]
    fn protocol_errors_are_errors_and_pump_gathers_late_events() {
        let (url, server) = fake_debugger();
        let mut cdp = Cdp::connect(&url).unwrap();
        let err = cdp.call("Bad.method", &json!({})).unwrap_err().to_string();
        assert!(err.contains("Bad.method") && err.contains("nope"), "{err}");
        let started = Instant::now();
        cdp.pump(Duration::from_millis(150)).unwrap();
        assert!(started.elapsed() >= Duration::from_millis(140));
        assert_eq!(cdp.take_events().len(), 1);
        cdp.close();
        drop(cdp);
        server.join().unwrap();
    }

    #[test]
    fn only_plain_websocket_urls_are_accepted() {
        assert!(Cdp::connect("wss://127.0.0.1:1/x").is_err());
    }

    #[test]
    fn http_get_returns_the_body() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 512];
            let _ = stream.read(&mut buf).unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\n[]")
                .unwrap();
        });
        assert_eq!(http_get(&addr, "/json").unwrap(), "[]");
        server.join().unwrap();
    }
}
