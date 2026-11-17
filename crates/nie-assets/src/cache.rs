//! Size-bounded read-through cache for files streamed from a remote source.
//!
//! The cache lives in a user-chosen directory outside the repository. Entries are named by the
//! SHA-256 of the logical path, written atomically, and evicted oldest-first when the byte budget
//! is exceeded. It never stores a CPK, only the decoded files that were actually requested.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use sha2::{Digest, Sha256};

/// A directory of cached files with a byte budget.
#[derive(Debug)]
pub struct DiskCache {
    dir: PathBuf,
    max_bytes: u64,
    used: Mutex<u64>,
}

impl DiskCache {
    /// Opens (and creates) `dir`, counting what is already stored.
    ///
    /// # Errors
    /// Fails when the directory cannot be created or listed.
    pub fn open(dir: impl Into<PathBuf>, max_bytes: u64) -> std::io::Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        let used = fs::read_dir(&dir)?
            .flatten()
            .filter_map(|entry| entry.metadata().ok())
            .filter(fs::Metadata::is_file)
            .map(|meta| meta.len())
            .sum();
        Ok(Self {
            dir,
            max_bytes,
            used: Mutex::new(used),
        })
    }

    /// The cache directory.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Bytes currently stored.
    #[must_use]
    pub fn used_bytes(&self) -> u64 {
        self.used.lock().map_or(0, |used| *used)
    }

    fn entry_path(&self, logical: &str) -> PathBuf {
        let digest = Sha256::digest(logical.as_bytes());
        let mut name = String::with_capacity(32);
        for byte in &digest[..16] {
            name.push_str(&format!("{byte:02x}"));
        }
        self.dir.join(name)
    }

    /// The cached bytes of `logical`, if present.
    #[must_use]
    pub fn get(&self, logical: &str) -> Option<Vec<u8>> {
        fs::read(self.entry_path(logical)).ok()
    }

    /// Stores `bytes` for `logical`, evicting the oldest entries to stay within the budget.
    /// A file larger than the whole budget is not cached.
    ///
    /// # Errors
    /// Fails when the entry cannot be written.
    pub fn put(&self, logical: &str, bytes: &[u8]) -> std::io::Result<()> {
        let size = bytes.len() as u64;
        if size > self.max_bytes {
            return Ok(());
        }
        let target = self.entry_path(logical);
        let previous = fs::metadata(&target).map_or(0, |meta| meta.len());
        let mut staged = tempfile_in(&self.dir)?;
        staged.0.write_all(bytes)?;
        fs::rename(&staged.1, &target)?;
        staged.1 = PathBuf::new();
        if let Ok(mut used) = self.used.lock() {
            *used = used.saturating_sub(previous) + size;
            if *used > self.max_bytes {
                *used = self.evict(*used, &target);
            }
        }
        Ok(())
    }

    /// Removes oldest entries (never `keep`) until under 90% of the budget; returns the new total.
    fn evict(&self, mut used: u64, keep: &Path) -> u64 {
        let goal = self.max_bytes / 10 * 9;
        let mut entries: Vec<(SystemTime, u64, PathBuf)> = fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let meta = entry.metadata().ok()?;
                meta.is_file().then(|| {
                    (
                        meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                        meta.len(),
                        entry.path(),
                    )
                })
            })
            .collect();
        entries.sort();
        for (_, size, path) in entries {
            if used <= goal {
                break;
            }
            if path != keep && fs::remove_file(&path).is_ok() {
                used = used.saturating_sub(size);
            }
        }
        used
    }
}

/// A uniquely named staging file in `dir`; the second field is removed on drop if still set.
struct Staged(fs::File, PathBuf);

impl Drop for Staged {
    fn drop(&mut self) {
        if !self.1.as_os_str().is_empty() {
            let _ = fs::remove_file(&self.1);
        }
    }
}

fn tempfile_in(dir: &Path) -> std::io::Result<Staged> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let path = dir.join(format!(
        ".stage-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let file = fs::File::create(&path)?;
    Ok(Staged(file, path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_evicts_oldest_first() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::open(dir.path(), 100).unwrap();
        cache.put("a", &[1; 40]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        cache.put("b", &[2; 40]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        cache.put("c", &[3; 40]).unwrap();
        assert_eq!(cache.get("c").as_deref(), Some(&[3u8; 40][..]));
        assert!(cache.get("a").is_none(), "oldest entry evicted");
        assert!(cache.used_bytes() <= 100);
        cache.put("huge", &[0; 101]).unwrap();
        assert!(
            cache.get("huge").is_none(),
            "over-budget file is not cached"
        );
    }
}
