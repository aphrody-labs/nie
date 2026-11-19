//! On-disk page cache: content-addressed files, a byte budget, least-recently-used eviction and
//! a hash check on every read, so a damaged or truncated page is dropped instead of served.
use crate::{PageId, hex};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug)]
struct Entry {
    len: u32,
    tick: u64,
}

#[derive(Debug, Default)]
struct Index {
    map: HashMap<PageId, Entry>,
    bytes: u64,
    tick: u64,
}

pub struct PageStore {
    dir: PathBuf,
    budget: u64,
    index: Mutex<Index>,
    tmp: AtomicU64,
    pub hits: AtomicU64,
    pub misses: AtomicU64,
    pub evicted: AtomicU64,
    pub corrupt: AtomicU64,
}

impl PageStore {
    /// Open `dir`, indexing the pages already there (oldest files are evicted first).
    pub fn open(dir: &Path, budget: u64) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let mut found: Vec<(std::time::SystemTime, PageId, u32)> = Vec::new();
        for shard in std::fs::read_dir(dir)? {
            let shard = shard?;
            if !shard.file_type()?.is_dir() {
                continue;
            }
            for page in std::fs::read_dir(shard.path())? {
                let page = page?;
                let name = page.file_name();
                let name = name.to_string_lossy();
                let meta = page.metadata()?;
                match crate::unhex::<32>(&name) {
                    Some(id) => found.push((
                        meta.modified().unwrap_or(std::time::UNIX_EPOCH),
                        id,
                        u32::try_from(meta.len()).unwrap_or(u32::MAX),
                    )),
                    None => {
                        let _ = std::fs::remove_file(page.path());
                    }
                }
            }
        }
        found.sort_by_key(|f| f.0);
        let mut index = Index::default();
        for (_, id, len) in found {
            index.tick += 1;
            index.bytes += u64::from(len);
            let tick = index.tick;
            index.map.insert(id, Entry { len, tick });
        }
        let store = Self {
            dir: dir.to_path_buf(),
            budget,
            index: Mutex::new(index),
            tmp: AtomicU64::new(0),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            evicted: AtomicU64::new(0),
            corrupt: AtomicU64::new(0),
        };
        store.evict_to(store.budget);
        Ok(store)
    }

    fn path(&self, id: &PageId) -> PathBuf {
        let h = hex(id);
        self.dir.join(&h[..2]).join(h)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Index> {
        self.index.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[must_use]
    pub fn contains(&self, id: &PageId) -> bool {
        self.lock().map.contains_key(id)
    }

    /// `(pages, bytes)` currently held.
    #[must_use]
    pub fn usage(&self) -> (usize, u64) {
        let i = self.lock();
        (i.map.len(), i.bytes)
    }

    /// The page, verified against its id, or `None`.
    pub fn get(&self, id: &PageId) -> Option<Vec<u8>> {
        {
            let mut i = self.lock();
            i.tick += 1;
            let tick = i.tick;
            match i.map.get_mut(id) {
                Some(e) => e.tick = tick,
                None => {
                    self.misses.fetch_add(1, Ordering::Relaxed);
                    return None;
                }
            }
        }
        let bytes = std::fs::read(self.path(id)).ok();
        if let Some(b) = &bytes
            && blake3::hash(b).as_bytes() == id
        {
            self.hits.fetch_add(1, Ordering::Relaxed);
            return bytes;
        }
        self.corrupt.fetch_add(1, Ordering::Relaxed);
        self.remove(id);
        None
    }

    fn remove(&self, id: &PageId) {
        {
            let mut i = self.lock();
            if let Some(e) = i.map.remove(id) {
                i.bytes = i.bytes.saturating_sub(u64::from(e.len));
            }
        }
        let _ = std::fs::remove_file(self.path(id));
    }

    /// Store a page; the caller has already checked it hashes to `id`.
    pub fn put(&self, id: &PageId, bytes: &[u8]) -> std::io::Result<()> {
        if self.lock().map.contains_key(id) {
            return Ok(());
        }
        let path = self.path(id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension(format!(
            "tmp{}-{}",
            std::process::id(),
            self.tmp.fetch_add(1, Ordering::Relaxed)
        ));
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.flush()?;
        drop(f);
        std::fs::rename(&tmp, &path)?;
        {
            let mut i = self.lock();
            i.tick += 1;
            let tick = i.tick;
            let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
            if i.map.insert(*id, Entry { len, tick }).is_none() {
                i.bytes += u64::from(len);
            }
        }
        self.evict_to(self.budget);
        Ok(())
    }

    fn evict_to(&self, budget: u64) {
        loop {
            let victim = {
                let i = self.lock();
                if i.bytes <= budget {
                    return;
                }
                i.map.iter().min_by_key(|(_, e)| e.tick).map(|(id, _)| *id)
            };
            let Some(id) = victim else { return };
            self.remove(&id);
            self.evicted.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(n: u8, len: usize) -> (PageId, Vec<u8>) {
        let bytes = vec![n; len];
        (*blake3::hash(&bytes).as_bytes(), bytes)
    }

    #[test]
    fn evicts_least_recently_used_over_budget() {
        let dir = tempfile::tempdir().unwrap();
        let s = PageStore::open(dir.path(), 250).unwrap();
        let (a, ab) = page(1, 100);
        let (b, bb) = page(2, 100);
        let (c, cb) = page(3, 100);
        s.put(&a, &ab).unwrap();
        s.put(&b, &bb).unwrap();
        assert_eq!(s.get(&a).as_deref(), Some(&ab[..]));
        s.put(&c, &cb).unwrap();
        assert!(s.contains(&a) && s.contains(&c) && !s.contains(&b));
        assert_eq!(s.usage(), (2, 200));
    }

    #[test]
    fn corrupt_page_is_dropped_and_reopen_keeps_pages() {
        let dir = tempfile::tempdir().unwrap();
        let (a, ab) = page(7, 64);
        {
            let s = PageStore::open(dir.path(), 1 << 20).unwrap();
            s.put(&a, &ab).unwrap();
        }
        let s = PageStore::open(dir.path(), 1 << 20).unwrap();
        assert!(s.contains(&a));
        std::fs::write(s.path(&a), b"rotten").unwrap();
        assert!(s.get(&a).is_none());
        assert!(!s.contains(&a));
        assert_eq!(s.corrupt.load(Ordering::Relaxed), 1);
    }
}
