//! The page server. `origin` owns the installation and serves it from the Steam library in place;
//! `cache` is a read-through tier with a byte budget. Both speak the same authenticated protocol:
//!
//! | route | answer |
//! |---|---|
//! | `GET /health` | `{ok}` (no credential) |
//! | `GET /v1/info` | role, build id, manifest id, counters |
//! | `GET /v1/manifest` | the manifest JSON |
//! | `GET /v1/page/<blake3 hex>` | the page bytes |
//! | `GET /v1/held?manifest=<id>` | bitmap, bit `n` set when page ordinal `n` is held |
//! | `GET /v1/steam-manifest` | Steam's own depot manifest of the build, when the host has it |
use crate::client::{FetchError, Upstream};
use crate::fsio::read_full_at;
use crate::http::{self, Body, Request, Response};
use crate::proto::PageId;
use crate::scan::{self, ScanOptions};
use crate::steam::{self, Install};
use crate::store::PageStore;
use crate::{Manifest, unhex};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::Duration;

/// A published manifest with its serialized form.
pub struct Snapshot {
    pub manifest: Manifest,
    pub json: Arc<Vec<u8>>,
}

impl Snapshot {
    fn new(manifest: Manifest) -> Self {
        let json = Arc::new(manifest.to_json());
        Self { manifest, json }
    }
}

pub enum PageError {
    NotFound,
    Failed(String),
}

pub trait Backend: Send + Sync {
    fn role(&self) -> &'static str;
    fn snapshot(&self) -> Option<Arc<Snapshot>>;
    fn page(&self, id: &PageId) -> Result<Vec<u8>, PageError>;
    fn held(&self, snap: &Snapshot) -> Vec<u8>;
    fn steam_manifest(&self) -> Option<Arc<Vec<u8>>>;
    fn counters(&self) -> serde_json::Value;
}

fn all_held(pages: usize) -> Vec<u8> {
    let mut bits = vec![0xffu8; pages.div_ceil(8)];
    if !pages.is_multiple_of(8)
        && let Some(last) = bits.last_mut()
    {
        *last = (1u8 << (pages % 8)) - 1;
    }
    bits
}

fn route(b: &dyn Backend, token: &str, req: &Request) -> Response {
    if req.path == "/health" || req.path == "/" {
        return Response::json(200, &serde_json::json!({ "ok": true, "service": "nie-link", "role": b.role() }));
    }
    if !http::bearer_ok(req, token) {
        return Response::error(401, "bearer token required");
    }
    let Some(snap) = b.snapshot() else {
        return Response::error(503, "manifest not ready");
    };
    match req.path.as_str() {
        "/v1/info" => {
            let m = &snap.manifest;
            Response::json(
                200,
                &serde_json::json!({
                    "role": b.role(), "buildId": m.build_id, "manifestId": m.manifest_id,
                    "files": m.files.len(), "pages": m.page_count(), "pageSize": m.page_size,
                    "bytes": m.total_bytes(), "counters": b.counters(),
                }),
            )
        }
        "/v1/manifest" => {
            let mut r = Response::bytes("application/json", Body::Shared(snap.json.clone()));
            r.headers.push(("ETag", format!("\"{}\"", snap.manifest.manifest_id)));
            r
        }
        "/v1/held" => {
            if req.param("manifest").is_some_and(|id| id != snap.manifest.manifest_id) {
                return Response::error(404, "manifest differs");
            }
            Response::bytes("application/octet-stream", Body::Bytes(b.held(&snap)))
        }
        "/v1/steam-manifest" => match b.steam_manifest() {
            Some(bytes) => Response::bytes("application/octet-stream", Body::Shared(bytes)),
            None => Response::error(404, "no steam manifest on this host"),
        },
        path => {
            let Some(id) = path.strip_prefix("/v1/page/").and_then(unhex::<32>) else {
                return Response::error(404, "unknown route");
            };
            match b.page(&id) {
                Ok(bytes) => {
                    let mut r = Response::bytes("application/octet-stream", Body::Bytes(bytes));
                    r.headers.push(("Cache-Control", "public, max-age=31536000, immutable".into()));
                    r
                }
                Err(PageError::NotFound) => Response::error(404, "unknown page"),
                Err(PageError::Failed(e)) => Response::error(502, &e),
            }
        }
    }
}

/// Run the HTTP loop for `backend`; returns only when the listener fails.
pub fn serve(listener: &TcpListener, token: &str, backend: Arc<dyn Backend>) {
    let token = token.to_owned();
    http::serve(listener, 512, Arc::new(move |req| route(backend.as_ref(), &token, req)));
}

