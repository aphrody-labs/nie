//! Self-hosted cloud-first HTTP server for `nie` (`GET /health`, `/ready`, `/v1/vfs/*`, `/v1/formats/*`, `/v1/game/query`).

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context;
use clap::Args;
use nie_explore::raw_access::{self, Token};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;
const MAX_FIND_LIMIT: usize = 500;

#[derive(Debug, Args)]
pub struct ServeArgs {
    /// Address to bind (`127.0.0.1` by default; `0.0.0.0` inside Docker/Kubernetes).
    #[arg(long, env = "NIE_HOST", default_value = "127.0.0.1")]
    pub host: String,
    /// TCP port to listen on.
    #[arg(long, env = "NIE_PORT", default_value_t = 8790)]
    pub port: u16,
    /// Optional local directory or remote URI (`r2://...`, `ievr-vfs:`, `s3://...`, `https://...`).
    #[arg(long, env = "NIE_GAME_SOURCE")]
    pub source: Option<String>,
    /// Fail startup immediately if the VFS source cannot be mounted.
    #[arg(long)]
    pub strict: bool,
    /// Mark `/ready` healthy even without a mounted CPK VFS (stateless format/remote worker mode).
    #[arg(long)]
    pub stateless: bool,
}

#[derive(Clone)]
struct ServerState {
    vfs: Option<Arc<nie_formats::vfs::Vfs>>,
    source_label: Option<String>,
    vfs_error: Option<String>,
    stateless: bool,
    /// Bearer token opening `/v1/vfs/read` in its untouched (raw) mode; closed without one.
    raw_token: Option<Token>,
}

impl ServerState {
    fn is_ready(&self) -> bool {
        self.vfs.is_some() || self.stateless
    }
}

pub fn run(args: ServeArgs) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("build tokio runtime for `nie serve`")?;
    runtime.block_on(serve(args))
}

async fn serve(args: ServeArgs) -> anyhow::Result<()> {
    let source_arg = args
        .source
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);

    let (vfs, source_label, vfs_error) = match nie_formats::vfs::open_source(source_arg.as_deref())
    {
        Ok(mounted) => {
            let resolved = nie_formats::game_source::resolve(source_arg.as_deref())
                .or_else(|_| {
                    Ok::<String, nie_formats::FormatError>(
                        source_arg.clone().unwrap_or_else(|| "auto".to_owned()),
                    )
                })
                .ok();
            tracing::info!(
                entries = mounted.asset_count(),
                cpks = mounted.cpk_count(),
                source = ?resolved,
                "nie serve: VFS mounted"
            );
            (Some(Arc::new(mounted)), resolved, None)
        }
        Err(err) => {
            if args.strict {
                return Err(anyhow::anyhow!("--strict VFS mount failed: {err}"));
            }
            tracing::warn!("nie serve: starting without VFS ({err})");
            (None, source_arg, Some(err.to_string()))
        }
    };

    // The game's own bytes stay closed unless a token of at least 32 bytes is configured, the
    // same rule as `nie-model-serve` (`nie_explore::raw_access`).
    let raw_token = match Token::from_env() {
        Ok(token) => token,
        Err(err) => {
            tracing::warn!("nie serve: {err}");
            None
        }
    };
    if raw_token.is_none() {
        tracing::info!(
            "nie serve: raw /v1/vfs/read closed (no {})",
            raw_access::TOKEN_ENV
        );
    }

    let state = Arc::new(ServerState {
        vfs,
        source_label,
        vfs_error,
        stateless: args.stateless,
        raw_token,
    });

    let bind_addr = format!("{}:{}", args.host, args.port);
    let listener = TcpListener::bind(&bind_addr)
        .await
        .with_context(|| format!("bind {bind_addr}"))?;
    eprintln!(
        "nie serve listening on http://{bind_addr} (vfs_ready={}, stateless={})",
        state.vfs.is_some(),
        state.stateless
    );

    loop {
        let (stream, _) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_connection(stream, state).await {
                tracing::debug!("http connection error: {err:#}");
            }
        });
    }
}

