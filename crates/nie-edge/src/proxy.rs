//! The pingora `ProxyHttp` implementation: routing, host check, limits, cache, health endpoint
//! and the access log.

use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use bytes::Bytes;
use pingora_cache::key::CacheKey;
use pingora_cache::{NoCacheReason, RespCacheable};
use pingora_core::upstreams::peer::HttpPeer;
use pingora_core::{Error, ErrorType, Result};
use pingora_http::{RequestHeader, ResponseHeader};
use pingora_load_balancing::LoadBalancer;
use pingora_load_balancing::selection::RoundRobin;
use pingora_proxy::{ProxyHttp, Session};
use serde_json::json;

use crate::cache::{self, CacheBackend};
use crate::limits::{Denied, Limits, Permit};
use crate::route::{self, Limited, Target};

/// A load-balanced upstream with its settings.
pub struct Pool {
    /// Backends and their health.
    pub lb: Arc<LoadBalancer<RoundRobin>>,
    /// The configured `host:port`, for the health report.
    pub addr: String,
    /// Upstream read timeout.
    pub read_timeout: Duration,
}

impl Pool {
    fn healthy(&self) -> bool {
        self.lb.select_with(b"", 4, |_, health| health).is_some()
    }
}

/// The edge service.
pub struct Edge {
    /// `nie serve`.
    pub core: Pool,
    /// `nie-model-serve`.
    pub media: Pool,
    /// Decode-heavy route limits.
    pub limits: Limits,
    /// Response cache, when enabled.
    pub cache: Option<&'static CacheBackend>,
    /// Host names accepted on TLS connections; empty accepts all.
    pub tls_hosts: Vec<String>,
    /// One JSON line per request on stdout.
    pub access_log: bool,
}

/// Per-request state.
pub struct Ctx {
    start: Instant,
    target: Option<Target>,
    client: String,
    tls: bool,
    limited: Option<Limited>,
    _permit: Option<Permit>,
    upstream: Option<String>,
}

impl Edge {
    fn pool(&self, target: Target) -> Option<&Pool> {
        match target {
            Target::Core => Some(&self.core),
            Target::Media => Some(&self.media),
            Target::Blocked | Target::Edge => None,
        }
    }

    fn health_json(&self) -> (u16, serde_json::Value) {
        let core = self.core.healthy();
        let media = self.media.healthy();
        let status = if core && media { 200 } else { 503 };
        let state = |ok: bool| if ok { "up" } else { "down" };
        (
            status,
            json!({
                "ok": core && media,
                "service": "nie-edge",
                "version": env!("CARGO_PKG_VERSION"),
                "upstreams": {
                    "core": {"addr": self.core.addr, "state": state(core)},
                    "media": {"addr": self.media.addr, "state": state(media)},
                },
            }),
        )
    }
}

async fn reply(
    session: &mut Session,
    status: u16,
    body: &serde_json::Value,
    headers: &[(&'static str, &str)],
    head_only: bool,
) -> Result<()> {
    let bytes = Bytes::from(serde_json::to_vec(body).unwrap_or_default());
    let mut header = ResponseHeader::build(status, Some(4))?;
    header.insert_header("Content-Type", "application/json")?;
    header.insert_header("Content-Length", bytes.len().to_string())?;
    header.insert_header("Cache-Control", "no-store")?;
    header.insert_header("X-Content-Type-Options", "nosniff")?;
    for (name, value) in headers {
        header.insert_header(*name, *value)?;
    }
    session.set_keepalive(None);
    session
        .write_response_header(Box::new(header), head_only)
        .await?;
    if !head_only {
        session.write_response_body(Some(bytes), true).await?;
    }
    Ok(())
}

fn header_str<'a>(req: &'a RequestHeader, name: &str) -> Option<&'a str> {
    req.headers.get(name).and_then(|v| v.to_str().ok())
}

fn client_ip(session: &Session, tls: bool) -> String {
    if tls
        && let Some(ip) = header_str(session.req_header(), "cf-connecting-ip")
        && ip.parse::<std::net::IpAddr>().is_ok()
    {
        return ip.to_owned();
    }
    session
        .client_addr()
        .and_then(|a| a.as_inet())
        .map_or_else(|| "unknown".into(), |a| a.ip().to_string())
}

fn request_host(req: &RequestHeader) -> Option<String> {
    let raw = header_str(req, "host").or_else(|| req.uri.authority().map(|a| a.as_str()))?;
    let host = raw.rsplit_once(':').map_or(raw, |(h, port)| {
        if port.bytes().all(|b| b.is_ascii_digit()) {
            h
        } else {
            raw
        }
    });
    Some(host.trim_end_matches('.').to_ascii_lowercase())
}

#[async_trait]
impl ProxyHttp for Edge {
    type CTX = Ctx;

    fn new_ctx(&self) -> Ctx {
        Ctx {
            start: Instant::now(),
            target: None,
            client: String::new(),
            tls: false,
            limited: None,
            _permit: None,
            upstream: None,
        }
    }