/// The bearer token: the first non-empty line of `file`, or `$NIE_LINK_TOKEN`; at least 32 bytes.
pub fn load_token(file: Option<&Path>) -> Result<String> {
    let token = match file {
        Some(f) => std::fs::read_to_string(f)
            .with_context(|| f.display().to_string())?
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .to_owned(),
        None => std::env::var("NIE_LINK_TOKEN").unwrap_or_default(),
    };
    anyhow::ensure!(token.len() >= 32, "the link token must be at least 32 bytes (--token-file or NIE_LINK_TOKEN)");
    Ok(token)
}

// ---------------------------------------------------------------- origin

pub struct Origin {
    install: Install,
    state: RwLock<Option<Arc<OriginState>>>,
    pub served: AtomicU64,
    pub served_bytes: AtomicU64,
    pub stale: AtomicU64,
}

struct OriginState {
    snap: Arc<Snapshot>,
    index: HashMap<PageId, (u32, u64, u32)>,
    steam: Option<Arc<Vec<u8>>>,
}

impl Origin {
    /// Scan (reusing the saved manifest in `state_dir`), then keep rescanning in the background.
    pub fn start(steamapps: &Path, app_id: Option<u32>, state_dir: &Path, page_size: u32, rescan: Duration) -> Result<Arc<Self>> {
        std::fs::create_dir_all(state_dir)?;
        let install = steam::read_install(steamapps, app_id)?;
        let origin = Arc::new(Self {
            install,
            state: RwLock::new(None),
            served: AtomicU64::new(0),
            served_bytes: AtomicU64::new(0),
            stale: AtomicU64::new(0),
        });
        let saved = state_dir.join("manifest.json");
        let prev = std::fs::read(&saved).ok().and_then(|b| Manifest::from_json(&b).ok());
        origin.refresh(page_size, prev.as_ref(), &saved)?;
        let bg = origin.clone();
        let state_dir = state_dir.to_path_buf();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(rescan);
                let prev = bg.state.read().ok().and_then(|s| s.clone()).map(|s| s.snap.clone());
                if let Err(e) = bg.refresh(page_size, prev.as_deref().map(|s| &s.manifest), &state_dir.join("manifest.json")) {
                    eprintln!("nie-link origin: rescan failed: {e:#}");
                }
            }
        });
        Ok(origin)
    }

    fn refresh(&self, page_size: u32, prev: Option<&Manifest>, saved: &Path) -> Result<()> {
        let acf = steam::read_install(
            self.install.acf.parent().unwrap_or(Path::new(".")),
            Some(self.install.app_id),
        )?;
        let t0 = std::time::Instant::now();
        let manifest = scan::scan(&acf, &ScanOptions { page_size, progress: true }, prev)?;
        if prev.is_some_and(|p| p.manifest_id == manifest.manifest_id) {
            return Ok(());
        }
        eprintln!(
            "nie-link origin: build {} manifest {} files {} pages {} bytes {} in {:.1}s",
            manifest.build_id,
            manifest.manifest_id,
            manifest.files.len(),
            manifest.page_count(),
            manifest.total_bytes(),
            t0.elapsed().as_secs_f64()
        );
        let tmp = saved.with_extension("tmp");
        std::fs::write(&tmp, manifest.to_json())?;
        std::fs::rename(&tmp, saved)?;
        let steam = acf
            .depot_manifest_file
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .map(Arc::new);
        let state = OriginState {
            index: manifest.locate_index(),
            snap: Arc::new(Snapshot::new(manifest)),
            steam,
        };
        *self.state.write().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::new(state));
        Ok(())
    }

    fn current(&self) -> Option<Arc<OriginState>> {
        self.state.read().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }

    fn root(&self) -> &PathBuf {
        &self.install.root
    }
}

