//! `web serve`: hosts a bundle made by `web pack` and forwards `/v1/*` and `/tex/*` to a running `nie serve`.
//!
//! A stand-in for the embedding in `nie serve` (`crates/nie-app`) for local proofs: the page and the data
//! come from one origin, as they do when the bundle is embedded. One thread per connection, HTTP/1.1 with
//! `Connection: close`.

use std::collections::BTreeSet;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use anyhow::{Context, Result};

/// Arguments of `web serve`.
#[derive(clap::Args, Debug)]
pub struct Args {
    /// Bundle directory made by `web pack`.
    pub dist: PathBuf,
    /// Base URL of a running `nie serve`.
    pub upstream: String,
    /// Port to listen on (loopback).
    #[arg(long, default_value_t = 8800)]
    pub port: u16,
}

/// One file of `manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub content_type: String,
    pub immutable: bool,
    pub entry: bool,
}

/// What a request path resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// Forward to the upstream with this path and query.
    Proxy(String),
    /// Serve this manifest entry.
    File(Entry),
    NotFound,
}

pub fn parse_manifest(text: &str) -> Result<Vec<Entry>> {
    let value: serde_json::Value = serde_json::from_str(text).context("manifest.json")?;
    let files = value["files"]
        .as_array()
        .context("manifest.json has no files array")?;
    Ok(files
        .iter()
        .filter_map(|f| {
            Some(Entry {
                name: f["name"].as_str()?.to_owned(),
                content_type: f["type"].as_str()?.to_owned(),
                immutable: f["immutable"].as_bool().unwrap_or(false),
                entry: f["entry"].as_bool().unwrap_or(false),
            })
        })
        .collect())
}

/// Routing of a request target (`path?query`): API and texture paths go upstream, `/` and `/app/` are the
/// entry page, anything else is a manifest name (optionally under `/app/`) that exists on disk.
pub fn route(target: &str, manifest: &[Entry], present: &BTreeSet<String>) -> Route {
    let (path, _) = target.split_once('?').unwrap_or((target, ""));
    if path.starts_with("/v1/") || path.starts_with("/tex/") {
        return Route::Proxy(target.to_owned());
    }
    let name = if path == "/" || path == "/app/" {
        match manifest.iter().find(|e| e.entry) {
            Some(e) => e.name.clone(),
            None => return Route::NotFound,
        }
    } else {
        let rest = path.strip_prefix('/').unwrap_or(path);
        rest.strip_prefix("app/").unwrap_or(rest).to_owned()
    };
    match manifest.iter().find(|e| e.name == name) {
        Some(e) if present.contains(&name) => Route::File(e.clone()),
        _ => Route::NotFound,
    }
}

struct State {
    dist: PathBuf,
    upstream: String,
    manifest: Vec<Entry>,
    present: BTreeSet<String>,
    client: OnceLock<reqwest::blocking::Client>,
}

impl State {
    fn load(dist: &Path, upstream: &str) -> Result<Self> {
        let manifest = parse_manifest(
            &std::fs::read_to_string(dist.join("manifest.json")).context("manifest.json")?,
        )?;
        let present = std::fs::read_dir(dist)?
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        Ok(Self {
            dist: dist.to_path_buf(),
            upstream: upstream.trim_end_matches('/').to_owned(),
            manifest,
            present,
            client: OnceLock::new(),
        })
    }

    fn client(&self) -> Result<&reqwest::blocking::Client> {
        if let Some(c) = self.client.get() {
            return Ok(c);
        }
        let built = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()?;
        Ok(self.client.get_or_init(|| built))
    }
}

struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Reply {
    fn text(status: u16, text: &str) -> Self {
        Self {
            status,
            headers: vec![("content-type".into(), "text/plain; charset=utf-8".into())],
            body: text.as_bytes().to_vec(),
        }
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        206 => "Partial Content",
        304 => "Not Modified",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        502 => "Bad Gateway",
        _ => "Status",
    }
}

fn respond(state: &State, target: &str) -> Reply {
    match route(target, &state.manifest, &state.present) {
        Route::NotFound => Reply::text(404, "not found"),
        Route::File(entry) => match std::fs::read(state.dist.join(&entry.name)) {
            Ok(body) => {
                let cache = if entry.immutable {
                    "public, max-age=31536000, immutable"
                } else {
                    "no-cache"
                };
                Reply {
                    status: 200,
                    headers: vec![
                        ("content-type".into(), entry.content_type),
                        ("cache-control".into(), cache.into()),
                    ],
                    body,
                }
            }
            Err(_) => Reply::text(404, "not found"),
        },
        Route::Proxy(path) => match proxy(state, &path) {
            Ok(reply) => reply,
            Err(e) => Reply::text(502, &format!("upstream: {e:#}")),
        },
    }
}

fn proxy(state: &State, path: &str) -> Result<Reply> {
    let response = state
        .client()?
        .get(format!("{}{path}", state.upstream))
        .send()?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let body = response.bytes()?.to_vec();
    let headers = content_type
        .map(|v| vec![("content-type".to_owned(), v)])
        .unwrap_or_default();
    Ok(Reply {
        status,
        headers,
        body,
    })
}