    async fn request_filter(&self, session: &mut Session, ctx: &mut Ctx) -> Result<bool> {
        let (path, query, method, host) = {
            let req = session.req_header();
            (
                req.uri.path().to_owned(),
                req.uri.query().map(str::to_owned),
                req.method.as_str().to_owned(),
                request_host(req),
            )
        };
        ctx.tls = session.digest().is_some_and(|d| d.ssl_digest.is_some());
        ctx.client = client_ip(session, ctx.tls);
        let head = method == "HEAD";

        if ctx.tls
            && !self.tls_hosts.is_empty()
            && !host.is_some_and(|h| self.tls_hosts.iter().any(|a| a.eq_ignore_ascii_case(&h)))
        {
            reply(
                session,
                421,
                &json!({"error": "misdirected request"}),
                &[],
                head,
            )
            .await?;
            return Ok(true);
        }

        let target = route::classify(&path);
        ctx.target = Some(target);
        match target {
            Target::Blocked => {
                reply(session, 404, &json!({"error": "not found"}), &[], head).await?;
                return Ok(true);
            }
            Target::Edge => {
                let (status, body) = self.health_json();
                reply(session, status, &body, &[], head).await?;
                return Ok(true);
            }
            Target::Media if !matches!(method.as_str(), "GET" | "HEAD" | "OPTIONS") => {
                reply(
                    session,
                    405,
                    &json!({"error": "method not allowed"}),
                    &[("Allow", "GET, HEAD, OPTIONS")],
                    head,
                )
                .await?;
                return Ok(true);
            }
            Target::Core | Target::Media => {}
        }

        if let Some(family) = route::limited(&path, query.as_deref()) {
            ctx.limited = Some(family);
            match self.limits.acquire(family, &ctx.client) {
                Ok(permit) => ctx._permit = Some(permit),
                Err(Denied::Rate) => {
                    reply(
                        session,
                        429,
                        &json!({"error": "rate limited", "family": family.name()}),
                        &[("Retry-After", "1")],
                        head,
                    )
                    .await?;
                    return Ok(true);
                }
                Err(Denied::Busy) => {
                    reply(
                        session,
                        503,
                        &json!({"error": "busy", "family": family.name()}),
                        &[("Retry-After", "1")],
                        head,
                    )
                    .await?;
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    async fn upstream_peer(&self, _session: &mut Session, ctx: &mut Ctx) -> Result<Box<HttpPeer>> {
        let pool = ctx
            .target
            .and_then(|t| self.pool(t))
            .ok_or_else(|| Error::explain(ErrorType::InternalError, "no upstream for target"))?;
        let backend = pool
            .lb
            .select(b"", 8)
            .ok_or_else(|| Error::explain(ErrorType::HTTPStatus(503), "no healthy upstream"))?;
        ctx.upstream = Some(backend.addr.to_string());
        let mut peer = HttpPeer::new(backend, false, String::new());
        peer.options.connection_timeout = Some(Duration::from_secs(3));
        peer.options.read_timeout = Some(pool.read_timeout);
        Ok(Box::new(peer))
    }

    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut Ctx,
    ) -> Result<()> {
        upstream_request.insert_header("X-Forwarded-For", ctx.client.as_str())?;
        upstream_request.insert_header("X-Real-IP", ctx.client.as_str())?;
        upstream_request
            .insert_header("X-Forwarded-Proto", if ctx.tls { "https" } else { "http" })?;
        Ok(())
    }

    fn request_cache_filter(&self, session: &mut Session, ctx: &mut Ctx) -> Result<()> {
        let Some(cache) = self.cache else {
            return Ok(());
        };
        let req = session.req_header();
        let cacheable_request = req.method.as_str() == "GET"
            && !req.headers.contains_key("range")
            && !req.headers.contains_key("authorization")
            && matches!(ctx.target, Some(Target::Core | Target::Media));
        if cacheable_request {
            session.cache.enable(
                cache.storage,
                Some(cache.eviction),
                Some(cache.predictor),
                Some(cache.lock),
                None,
            );
            session.cache.set_max_file_size_bytes(cache.max_object);
        }
        Ok(())
    }

    fn cache_key_callback(&self, session: &Session, _ctx: &mut Ctx) -> Result<CacheKey> {
        let uri = &session.req_header().uri;
        let key = uri
            .path_and_query()
            .map_or_else(|| uri.path().to_owned(), |pq| pq.as_str().to_owned());
        Ok(CacheKey::new(key.into_bytes(), ""))
    }

    fn response_cache_filter(
        &self,
        _session: &Session,
        resp: &ResponseHeader,
        _ctx: &mut Ctx,
    ) -> Result<RespCacheable> {
        Ok(match cache::decide(resp) {
            RespCacheable::Cacheable(meta) => RespCacheable::Cacheable(meta),
            RespCacheable::Uncacheable(_) => {
                RespCacheable::Uncacheable(NoCacheReason::OriginNotCache)
            }
        })
    }

    async fn response_filter(
        &self,
        session: &mut Session,
        upstream_response: &mut ResponseHeader,
        _ctx: &mut Ctx,
    ) -> Result<()> {
        if session.cache.enabled() {
            upstream_response.insert_header("X-Nie-Cache", session.cache.phase().as_str())?;
        }
        Ok(())
    }

    async fn logging(&self, session: &mut Session, error: Option<&Error>, ctx: &mut Ctx) {
        if !self.access_log {
            return;
        }
        let status = session.response_written().map_or(0, |r| r.status.as_u16());
        let req = session.req_header();
        let line = json!({
            "ts": SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64),
            "client": ctx.client,
            "tls": ctx.tls,
            "method": req.method.as_str(),
            "uri": req.uri.path_and_query().map_or_else(|| req.uri.path(), |p| p.as_str()),
            "status": status,
            "bytes": session.body_bytes_sent(),
            "ms": ctx.start.elapsed().as_millis() as u64,
            "target": ctx.target.map(|t| format!("{t:?}").to_ascii_lowercase()),
            "upstream": ctx.upstream,
            "limit": ctx.limited.map(Limited::name),
            "cache": session.cache.enabled().then(|| session.cache.phase().as_str()),
            "error": error.map(ToString::to_string),
        });
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{line}");
    }
}