impl Backend for Origin {
    fn role(&self) -> &'static str {
        "origin"
    }

    fn snapshot(&self) -> Option<Arc<Snapshot>> {
        self.current().map(|s| s.snap.clone())
    }

    fn page(&self, id: &PageId) -> Result<Vec<u8>, PageError> {
        let st = self.current().ok_or(PageError::NotFound)?;
        let &(file, off, len) = st.index.get(id).ok_or(PageError::NotFound)?;
        let entry = &st.snap.manifest.files[file as usize];
        let f = std::fs::File::open(self.root().join(&entry.path)).map_err(|e| PageError::Failed(e.to_string()))?;
        let mut buf = vec![0u8; len as usize];
        let got = read_full_at(&f, &mut buf, off).map_err(|e| PageError::Failed(e.to_string()))?;
        if got != buf.len() || blake3::hash(&buf).as_bytes() != id {
            self.stale.fetch_add(1, Ordering::Relaxed);
            return Err(PageError::NotFound);
        }
        self.served.fetch_add(1, Ordering::Relaxed);
        self.served_bytes.fetch_add(u64::from(len), Ordering::Relaxed);
        Ok(buf)
    }

    fn held(&self, snap: &Snapshot) -> Vec<u8> {
        all_held(snap.manifest.page_count())
    }

    fn steam_manifest(&self) -> Option<Arc<Vec<u8>>> {
        self.current().and_then(|s| s.steam.clone())
    }

    fn counters(&self) -> serde_json::Value {
        serde_json::json!({
            "pagesServed": self.served.load(Ordering::Relaxed),
            "bytesServed": self.served_bytes.load(Ordering::Relaxed),
            "stalePages": self.stale.load(Ordering::Relaxed),
        })
    }
}

// ---------------------------------------------------------------- cache tier

type Flight = Arc<(Mutex<bool>, Condvar)>;

pub struct Cache {
    up: Upstream,
    store: PageStore,
    snap: RwLock<Option<Arc<Snapshot>>>,
    steam: RwLock<Option<Arc<Vec<u8>>>>,
    inflight: Mutex<HashMap<PageId, Flight>>,
    pub hits: AtomicU64,
    pub fills: AtomicU64,
}

impl Cache {
    /// Open the store in `dir` and follow `upstream`'s manifest.
    pub fn start(upstream: Upstream, dir: &Path, budget: u64, refresh: Duration) -> Result<Arc<Self>> {
        let cache = Arc::new(Self {
            up: upstream,
            store: PageStore::open(dir, budget)?,
            snap: RwLock::new(None),
            steam: RwLock::new(None),
            inflight: Mutex::new(HashMap::new()),
            hits: AtomicU64::new(0),
            fills: AtomicU64::new(0),
        });
        if let Err(e) = cache.follow() {
            eprintln!("nie-link cache: upstream not ready: {e}");
        }
        let bg = cache.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(refresh);
                if let Err(e) = bg.follow() {
                    eprintln!("nie-link cache: refresh failed: {e}");
                }
            }
        });
        Ok(cache)
    }

    fn follow(&self) -> Result<(), FetchError> {
        let (body, _) = self.up.get("/v1/manifest")?;
        let m = Manifest::from_json(&body).map_err(|e| FetchError::Transport(self.up.name.clone(), e.to_string()))?;
        let same = self
            .snap
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .is_some_and(|s| s.manifest.manifest_id == m.manifest_id);
        if !same {
            *self.snap.write().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::new(Snapshot::new(m)));
            let steam = self.up.get("/v1/steam-manifest").ok().map(|(b, _)| Arc::new(b));
            *self.steam.write().unwrap_or_else(std::sync::PoisonError::into_inner) = steam;
        }
        Ok(())
    }

    fn fill(&self, id: &PageId) -> Result<Vec<u8>, PageError> {
        match self.up.page(id) {
            Ok((bytes, _)) => {
                let _ = self.store.put(id, &bytes);
                self.fills.fetch_add(1, Ordering::Relaxed);
                Ok(bytes)
            }
            Err(FetchError::NotFound(_)) => Err(PageError::NotFound),
            Err(e) => Err(PageError::Failed(e.to_string())),
        }
    }
}