struct HttpResponse {
    status: u16,
    reason: &'static str,
    content_type: &'static str,
    cache_control: &'static str,
    body: Vec<u8>,
}

impl HttpResponse {
    fn json(status: u16, reason: &'static str, value: &Value) -> Self {
        Self {
            status,
            reason,
            content_type: "application/json; charset=utf-8",
            cache_control: "no-store",
            body: serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec()),
        }
    }

    fn bytes(
        status: u16,
        reason: &'static str,
        content_type: &'static str,
        cache_control: &'static str,
        body: Vec<u8>,
    ) -> Self {
        Self {
            status,
            reason,
            content_type,
            cache_control,
            body,
        }
    }

    fn into_wire(self, head_only: bool) -> Vec<u8> {
        let header = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: Authorization, Content-Type\r\nAccess-Control-Allow-Methods: GET, HEAD, POST, OPTIONS\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
            self.status,
            self.reason,
            self.content_type,
            self.body.len(),
            self.cache_control
        );
        let mut out =
            Vec::with_capacity(header.len() + if head_only { 0 } else { self.body.len() });
        out.extend_from_slice(header.as_bytes());
        if !head_only {
            out.extend_from_slice(&self.body);
        }
        out
    }
}

async fn handle_connection(mut stream: TcpStream, state: Arc<ServerState>) -> anyhow::Result<()> {
    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0_u8; 4096];

    let header_end = loop {
        if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
            break pos;
        }
        if buf.len() >= MAX_HEADER_BYTES {
            let resp = HttpResponse::json(
                431,
                "Request Header Fields Too Large",
                &json!({ "error": "request headers exceed 16 KiB" }),
            );
            stream.write_all(&resp.into_wire(false)).await?;
            return Ok(());
        }
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
    };

    let head_bytes = &buf[..header_end];
    let head_str = String::from_utf8_lossy(head_bytes);
    let mut lines = head_str.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");

    let mut headers = HashMap::new();
    for line in lines {
        if let Some((name, val)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), val.trim().to_owned());
        }
    }

    let content_length: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if content_length > MAX_BODY_BYTES {
        let resp = HttpResponse::json(
            413,
            "Payload Too Large",
            &json!({ "error": "request body exceeds 32 MiB" }),
        );
        stream.write_all(&resp.into_wire(false)).await?;
        return Ok(());
    }

    let body_start = header_end + 4;
    let mut body = buf[body_start..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        let need = (content_length - body.len()).min(n);
        body.extend_from_slice(&chunk[..need]);
    }
    if body.len() > content_length {
        body.truncate(content_length);
    }

    let head_only = method.eq_ignore_ascii_case("HEAD");
    let response = if method.eq_ignore_ascii_case("OPTIONS") {
        HttpResponse::bytes(
            204,
            "No Content",
            "text/plain; charset=utf-8",
            "no-store",
            Vec::new(),
        )
    } else {
        route_request(method, target, &headers, body, state).await
    };

    stream.write_all(&response.into_wire(head_only)).await?;
    Ok(())
}

