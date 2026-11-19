//! Cut an installation into pages and name them: the manifest of one Steam build.
use crate::fsio::{mtime_ns, read_full_at};
use crate::proto::{FileEntry, Manifest, PageId};
use crate::steam::Install;
use anyhow::{Context, Result, bail};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Pages hashed per task, so a few huge archives still spread over every core.
const SEGMENT_PAGES: u32 = 64;

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub page_size: u32,
    /// Print progress to stderr while hashing.
    pub progress: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self { page_size: crate::proto::DEFAULT_PAGE_SIZE, progress: false }
    }
}

struct Found {
    path: String,
    abs: PathBuf,
    size: u64,
    mtime_ns: i64,
}

/// Regular files below `root`, `/`-separated, sorted. Hidden top-level entries (`.depotdownloader`,
/// `.nie-steam-cache`) are tooling state, not part of the game.
fn walk(root: &Path) -> Result<Vec<Found>> {
    let mut out = Vec::new();
    let mut stack = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, rel)) = stack.pop() {
        for entry in std::fs::read_dir(&dir).with_context(|| dir.display().to_string())? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                bail!("non UTF-8 file name in {}", dir.display());
            };
            if rel.is_empty() && name.starts_with('.') {
                continue;
            }
            let path = if rel.is_empty() { name.to_owned() } else { format!("{rel}/{name}") };
            let meta = std::fs::metadata(entry.path())?;
            if meta.is_dir() {
                stack.push((entry.path(), path));
            } else if meta.is_file() {
                out.push(Found { path, abs: entry.path(), size: meta.len(), mtime_ns: mtime_ns(&meta) });
            }
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// Hash `pages` pages of `file` from page index `start`.
fn hash_pages(abs: &Path, size: u64, page_size: u32, start: u32, end: u32) -> Result<Vec<PageId>> {
    let file = std::fs::File::open(abs).with_context(|| abs.display().to_string())?;
    let mut buf = vec![0u8; page_size as usize];
    let mut ids = Vec::with_capacity((end - start) as usize);
    for index in start..end {
        let off = u64::from(index) * u64::from(page_size);
        let want = usize::try_from((size - off).min(u64::from(page_size)))?;
        let got = read_full_at(&file, &mut buf[..want], off)?;
        if got != want {
            bail!("{} shrank while scanning", abs.display());
        }
        ids.push(*blake3::hash(&buf[..want]).as_bytes());
    }
    Ok(ids)
}