impl Backend for Cache {
    fn role(&self) -> &'static str {
        "cache"
    }

    fn snapshot(&self) -> Option<Arc<Snapshot>> {
        self.snap.read().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }

    fn page(&self, id: &PageId) -> Result<Vec<u8>, PageError> {
        if let Some(b) = self.store.get(id) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            return Ok(b);
        }
        // One upstream request per page, however many clients ask at once.
        let (flight, leader) = {
            let mut map = self.inflight.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            match map.get(id) {
                Some(f) => (f.clone(), false),
                None => {
                    let f: Flight = Arc::new((Mutex::new(false), Condvar::new()));
                    map.insert(*id, f.clone());
                    (f, true)
                }
            }
        };
        if leader {
            let r = self.fill(id);
            self.inflight.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(id);
            *flight.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = true;
            flight.1.notify_all();
            return r;
        }
        let mut done = flight.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        while !*done {
            done = flight.1.wait(done).unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        drop(done);
        self.store.get(id).ok_or(PageError::NotFound).or_else(|_| self.fill(id))
    }

    fn held(&self, snap: &Snapshot) -> Vec<u8> {
        let n = snap.manifest.page_count();
        let mut bits = vec![0u8; n.div_ceil(8)];
        for i in 0..n {
            if let Some(id) = snap.manifest.page_id(i as u32)
                && self.store.contains(id)
            {
                bits[i / 8] |= 1 << (i % 8);
            }
        }
        bits
    }

    fn steam_manifest(&self) -> Option<Arc<Vec<u8>>> {
        self.steam.read().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }

    fn counters(&self) -> serde_json::Value {
        let (pages, bytes) = self.store.usage();
        serde_json::json!({
            "hits": self.hits.load(Ordering::Relaxed),
            "fills": self.fills.load(Ordering::Relaxed),
            "heldPages": pages, "heldBytes": bytes,
            "evicted": self.store.evicted.load(Ordering::Relaxed),
            "corrupt": self.store.corrupt.load(Ordering::Relaxed),
            "upstream": self.up.base,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    fn get(addr: &str, path: &str, token: &str) -> (u16, Vec<u8>) {
        let mut s = TcpStream::connect(addr).unwrap();
        write!(s, "GET {path} HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n").unwrap();
        let mut raw = Vec::new();
        s.read_to_end(&mut raw).unwrap();
        let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let head = String::from_utf8_lossy(&raw[..split]).into_owned();
        let status = head.split(' ').nth(1).unwrap().parse().unwrap();
        (status, raw[split + 4..].to_vec())
    }

    fn library(dir: &Path) -> PathBuf {
        let apps = dir.join("steamapps");
        std::fs::create_dir_all(apps.join("common/Game/data")).unwrap();
        std::fs::write(
            apps.join("appmanifest_4.acf"),
            "\"AppState\"\n{\n\"appid\" \"4\"\n\"installdir\" \"Game\"\n\"buildid\" \"5\"\n}\n",
        )
        .unwrap();
        let bytes: Vec<u8> = (0..300_000u32).map(|i| (i * 7 % 256) as u8).collect();
        std::fs::write(apps.join("common/Game/data/a.cpk"), bytes).unwrap();
        std::fs::write(apps.join("common/Game/nie.exe"), b"MZ-hello").unwrap();
        apps
    }

    #[test]
    fn origin_and_cache_tier_serve_verified_pages() {
        let tmp = tempfile::tempdir().unwrap();
        let apps = library(tmp.path());
        let token = "t".repeat(40);
        let origin = Origin::start(&apps, None, &tmp.path().join("state"), 65536, Duration::from_secs(3600)).unwrap();
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let oaddr = l.local_addr().unwrap().to_string();
        let (o, t) = (origin.clone(), token.clone());
        std::thread::spawn(move || serve(&l, &t, o));

        assert_eq!(get(&oaddr, "/v1/info", "wrong").0, 401);
        assert_eq!(get(&oaddr, "/health", "").0, 200);
        let (st, body) = get(&oaddr, "/v1/manifest", &token);
        assert_eq!(st, 200);
        let m = Manifest::from_json(&body).unwrap();
        assert_eq!(m.files.len(), 2);
        let a = m.find("data/a.cpk").unwrap();
        let id = m.file_ids(a)[4];
        let (st, page) = get(&oaddr, &format!("/v1/page/{}", hex(&id)), &token);
        assert_eq!(st, 200);
        assert_eq!(blake3::hash(&page).as_bytes(), &id);
        assert_eq!(get(&oaddr, &format!("/v1/page/{}", hex(&[9u8; 32])), &token).0, 404);

        let cache = Cache::start(
            Upstream::new("origin", &format!("http://{oaddr}"), &token),
            &tmp.path().join("cache"),
            1 << 20,
            Duration::from_secs(3600),
        )
        .unwrap();
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let caddr = l.local_addr().unwrap().to_string();
        let (c, t) = (cache.clone(), token.clone());
        std::thread::spawn(move || serve(&l, &t, c));
        let held = |n: usize| get(&caddr, &format!("/v1/held?manifest={}", m.manifest_id), &token).1.iter().map(|b| b.count_ones()).sum::<u32>() as usize == n;
        assert!(held(0));
        let (st, page2) = get(&caddr, &format!("/v1/page/{}", hex(&id)), &token);
        assert_eq!((st, page2), (200, page));
        assert!(held(1));
        assert_eq!(cache.fills.load(Ordering::Relaxed), 1);
        get(&caddr, &format!("/v1/page/{}", hex(&id)), &token);
        assert_eq!(cache.hits.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn full_bitmap_has_no_stray_bits() {
        assert_eq!(all_held(10), vec![0xff, 0b11]);
        assert_eq!(all_held(8), vec![0xff]);
    }
}