async fn route_request(
    method: &str,
    target: &str,
    headers: &HashMap<String, String>,
    body: Vec<u8>,
    state: Arc<ServerState>,
) -> HttpResponse {
    let (raw_path, raw_query) = target.split_once('?').unwrap_or((target, ""));
    let query = parse_query(raw_query);

    if (method == "GET" || method == "HEAD") && (raw_path == "/health" || raw_path == "/") {
        let profile = if cfg!(feature = "host") {
            "host"
        } else {
            "cloud"
        };
        let (entries, cpks) = state
            .vfs
            .as_ref()
            .map(|v| (v.asset_count(), v.cpk_count()))
            .unwrap_or((0, 0));
        return HttpResponse::json(
            200,
            "OK",
            &json!({
                "ok": true,
                "service": "nie",
                "profile": profile,
                "vfsReady": state.vfs.is_some(),
                "stateless": state.stateless,
                "source": state.source_label,
                "vfsError": state.vfs_error,
                "entries": entries,
                "cpks": cpks,
            }),
        );
    }

    if (method == "GET" || method == "HEAD") && raw_path == "/ready" {
        if state.is_ready() {
            return HttpResponse::json(
                200,
                "OK",
                &json!({
                    "ready": true,
                    "vfsReady": state.vfs.is_some(),
                    "stateless": state.stateless,
                }),
            );
        }
        return HttpResponse::json(
            503,
            "Service Unavailable",
            &json!({
                "ready": false,
                "vfsReady": false,
                "reason": state.vfs_error.as_deref().unwrap_or("VFS not mounted"),
            }),
        );
    }

    if method == "GET" && raw_path == "/v1/vfs/remotes" {
        return tokio::task::spawn_blocking(
            || match nie_formats::vfs_remote::rclone_list_remotes() {
                Ok(raw) => {
                    let parsed: Value = serde_json::from_str(&raw).unwrap_or(Value::String(raw));
                    HttpResponse::json(200, "OK", &json!({ "remotes": parsed }))
                }
                Err(err) => HttpResponse::json(
                    503,
                    "Service Unavailable",
                    &json!({ "error": err.to_string() }),
                ),
            },
        )
        .await
        .unwrap_or_else(|_| {
            HttpResponse::json(
                500,
                "Internal Server Error",
                &json!({ "error": "task failed" }),
            )
        });
    }

    if method == "POST" && raw_path == "/v1/formats/detect" {
        let fmt = nie_formats::detect(&body);
        return HttpResponse::json(
            200,
            "OK",
            &json!({
                "format": fmt.name(),
                "size": body.len(),
            }),
        );
    }

    if method == "POST" && raw_path == "/v1/formats/decode" {
        let name = query
            .get("name")
            .cloned()
            .unwrap_or_else(|| "asset".to_owned());
        if let Some(ext) = query.get("format").map(|s| s.to_ascii_lowercase())
            && ext == "png"
        {
            return match nie_formats::g4tx_decode::decode_best_to_png(&body, &name) {
                Some(png) => {
                    HttpResponse::bytes(200, "OK", "image/png", "public, max-age=300", png)
                }
                None => HttpResponse::json(
                    422,
                    "Unprocessable Entity",
                    &json!({ "error": "payload is not a decodable G4TX texture" }),
                ),
            };
        }
        return match nie_formats::decode::decode(&body) {
            Some(decoded) => HttpResponse::bytes(
                200,
                "OK",
                "application/json; charset=utf-8",
                "no-store",
                decoded.json,
            ),
            None => HttpResponse::json(
                422,
                "Unprocessable Entity",
                &json!({ "error": "unsupported or unreadable format" }),
            ),
        };
    }

    let Some(vfs) = state.vfs.clone() else {
        return HttpResponse::json(
            503,
            "Service Unavailable",
            &json!({
                "error": "VFS is not mounted",
                "reason": state.vfs_error.as_deref().unwrap_or("no game source configured"),
            }),
        );
    };

    if method == "GET" && raw_path == "/v1/vfs/info" {
        let (io_calls, payload_bytes) = vfs.remote_stats().unwrap_or_default();
        return HttpResponse::json(
            200,
            "OK",
            &json!({
                "source": state.source_label,
                "entries": vfs.asset_count(),
                "cpks": vfs.cpk_count(),
                "transport": {
                    "ioCalls": io_calls,
                    "payloadBytes": payload_bytes,
                }
            }),
        );
    }

    if method == "GET" && raw_path == "/v1/vfs/find" {
        let q = query.get("q").map(String::as_str).unwrap_or("");
        let ext = query
            .get("ext")
            .map(|e| format!(".{}", e.trim_start_matches('.').to_ascii_lowercase()));
        let limit = query
            .get("limit")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(50)
            .clamp(1, MAX_FIND_LIMIT);
        let q_lower = q.to_ascii_lowercase();
        return tokio::task::spawn_blocking(move || {
            let mut matches = Vec::new();
            for (path, entry) in vfs.iter() {
                if !q_lower.is_empty() && !path.to_ascii_lowercase().contains(&q_lower) {
                    continue;
                }
                if let Some(ref ext_dot) = ext
                    && !path.to_ascii_lowercase().ends_with(ext_dot)
                {
                    continue;
                }
                matches.push(json!({
                    "path": path,
                    "size": entry.file_size,
                    "cpk": entry.cpk_filename,
                }));
                if matches.len() >= limit {
                    break;
                }
            }
            HttpResponse::json(
                200,
                "OK",
                &json!({ "count": matches.len(), "items": matches }),
            )
        })
        .await
        .unwrap_or_else(|_| {
            HttpResponse::json(
                500,
                "Internal Server Error",
                &json!({ "error": "task failed" }),
            )
        });
    }

    if method == "GET" && raw_path == "/v1/vfs/stat" {
        let Some(raw_vfs_path) = query.get("path") else {
            return HttpResponse::json(
                400,
                "Bad Request",
                &json!({ "error": "missing path query param" }),
            );
        };
        let Ok(clean_path) = sanitize_vfs_path(raw_vfs_path) else {
            return HttpResponse::json(400, "Bad Request", &json!({ "error": "invalid VFS path" }));
        };
        return tokio::task::spawn_blocking(move || match vfs.find(&clean_path) {
            Some(entry) => HttpResponse::json(
                200,
                "OK",
                &json!({
                    "path": clean_path,
                    "size": entry.file_size,
                    "cpk": entry.cpk_filename,
                }),
            ),
            None => HttpResponse::json(404, "Not Found", &json!({ "error": "entry not found" })),
        })
        .await
        .unwrap_or_else(|_| {
            HttpResponse::json(
                500,
                "Internal Server Error",
                &json!({ "error": "task failed" }),
            )
        });
    }

    if method == "GET" && raw_path == "/v1/vfs/read" {
        // `decode=json|png` is derived data and stays public; any other mode returns the game's
        // own bytes and needs the configured bearer token. Refusals answer 404 so the origin
        // does not advertise a private space.
        let derived = matches!(
            query.get("decode").map(String::as_str),
            Some("json" | "png")
        );
        if !derived
            && !raw_access::allows(
                state.raw_token.as_ref(),
                headers.get("authorization").map(String::as_str),
            )
        {
            return HttpResponse::json(404, "Not Found", &json!({ "error": "route not found" }));
        }
        let Some(raw_vfs_path) = query.get("path") else {
            return HttpResponse::json(
                400,
                "Bad Request",
                &json!({ "error": "missing path query param" }),
            );
        };
        let Ok(clean_path) = sanitize_vfs_path(raw_vfs_path) else {
            return HttpResponse::json(400, "Bad Request", &json!({ "error": "invalid VFS path" }));
        };
        let decode_mode = query
            .get("decode")
            .map(String::as_str)
            .unwrap_or("raw")
            .to_owned();
        return tokio::task::spawn_blocking(move || match vfs.read(&clean_path) {
            Ok(bytes) => {
                if decode_mode == "json" {
                    return match nie_formats::decode::decode(&bytes) {
                        Some(d) => HttpResponse::bytes(
                            200,
                            "OK",
                            "application/json; charset=utf-8",
                            "public, max-age=300",
                            d.json,
                        ),
                        None => HttpResponse::json(
                            422,
                            "Unprocessable Entity",
                            &json!({ "error": "cannot decode entry as JSON" }),
                        ),
                    };
                }
                if decode_mode == "png" {
                    let stem = clean_path
                        .rsplit('/')
                        .next()
                        .unwrap_or("asset")
                        .split('.')
                        .next()
                        .unwrap_or("asset");
                    return match nie_formats::g4tx_decode::decode_best_to_png(&bytes, stem) {
                        Some(png) => {
                            HttpResponse::bytes(200, "OK", "image/png", "public, max-age=300", png)
                        }
                        None => HttpResponse::json(
                            422,
                            "Unprocessable Entity",
                            &json!({ "error": "cannot decode entry as PNG" }),
                        ),
                    };
                }
                HttpResponse::bytes(
                    200,
                    "OK",
                    "application/octet-stream",
                    "public, max-age=3600",
                    bytes,
                )
            }
            Err(err) => HttpResponse::json(
                404,
                "Not Found",
                &json!({ "error": err.to_string(), "path": clean_path }),
            ),
        })
        .await
        .unwrap_or_else(|_| {
            HttpResponse::json(
                500,
                "Internal Server Error",
                &json!({ "error": "task failed" }),
            )
        });
    }

    if method == "GET" && raw_path == "/v1/game/query" {
        let domain = query
            .get("domain")
            .cloned()
            .unwrap_or_else(|| "characters".to_owned());
        let q = query.get("q").cloned().unwrap_or_default();
        let limit = query
            .get("limit")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(50)
            .clamp(1, MAX_FIND_LIMIT);
        let req = json!({
            "domain": domain,
            "query": q,
            "limit": limit,
        });
        return tokio::task::spawn_blocking(move || {
            match nie_explore::domain_data::query_vfs(&vfs, &req) {
                Ok(res) => HttpResponse::json(200, "OK", &res),
                Err(err) => HttpResponse::json(400, "Bad Request", &json!({ "error": err })),
            }
        })
        .await
        .unwrap_or_else(|_| {
            HttpResponse::json(
                500,
                "Internal Server Error",
                &json!({ "error": "task failed" }),
            )
        });
    }

    HttpResponse::json(404, "Not Found", &json!({ "error": "route not found" }))
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn parse_query(raw: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in raw.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        map.insert(percent_decode(k), percent_decode(v));
    }
    map
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3])
                    && let Ok(val) = u8::from_str_radix(hex, 16)
                {
                    out.push(val);
                    i += 3;
                    continue;
                }
                out.push(b'%');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub(crate) fn sanitize_vfs_path(path: &str) -> Result<String, &'static str> {
    let trimmed = path.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('/')
        || trimmed.contains('\\')
        || trimmed.contains('\0')
        || trimmed.split('/').any(|seg| seg == ".." || seg.is_empty())
    {
        return Err("invalid VFS path");
    }
    Ok(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    fn state(stateless: bool, raw_token: Option<&str>) -> Arc<ServerState> {
        Arc::new(ServerState {
            vfs: None,
            source_label: None,
            vfs_error: Some("no source in unit test".to_owned()),
            stateless,
            raw_token: raw_token.and_then(Token::new),
        })
    }

    /// A VFS holding one loose synthetic file, mounted as an overlay: no game data involved.
    fn synthetic_state(dir: &std::path::Path, raw_token: Option<&str>) -> Arc<ServerState> {
        std::fs::create_dir_all(dir.join("data/common")).expect("create synthetic tree");
        std::fs::write(dir.join("data/common/probe.bin"), b"synthetic-probe")
            .expect("write synthetic file");
        let mut vfs = nie_formats::vfs::Vfs::new();
        assert_eq!(vfs.mount_overlay_dir(dir, ""), 1);
        Arc::new(ServerState {
            vfs: Some(Arc::new(vfs)),
            source_label: Some("synthetic".to_owned()),
            vfs_error: None,
            stateless: false,
            raw_token: raw_token.and_then(Token::new),
        })
    }

    #[tokio::test]
    async fn health_and_readiness_reflect_stateless_flag() {
        let headers = HashMap::new();
        let h = route_request("GET", "/health", &headers, Vec::new(), state(false, None)).await;
        assert_eq!(h.status, 200);

        let r_unready =
            route_request("GET", "/ready", &headers, Vec::new(), state(false, None)).await;
        assert_eq!(r_unready.status, 503);

        let r_ready = route_request("GET", "/ready", &headers, Vec::new(), state(true, None)).await;
        assert_eq!(r_ready.status, 200);
    }

    #[tokio::test]
    async fn format_detect_identifies_g4tx_and_rejects_traversal() {
        let headers = HashMap::new();
        let resp = route_request(
            "POST",
            "/v1/formats/detect",
            &headers,
            b"G4TX\x00\x00\x00\x00".to_vec(),
            state(true, None),
        )
        .await;
        assert_eq!(resp.status, 200);
        let parsed: Value = serde_json::from_slice(&resp.body).expect("json body");
        assert_eq!(parsed["format"], "G4TX");
        assert_eq!(parsed["size"], 8);

        assert!(sanitize_vfs_path("data/common/text/fr.cfg.bin").is_ok());
        assert!(sanitize_vfs_path("../secret").is_err());
        assert!(sanitize_vfs_path("/abs/path").is_err());
        assert!(sanitize_vfs_path("data\\win").is_err());
    }

    #[tokio::test]
    async fn raw_vfs_read_is_closed_without_a_token_and_open_with_it() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = "/v1/vfs/read?path=data/common/probe.bin";

        // No token configured: closed for everyone, even with a header.
        let closed = synthetic_state(dir.path(), None);
        let mut headers = HashMap::new();
        headers.insert("authorization".to_owned(), format!("Bearer {TOKEN}"));
        let resp = route_request("GET", path, &headers, Vec::new(), closed).await;
        assert_eq!(resp.status, 404);

        // Token configured: missing and wrong credentials stay refused.
        let gated = synthetic_state(dir.path(), Some(TOKEN));
        let resp = route_request("GET", path, &HashMap::new(), Vec::new(), gated.clone()).await;
        assert_eq!(resp.status, 404);
        let mut wrong = HashMap::new();
        wrong.insert("authorization".to_owned(), "Bearer wrong".to_owned());
        let resp = route_request("GET", path, &wrong, Vec::new(), gated.clone()).await;
        assert_eq!(resp.status, 404);

        // Exact token: the bytes come back untouched.
        let resp = route_request("GET", path, &headers, Vec::new(), gated.clone()).await;
        assert_eq!(resp.status, 200);
        assert_eq!(resp.content_type, "application/octet-stream");
        assert_eq!(resp.body, b"synthetic-probe");

        // A derived mode is public: it reaches the decoder (422 on a non-texture), not the gate.
        let derived = route_request(
            "GET",
            "/v1/vfs/read?path=data/common/probe.bin&decode=png",
            &HashMap::new(),
            Vec::new(),
            gated,
        )
        .await;
        assert_eq!(derived.status, 422);
    }

    #[tokio::test]
    async fn vfs_read_rejects_traversal_and_unknown_paths() {
        let dir = tempfile::tempdir().expect("temp dir");
        let st = synthetic_state(dir.path(), Some(TOKEN));
        let mut headers = HashMap::new();
        headers.insert("authorization".to_owned(), format!("Bearer {TOKEN}"));
        let resp = route_request(
            "GET",
            "/v1/vfs/read?path=../secret",
            &headers,
            Vec::new(),
            st.clone(),
        )
        .await;
        assert_eq!(resp.status, 400);
        let resp = route_request(
            "GET",
            "/v1/vfs/read?path=data/common/missing.bin",
            &headers,
            Vec::new(),
            st,
        )
        .await;
        assert_eq!(resp.status, 404);
    }
}