fn handle(mut stream: TcpStream, state: &State) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    // Drain the headers; no request carries a body here.
    loop {
        let mut line = String::new();
        if reader.by_ref().take(16 * 1024).read_line(&mut line)? == 0 || line.trim_end().is_empty()
        {
            break;
        }
    }
    let mut parts = request_line.split_whitespace();
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or("/"));
    let reply = match method {
        "GET" | "HEAD" => respond(state, target),
        _ => Reply::text(405, "method not allowed"),
    };
    let mut head = format!("HTTP/1.1 {} {}\r\n", reply.status, reason(reply.status));
    for (k, v) in &reply.headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str(&format!(
        "content-length: {}\r\nconnection: close\r\n\r\n",
        reply.body.len()
    ));
    stream.write_all(head.as_bytes())?;
    if method != "HEAD" {
        stream.write_all(&reply.body)?;
    }
    stream.flush()?;
    Ok(())
}

/// Accepts connections forever, one thread each.
fn serve_on(listener: &TcpListener, state: Arc<State>) {
    for stream in listener.incoming().flatten() {
        let state = Arc::clone(&state);
        std::thread::spawn(move || {
            if let Err(e) = handle(stream, &state) {
                eprintln!("web serve: {e:#}");
            }
        });
    }
}

pub fn run(args: &Args) -> Result<()> {
    let state = Arc::new(State::load(&args.dist, &args.upstream)?);
    let listener = TcpListener::bind(("127.0.0.1", args.port))
        .with_context(|| format!("bind port {}", args.port))?;
    println!(
        "serving {} on :{}, data from {}",
        args.dist.display(),
        args.port,
        state.upstream
    );
    serve_on(&listener, state);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, immutable: bool, entry: bool) -> Entry {
        Entry {
            name: name.into(),
            content_type: "text/plain".into(),
            immutable,
            entry,
        }
    }

    fn fixture() -> (Vec<Entry>, BTreeSet<String>) {
        let manifest = vec![
            entry("index.html", false, true),
            entry("a.1234.js", true, false),
            entry("gone.js", true, false),
        ];
        let present = ["index.html", "a.1234.js", "manifest.json"]
            .map(String::from)
            .into_iter()
            .collect();
        (manifest, present)
    }

    #[test]
    fn api_and_texture_paths_go_upstream_with_their_query() {
        let (m, p) = fixture();
        assert_eq!(
            route("/v1/list?x=1", &m, &p),
            Route::Proxy("/v1/list?x=1".into())
        );
        assert_eq!(route("/tex/abc", &m, &p), Route::Proxy("/tex/abc".into()));
    }

    #[test]
    fn root_and_app_prefix_resolve_to_the_entry_and_names_to_files() {
        let (m, p) = fixture();
        for path in ["/", "/app/", "/index.html", "/app/index.html"] {
            assert!(
                matches!(route(path, &m, &p), Route::File(e) if e.name == "index.html"),
                "{path}"
            );
        }
        assert!(matches!(route("/a.1234.js?v=2", &m, &p), Route::File(e) if e.immutable));
        assert_eq!(route("/gone.js", &m, &p), Route::NotFound);
        assert_eq!(route("/manifest.json", &m, &p), Route::NotFound);
        assert_eq!(route("/../secret", &m, &p), Route::NotFound);
    }

    #[test]
    fn the_manifest_parses_the_pack_format() {
        let text = "{\"files\":[\n{\"name\":\"index.html\",\"type\":\"text/html\",\"immutable\":false,\"entry\":true,\"bytes\":3,\"brotli\":5,\"sha256\":\"x\"}\n]}\n";
        let entries = parse_manifest(text).unwrap();
        assert_eq!(
            entries,
            [Entry {
                name: "index.html".into(),
                content_type: "text/html".into(),
                immutable: false,
                entry: true
            }]
        );
        assert!(parse_manifest("{}").is_err());
    }

    fn get(state: &Arc<State>, request: &str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let st = Arc::clone(state);
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            handle(stream, &st).unwrap();
        });
        let mut client = TcpStream::connect(addr).unwrap();
        client.write_all(request.as_bytes()).unwrap();
        let mut out = String::new();
        client.read_to_string(&mut out).unwrap();
        server.join().unwrap();
        out
    }

    #[test]
    fn files_are_served_with_their_cache_headers_and_unknown_paths_are_404() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<p>hi</p>").unwrap();
        std::fs::write(dir.path().join("a.1234.js"), "x").unwrap();
        std::fs::write(
            dir.path().join("manifest.json"),
            "{\"files\":[\n{\"name\":\"index.html\",\"type\":\"text/html; charset=utf-8\",\"immutable\":false,\"entry\":true},\n{\"name\":\"a.1234.js\",\"type\":\"text/javascript\",\"immutable\":true,\"entry\":false}\n]}\n",
        )
        .unwrap();
        let state = Arc::new(State::load(dir.path(), "http://127.0.0.1:1/").unwrap());
        let page = get(&state, "GET / HTTP/1.1\r\nHost: x\r\n\r\n");
        assert!(page.starts_with("HTTP/1.1 200 OK\r\n"), "{page}");
        assert!(page.contains("cache-control: no-cache"));
        assert!(page.contains("content-type: text/html; charset=utf-8"));
        assert!(page.ends_with("<p>hi</p>"));
        let js = get(&state, "GET /app/a.1234.js HTTP/1.1\r\n\r\n");
        assert!(js.contains("cache-control: public, max-age=31536000, immutable"));
        assert!(get(&state, "GET /nope HTTP/1.1\r\n\r\n").starts_with("HTTP/1.1 404"));
        assert!(get(&state, "POST / HTTP/1.1\r\n\r\n").starts_with("HTTP/1.1 405"));
        let head = get(&state, "HEAD / HTTP/1.1\r\n\r\n");
        assert!(head.contains("content-length: 9") && !head.contains("<p>"));
    }
}
