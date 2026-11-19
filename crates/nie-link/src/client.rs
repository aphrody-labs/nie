//! Talking to page servers: one [`Upstream`] per host and a [`Router`] that sends each page request
//! to the host that holds the page and answers fastest.
use crate::{Manifest, PageId, hex};
use std::sync::Mutex;
use std::sync::RwLock;
use std::time::{Duration, Instant};

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("{0}: not found")]
    NotFound(String),
    #[error("{0}: HTTP {1}")]
    Status(String, u16),
    #[error("{0}: {1}")]
    Transport(String, String),
    #[error("{0}: page {1} failed its hash check")]
    BadHash(String, String),
    #[error("no host could serve page {0}: {1}")]
    Exhausted(String, String),
}

/// One page server and the credential for it.
pub struct Upstream {
    pub name: String,
    pub base: String,
    token: String,
    agent: ureq::Agent,
}

impl Upstream {
    #[must_use]
    pub fn new(name: &str, base: &str, token: &str) -> Self {
        let agent = ureq::Agent::config_builder()
            .proxy(None)
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(5)))
            .timeout_global(Some(Duration::from_secs(120)))
            .max_idle_connections_per_host(64)
            .max_idle_age(Duration::from_secs(30))
            .build()
            .new_agent();
        Self {
            name: name.to_owned(),
            base: base.trim_end_matches('/').to_owned(),
            token: token.to_owned(),
            agent,
        }
    }

    /// GET `path`; returns the body and the time until the response headers arrived.
    pub fn get(&self, path: &str) -> Result<(Vec<u8>, Duration), FetchError> {
        let t0 = Instant::now();
        let mut resp = self
            .agent
            .get(format!("{}{path}", self.base))
            .header("Authorization", format!("Bearer {}", self.token))
            .call()
            .map_err(|e| FetchError::Transport(self.name.clone(), e.to_string()))?;
        let ttfb = t0.elapsed();
        match resp.status().as_u16() {
            200 => {}
            404 => return Err(FetchError::NotFound(self.name.clone())),
            s => return Err(FetchError::Status(self.name.clone(), s)),
        }
        let body = resp
            .body_mut()
            .with_config()
            .limit(256 << 20)
            .read_to_vec()
            .map_err(|e| FetchError::Transport(self.name.clone(), e.to_string()))?;
        Ok((body, ttfb))
    }

    /// Fetch a page and check it against `id`.
    pub fn page(&self, id: &PageId) -> Result<(Vec<u8>, Duration), FetchError> {
        let (bytes, ttfb) = self.get(&format!("/v1/page/{}", hex(id)))?;
        if blake3::hash(&bytes).as_bytes() != id {
            return Err(FetchError::BadHash(self.name.clone(), hex(id)));
        }
        Ok((bytes, ttfb))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Owns the bytes; every page is there.
    Origin,
    /// Read-through tier; holds the pages listed in its `/v1/held` bitmap.
    Cache,
}

#[derive(Debug)]
struct Stats {
    rtt_ms: f64,
    bytes_per_s: f64,
    fails: u32,
    down_until: Option<Instant>,
    served: u64,
}

pub struct Host {
    pub up: Upstream,
    pub role: Role,
    /// Send pages that no tier holds through this cache so it fills.
    pub fill: bool,
    stats: Mutex<Stats>,
    held: RwLock<Option<Vec<u8>>>,
}

impl Host {
    #[must_use]
    pub fn new(up: Upstream, role: Role, fill: bool) -> Self {
        Self {
            up,
            role,
            fill,
            stats: Mutex::new(Stats {
                rtt_ms: 50.0,
                bytes_per_s: 20e6,
                fails: 0,
                down_until: None,
                served: 0,
            }),
            held: RwLock::new(None),
        }
    }

    fn stats(&self) -> std::sync::MutexGuard<'_, Stats> {
        self.stats.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn holds(&self, ordinal: u32) -> bool {
        match self.role {
            Role::Origin => true,
            Role::Cache => self
                .held
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref()
                .is_some_and(|b| {
                    b.get(ordinal as usize / 8).is_some_and(|byte| byte >> (ordinal % 8) & 1 == 1)
                }),
        }
    }

    fn cost_ms(&self, len: usize) -> f64 {
        let s = self.stats();
        s.rtt_ms + len as f64 / s.bytes_per_s * 1000.0
    }

    fn is_down(&self) -> bool {
        self.stats().down_until.is_some_and(|t| t > Instant::now())
    }

    fn ok(&self, ttfb: Duration, total: Duration, len: usize) {
        let mut s = self.stats();
        s.fails = 0;
        s.down_until = None;
        s.served += 1;
        let a = 0.2;
        s.rtt_ms = s.rtt_ms * (1.0 - a) + ttfb.as_secs_f64() * 1000.0 * a;
        let body_s = total.saturating_sub(ttfb).as_secs_f64();
        if len >= 64 * 1024 && body_s > 0.0 {
            s.bytes_per_s = s.bytes_per_s * (1.0 - a) + len as f64 / body_s * a;
        }
    }