/// Scan `install.root`. Pages of files whose size and mtime match `prev` are not hashed again.
pub fn scan(install: &Install, opts: &ScanOptions, prev: Option<&Manifest>) -> Result<Manifest> {
    anyhow::ensure!(
        opts.page_size.is_power_of_two() && opts.page_size >= 4096,
        "page size must be a power of two of at least 4096"
    );
    let found = walk(&install.root)?;
    let reuse: HashMap<&str, &FileEntry> = prev
        .filter(|p| p.page_size == opts.page_size)
        .map(|p| p.files.iter().map(|f| (f.path.as_str(), f)).collect())
        .unwrap_or_default();

    let mut ids_by_file: Vec<Vec<PageId>> = vec![Vec::new(); found.len()];
    let mut tasks: Vec<(usize, u32, u32)> = Vec::new();
    let mut todo_bytes = 0u64;
    for (i, f) in found.iter().enumerate() {
        let pages = u32::try_from(f.size.div_ceil(u64::from(opts.page_size)))?;
        match (reuse.get(f.path.as_str()), prev) {
            (Some(old), Some(p)) if old.size == f.size && old.mtime_ns == f.mtime_ns => {
                ids_by_file[i] = p.file_ids(old).to_vec();
            }
            _ => {
                todo_bytes += f.size;
                let mut s = 0;
                while s < pages {
                    let e = (s + SEGMENT_PAGES).min(pages);
                    tasks.push((i, s, e));
                    s = e;
                }
            }
        }
    }

    let done = AtomicU64::new(0);
    let finished = std::sync::atomic::AtomicBool::new(false);
    let results: Result<Vec<(usize, u32, Vec<PageId>)>> = std::thread::scope(|scope| {
        if opts.progress && todo_bytes > 0 {
            scope.spawn(|| {
                let t0 = Instant::now();
                let mut last = Instant::now();
                while !finished.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(200));
                    if last.elapsed() >= Duration::from_secs(5) {
                        last = Instant::now();
                        let d = done.load(Ordering::Relaxed);
                        eprintln!(
                            "scan {:.1}/{:.1} GiB {:.0} MB/s",
                            d as f64 / 1073741824.0,
                            todo_bytes as f64 / 1073741824.0,
                            d as f64 / 1e6 / t0.elapsed().as_secs_f64().max(0.001)
                        );
                    }
                }
            });
        }
        let r = tasks
            .par_iter()
            .map(|&(i, s, e)| {
                let f = &found[i];
                let ids = hash_pages(&f.abs, f.size, opts.page_size, s, e)?;
                let bytes = (u64::from(e) * u64::from(opts.page_size)).min(f.size)
                    - u64::from(s) * u64::from(opts.page_size);
                done.fetch_add(bytes, Ordering::Relaxed);
                Ok((i, s, ids))
            })
            .collect();
        finished.store(true, Ordering::Relaxed);
        r
    });
    let mut results = results?;
    results.sort_by_key(|(i, s, _)| (*i, *s));
    for (i, _, ids) in results {
        ids_by_file[i].extend(ids);
    }

    let mut files = Vec::with_capacity(found.len());
    let mut all = Vec::new();
    for (f, ids) in found.iter().zip(ids_by_file) {
        let meta = std::fs::metadata(&f.abs)?;
        if meta.len() != f.size || mtime_ns(&meta) != f.mtime_ns {
            bail!("{} changed while scanning", f.path);
        }
        files.push(FileEntry {
            path: f.path.clone(),
            size: f.size,
            mtime_ns: f.mtime_ns,
            first_page: u32::try_from(all.len())?,
            pages: u32::try_from(ids.len())?,
        });
        all.extend(ids);
    }
    Ok(Manifest::assemble(
        install.app_id,
        install.build_id.clone(),
        install.depot_manifest.clone(),
        install.install_dir.clone(),
        opts.page_size,
        files,
        all,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn install(root: &Path) -> Install {
        Install {
            app_id: 1,
            build_id: "9".into(),
            install_dir: "g".into(),
            root: root.to_path_buf(),
            acf: PathBuf::new(),
            depot_manifest: None,
            depot_manifest_file: None,
        }
    }

    #[test]
    fn scans_pages_and_reuses_unchanged_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("data/packs")).unwrap();
        std::fs::create_dir_all(root.join(".nie-steam-cache")).unwrap();
        std::fs::write(root.join(".nie-steam-cache/x"), b"ignored").unwrap();
        let big: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(root.join("data/packs/a.cpk"), &big).unwrap();
        std::fs::write(root.join("nie.exe"), b"MZ").unwrap();
        std::fs::write(root.join("empty"), b"").unwrap();
        let opts = ScanOptions { page_size: 4096, progress: false };
        let m = scan(&install(root), &opts, None).unwrap();
        assert_eq!(m.files.len(), 3);
        let a = m.find("data/packs/a.cpk").unwrap();
        assert_eq!((a.size, a.pages), (10_000, 3));
        assert_eq!(m.file_ids(a)[0], *blake3::hash(&big[..4096]).as_bytes());
        assert_eq!(m.file_ids(a)[2], *blake3::hash(&big[8192..]).as_bytes());
        assert_eq!(m.find("empty").unwrap().pages, 0);
        let again = scan(&install(root), &opts, Some(&m)).unwrap();
        assert_eq!(again.manifest_id, m.manifest_id);
        std::fs::write(root.join("nie.exe"), b"MZ2").unwrap();
        let changed = scan(&install(root), &opts, Some(&m)).unwrap();
        assert_ne!(changed.manifest_id, m.manifest_id);
    }
}
