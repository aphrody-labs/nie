//! The client engine: manifest, verified page cache, routing, read-ahead and the access trace.
use crate::client::{FetchError, Router};
use crate::config::{Config, state_dir};
use crate::proto::{FileEntry, PageId};
use crate::store::PageStore;
use crate::Manifest;
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};

/// Pages fetched at once for one read.
const READ_PARALLEL: usize = 8;
const MAX_READAHEAD: u32 = 16;
const MAX_PREFETCH_BATCHES: usize = 4;

type Flight = Arc<(Mutex<bool>, Condvar)>;
type Slot = Mutex<Option<Result<Vec<u8>, FetchError>>>;

#[derive(Default)]
struct Trace {
    seen: HashSet<u32>,
    order: Vec<u32>,
}

pub struct Link {
    pub manifest: Arc<Manifest>,
    pub store: Arc<PageStore>,
    pub router: Arc<Router>,
    inflight: Mutex<HashMap<PageId, Flight>>,
    sequential: Mutex<HashMap<usize, (u64, u32)>>,
    trace: Mutex<Option<Trace>>,
    prefetching: AtomicUsize,
    pub net_pages: AtomicU64,
    pub net_bytes: AtomicU64,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Link {
    /// Load the manifest from the hosts (falling back to the saved copy when none answers), open the
    /// page cache and measure the hosts.
    pub fn open(cfg: &Config) -> Result<Arc<Self>> {
        let router = Arc::new(cfg.router()?);
        let saved = state_dir().join("manifest.json");
        let manifest = match router.manifest() {
            Ok(m) => {
                let _ = std::fs::create_dir_all(state_dir());
                let _ = std::fs::write(&saved, m.to_json());
                m
            }
            Err(net) => {
                let bytes = std::fs::read(&saved).with_context(|| format!("{net}; no saved manifest"))?;
                eprintln!("nie-link: hosts unreachable ({net}); using the saved manifest");
                Manifest::from_json(&bytes)?
            }
        };
        let store = Arc::new(PageStore::open(&cfg.cache_dir(), cfg.cache_bytes())?);
        router.probe(&manifest.manifest_id);
        Ok(Arc::new(Self {
            manifest: Arc::new(manifest),
            store,
            router,
            inflight: Mutex::new(HashMap::new()),
            sequential: Mutex::new(HashMap::new()),
            trace: Mutex::new(None),
            prefetching: AtomicUsize::new(0),
            net_pages: AtomicU64::new(0),
            net_bytes: AtomicU64::new(0),
        }))
    }

    /// Start recording the order in which pages are first needed.
    pub fn record_trace(&self) {
        *lock(&self.trace) = Some(Trace::default());
    }

    fn touch(&self, ordinal: u32) {
        if let Some(t) = lock(&self.trace).as_mut()
            && t.seen.insert(ordinal)
        {
            t.order.push(ordinal);
        }
    }

    fn trace_path(name: &str) -> PathBuf {
        state_dir().join(format!("trace-{name}.bin"))
    }

    /// Write the recorded trace as little-endian page ordinals; returns how many.
    pub fn save_trace(&self, name: &str) -> Result<usize> {
        let guard = lock(&self.trace);
        let Some(t) = guard.as_ref() else { return Ok(0) };
        let mut raw = Vec::with_capacity(t.order.len() * 4);
        for o in &t.order {
            raw.extend_from_slice(&o.to_le_bytes());
        }
        let tmp = Self::trace_path(name).with_extension("tmp");
        std::fs::write(&tmp, raw)?;
        std::fs::rename(&tmp, Self::trace_path(name))?;
        Ok(t.order.len())
    }

    pub fn load_trace(&self, name: &str) -> Result<Vec<u32>> {
        let raw = std::fs::read(Self::trace_path(name)).with_context(|| format!("trace {name}"))?;
        Ok(raw
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c))
            .filter(|o| (*o as usize) < self.manifest.page_count())
            .collect())
    }

    /// Routing only needs the order of magnitude of a page's size.
    fn page_len_of(&self) -> usize {
        self.manifest.page_size as usize
    }

    /// Fetch one page into the cache (a no-op when it is there). `demand` pages enter the trace.
    fn ensure(&self, ordinal: u32, demand: bool) -> Result<(), FetchError> {
        if demand {
            self.touch(ordinal);
        }
        let id = *self.manifest.page_id(ordinal).ok_or_else(|| {
            FetchError::Exhausted(ordinal.to_string(), "ordinal out of range".into())
        })?;
        if self.store.contains(&id) {
            return Ok(());
        }
        let (flight, leader) = {
            let mut map = lock(&self.inflight);
            match map.get(&id) {
                Some(f) => (f.clone(), false),
                None => {
                    let f: Flight = Arc::new((Mutex::new(false), Condvar::new()));
                    map.insert(id, f.clone());
                    (f, true)
                }
            }
        };
        if !leader {
            let mut done = lock(&flight.0);
            while !*done {
                done = flight.1.wait(done).unwrap_or_else(std::sync::PoisonError::into_inner);
            }
            return Ok(());
        }
        let r = self.router.fetch(ordinal, &id, self.page_len_of());
        let out = match r {
            Ok((bytes, _host)) => {
                self.net_pages.fetch_add(1, Ordering::Relaxed);
                self.net_bytes.fetch_add(bytes.len() as u64, Ordering::Relaxed);
                self.store
                    .put(&id, &bytes)
                    .map_err(|e| FetchError::Transport("cache".into(), e.to_string()))
            }
            Err(e) => Err(e),
        };
        lock(&self.inflight).remove(&id);
        *lock(&flight.0) = true;
        flight.1.notify_all();
        out
    }

    /// The bytes of page `ordinal`, from the cache or the network.
    pub fn page(&self, ordinal: u32, demand: bool) -> Result<Vec<u8>, FetchError> {
        let id = *self.manifest.page_id(ordinal).ok_or_else(|| {
            FetchError::Exhausted(ordinal.to_string(), "ordinal out of range".into())
        })?;
        for _ in 0..3 {
            self.ensure(ordinal, demand)?;
            if let Some(b) = self.store.get(&id) {
                return Ok(b);
            }
        }
        // The cache could not keep the page (budget smaller than the working set): go direct.
        self.router
            .fetch(ordinal, &id, self.page_len_of())
            .map(|(b, _)| b)
    }

    /// Like [`Self::page`], but a page that is not cached is fetched without being stored: for readers
    /// (the ProjFS provider) whose filesystem keeps the bytes itself, so the cache is not churned.
    pub fn page_transit(&self, ordinal: u32) -> Result<Vec<u8>, FetchError> {
        let id = *self.manifest.page_id(ordinal).ok_or_else(|| {
            FetchError::Exhausted(ordinal.to_string(), "ordinal out of range".into())
        })?;
        self.touch(ordinal);
        if let Some(b) = self.store.get(&id) {
            return Ok(b);
        }
        let (bytes, _) = self.router.fetch(ordinal, &id, self.page_len_of())?;
        self.net_pages.fetch_add(1, Ordering::Relaxed);
        self.net_bytes.fetch_add(bytes.len() as u64, Ordering::Relaxed);
        Ok(bytes)
    }

    /// Pages `first..=last` of `file`, fetched in parallel; `store` keeps fetched pages in the cache.
    pub fn pages(&self, file: &FileEntry, first: u32, last: u32, store: bool) -> Result<Vec<Vec<u8>>, FetchError> {
        let n = (last - first + 1) as usize;
        let slots: Vec<Slot> = (0..n).map(|_| Mutex::new(None)).collect();
        let next = AtomicUsize::new(0);
        let workers = n.min(READ_PARALLEL);
        std::thread::scope(|s| {
            for _ in 0..workers.max(1) {
                s.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= n {
                            break;
                        }
                        let ordinal = file.first_page + first + i as u32;
                        let r = if store { self.page(ordinal, true) } else { self.page_transit(ordinal) };
                        *lock(&slots[i]) = Some(r);
                    }
                });
            }
        });
        slots
            .into_iter()
            .map(|s| lock(&s).take().unwrap_or_else(|| Err(FetchError::Exhausted("read".into(), "worker lost".into()))))
            .collect()
    }

    /// Bytes `[offset, offset + len)` of file `index`, clipped to the file size.
    pub fn read(self: &Arc<Self>, index: usize, offset: u64, len: usize) -> Result<Vec<u8>, FetchError> {
        let file = &self.manifest.files[index];
        if offset >= file.size || len == 0 {
            return Ok(Vec::new());
        }
        let end = (offset + len as u64).min(file.size);
        let ps = u64::from(self.manifest.page_size);
        let (first, last) = ((offset / ps) as u32, ((end - 1) / ps) as u32);
        let pages = self.pages(file, first, last, true)?;
        let mut out = Vec::with_capacity((end - offset) as usize);
        for (i, p) in pages.iter().enumerate() {
            let page_start = (u64::from(first) + i as u64) * ps;
            let a = offset.saturating_sub(page_start) as usize;
            let b = ((end - page_start) as usize).min(p.len());
            out.extend_from_slice(&p[a..b]);
        }
        self.readahead(index, offset, end, last);
        Ok(out)
    }

    /// Sequential reads grow a window of pages that are fetched in the background.
    fn readahead(self: &Arc<Self>, index: usize, offset: u64, end: u64, last: u32) {
        let window = {
            let mut seq = lock(&self.sequential);
            let e = seq.entry(index).or_insert((0, 0));
            e.1 = if e.0 == offset && offset != 0 { (e.1.max(1) * 2).min(MAX_READAHEAD) } else { 0 };
            e.0 = end;
            e.1
        };
        if window == 0 {
            return;
        }
        let file = &self.manifest.files[index];
        let from = file.first_page + last + 1;
        let to = (file.first_page + file.pages).min(from + window);
        let todo: Vec<u32> = (from..to)
            .filter(|o| self.manifest.page_id(*o).is_some_and(|id| !self.store.contains(id)))
            .collect();
        if !todo.is_empty() {
            self.prefetch_background(todo);
        }
    }

    fn prefetch_background(self: &Arc<Self>, ordinals: Vec<u32>) {
        if self.prefetching.fetch_add(1, Ordering::AcqRel) >= MAX_PREFETCH_BATCHES {
            self.prefetching.fetch_sub(1, Ordering::AcqRel);
            return;
        }
        let me = self.clone();
        std::thread::spawn(move || {
            for o in ordinals {
                let _ = me.ensure(o, false);
            }
            me.prefetching.fetch_sub(1, Ordering::AcqRel);
        });
    }

    /// Fetch `ordinals` into the cache with `parallel` connections; returns `(fetched, failed)`.
    pub fn prefetch(&self, ordinals: &[u32], parallel: usize) -> (usize, usize) {
        let next = AtomicUsize::new(0);
        let failed = AtomicUsize::new(0);
        let fetched = AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..parallel.max(1) {
                s.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(&o) = ordinals.get(i) else { break };
                        let cached = self.manifest.page_id(o).is_some_and(|id| self.store.contains(id));
                        match self.ensure(o, false) {
                            Ok(()) if !cached => {
                                fetched.fetch_add(1, Ordering::Relaxed);
                            }
                            Ok(()) => {}
                            Err(_) => {
                                failed.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                });
            }
        });
        (fetched.into_inner(), failed.into_inner())
    }

    /// Fetch `ordinals` from the host called `name` only, so that a cache tier fills (the tier is a
    /// read-through cache: it keeps what it serves). Returns `(ok, failed)`.
    pub fn warm(&self, name: &str, ordinals: &[u32], parallel: usize) -> Result<(usize, usize)> {
        let host = self
            .router
            .hosts
            .iter()
            .find(|h| h.up.name == name)
            .with_context(|| format!("no host named {name}"))?;
        let next = AtomicUsize::new(0);
        let ok = AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..parallel.max(1) {
                s.spawn(|| {
                    while let Some(&o) = ordinals.get(next.fetch_add(1, Ordering::Relaxed)) {
                        if let Some(id) = self.manifest.page_id(o)
                            && host.up.page(id).is_ok()
                        {
                            ok.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                });
            }
        });
        let ok = ok.into_inner();
        Ok((ok, ordinals.len() - ok))
    }

    #[must_use]
    pub fn status(&self) -> serde_json::Value {
        let (pages, bytes) = self.store.usage();
        serde_json::json!({
            "buildId": self.manifest.build_id, "manifestId": self.manifest.manifest_id,
            "files": self.manifest.files.len(), "pages": self.manifest.page_count(),
            "bytes": self.manifest.total_bytes(),
            "cache": {
                "pages": pages, "bytes": bytes,
                "hits": self.store.hits.load(Ordering::Relaxed),
                "evicted": self.store.evicted.load(Ordering::Relaxed),
                "corrupt": self.store.corrupt.load(Ordering::Relaxed),
            },
            "network": {
                "pages": self.net_pages.load(Ordering::Relaxed),
                "bytes": self.net_bytes.load(Ordering::Relaxed),
            },
            "hosts": self.router.hosts.iter().map(crate::client::Host::summary).collect::<Vec<_>>(),
        })
    }
}