    fn fail(&self) {
        let mut s = self.stats();
        s.fails += 1;
        let secs = (2u64 << s.fails.min(6)).min(120);
        s.down_until = Some(Instant::now() + Duration::from_secs(secs));
    }

    /// Replace the held-page bitmap from `/v1/held`.
    pub fn refresh_held(&self, manifest_id: &str) {
        if self.role != Role::Cache {
            return;
        }
        let bits = self
            .up
            .get(&format!("/v1/held?manifest={manifest_id}"))
            .ok()
            .map(|(b, _)| b);
        *self.held.write().unwrap_or_else(std::sync::PoisonError::into_inner) = bits;
    }

    /// Round-trip time of `/health`, in milliseconds.
    pub fn ping(&self) -> Option<f64> {
        let t0 = Instant::now();
        self.up.get("/health").ok()?;
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        self.stats().rtt_ms = ms;
        Some(ms)
    }

    #[must_use]
    pub fn summary(&self) -> serde_json::Value {
        let s = self.stats();
        serde_json::json!({
            "name": self.up.name, "url": self.up.base, "role": self.role,
            "rttMs": (s.rtt_ms * 10.0).round() / 10.0,
            "mbPerS": (s.bytes_per_s / 1e5).round() / 10.0,
            "served": s.served, "fails": s.fails,
        })
    }
}

/// Cache-aware routing: candidates are the hosts that hold the page (an origin always does), ordered by
/// predicted time; a cache with `fill` is also a candidate for pages nobody holds, so it warms.
pub struct Router {
    pub hosts: Vec<Host>,
}

impl Router {
    #[must_use]
    pub fn new(hosts: Vec<Host>) -> Self {
        Self { hosts }
    }

    /// Download the manifest from the first origin that answers (any host as a last resort).
    pub fn manifest(&self) -> Result<Manifest, FetchError> {
        let mut last = FetchError::Exhausted("manifest".into(), "no host configured".into());
        let order = self
            .hosts
            .iter()
            .filter(|h| h.role == Role::Origin)
            .chain(self.hosts.iter().filter(|h| h.role == Role::Cache));
        for h in order {
            match h.up.get("/v1/manifest") {
                Ok((body, _)) => match Manifest::from_json(&body) {
                    Ok(m) => return Ok(m),
                    Err(e) => last = FetchError::Transport(h.up.name.clone(), e.to_string()),
                },
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    /// Raw bytes of Steam's depot manifest as served by an origin.
    pub fn steam_manifest(&self) -> Result<Vec<u8>, FetchError> {
        let mut last = FetchError::Exhausted("steam manifest".into(), "no origin".into());
        for h in self.hosts.iter().filter(|h| h.role == Role::Origin) {
            match h.up.get("/v1/steam-manifest") {
                Ok((b, _)) => return Ok(b),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    /// Measure every host and refresh the cache bitmaps.
    pub fn probe(&self, manifest_id: &str) {
        std::thread::scope(|s| {
            for h in &self.hosts {
                s.spawn(move || {
                    h.ping();
                    h.refresh_held(manifest_id);
                });
            }
        });
    }

    fn candidates(&self, ordinal: u32, len: usize) -> Vec<&Host> {
        let mut hs: Vec<&Host> = self
            .hosts
            .iter()
            .filter(|h| h.holds(ordinal) || h.fill)
            .collect();
        hs.sort_by(|a, b| {
            (a.is_down(), a.cost_ms(len))
                .partial_cmp(&(b.is_down(), b.cost_ms(len)))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if hs.is_empty() {
            hs = self.hosts.iter().collect();
        }
        hs
    }

    /// The host that would be asked first.
    #[must_use]
    pub fn route(&self, ordinal: u32, len: usize) -> Option<&str> {
        self.candidates(ordinal, len).first().map(|h| h.up.name.as_str())
    }

    /// Fetch and verify one page, trying candidates in order; returns the bytes and the host's name.
    pub fn fetch(&self, ordinal: u32, id: &PageId, len: usize) -> Result<(Vec<u8>, String), FetchError> {
        let mut errors = Vec::new();
        for h in self.candidates(ordinal, len) {
            let t0 = Instant::now();
            match h.up.page(id) {
                Ok((bytes, ttfb)) => {
                    h.ok(ttfb, t0.elapsed(), bytes.len());
                    return Ok((bytes, h.up.name.clone()));
                }
                Err(e) => {
                    if !matches!(e, FetchError::NotFound(_)) {
                        h.fail();
                    }
                    errors.push(e.to_string());
                }
            }
        }
        Err(FetchError::Exhausted(hex(id), errors.join("; ")))
    }
}
