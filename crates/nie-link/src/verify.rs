//! Hash every file of the linked folder through the filesystem and compare it with the origin manifest
//! (page by page, BLAKE3) and with Steam's own depot manifest (size and SHA-1).
use crate::fsio::read_full_at;
use crate::steam::{self, SteamMatch};
use crate::Manifest;
use anyhow::Result;
use rayon::prelude::*;
use serde::Serialize;
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mismatch {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub manifest_id: String,
    pub build_id: String,
    pub root: String,
    pub files: usize,
    pub bytes: u64,
    pub pages: u64,
    pub files_ok: usize,
    pub missing: Vec<String>,
    pub mismatched: Vec<Mismatch>,
    /// Files below the root that the manifest does not list (local files such as logs).
    pub extra: Vec<String>,
    pub steam: Option<SteamMatch>,
    pub seconds: f64,
    pub mb_per_s: f64,
}

impl Report {
    #[must_use]
    pub fn clean(&self) -> bool {
        self.missing.is_empty()
            && self.mismatched.is_empty()
            && self
                .steam
                .as_ref()
                .is_none_or(|s| s.missing.is_empty() && s.unlisted == 0)
    }
}

pub struct Options {
    pub threads: usize,
    /// Only files whose path starts with one of these (empty: all).
    pub only: Vec<String>,
    pub steam_manifest: Option<Vec<u8>>,
    pub progress: bool,
}

fn native(root: &Path, rel: &str) -> PathBuf {
    rel.split('/').fold(root.to_path_buf(), |p, part| p.join(part))
}

fn extra_files(root: &Path, m: &Manifest) -> Vec<String> {
    let known: std::collections::HashSet<String> = m.files.iter().map(|f| f.path.to_uppercase()).collect();
    let mut out = Vec::new();
    let mut stack = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, rel)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = if rel.is_empty() { name } else { format!("{rel}/{name}") };
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push((e.path(), path)),
                Ok(t) if t.is_file() && !known.contains(&path.to_uppercase()) => out.push(path),
                _ => {}
            }
        }
    }
    out.sort();
    out
}

struct One {
    pages: u64,
    sha1: [u8; 20],
    size: u64,
    problem: Option<String>,
}

fn check_file(root: &Path, m: &Manifest, idx: usize, done: &AtomicU64) -> std::io::Result<One> {
    let f = &m.files[idx];
    let file = std::fs::File::open(native(root, &f.path))?;
    let size = file.metadata()?.len();
    let mut out = One { pages: 0, sha1: [0; 20], size, problem: None };
    if size != f.size {
        out.problem = Some(format!("size {size}, origin {}", f.size));
        return Ok(out);
    }
    let ids = m.file_ids(f);
    let mut buf = vec![0u8; m.page_size as usize];
    let mut sha = Sha1::new();
    let mut bad: Vec<usize> = Vec::new();
    for (i, id) in ids.iter().enumerate() {
        let want = m.page_len(f, i as u32);
        let got = read_full_at(&file, &mut buf[..want], i as u64 * u64::from(m.page_size))?;
        if got != want {
            out.problem = Some(format!("short read at page {i}: {got} of {want}"));
            return Ok(out);
        }
        sha.update(&buf[..want]);
        if blake3::hash(&buf[..want]).as_bytes() != id {
            bad.push(i);
        }
        out.pages += 1;
        done.fetch_add(want as u64, Ordering::Relaxed);
    }
    out.sha1 = sha.finalize().into();
    if !bad.is_empty() {
        let shown: Vec<String> = bad.iter().take(8).map(ToString::to_string).collect();
        out.problem = Some(format!("{} of {} pages differ ({}...)", bad.len(), ids.len(), shown.join(",")));
    }
    Ok(out)
}

/// Verify `root` against `manifest`.
pub fn run(root: &Path, m: &Manifest, opts: &Options) -> Result<Report> {
    let t0 = Instant::now();
    let selected: Vec<usize> = (0..m.files.len())
        .filter(|&i| opts.only.is_empty() || opts.only.iter().any(|p| m.files[i].path.starts_with(p.as_str())))
        .collect();
    let total_bytes: u64 = selected.iter().map(|&i| m.files[i].size).sum();
    let done = AtomicU64::new(0);
    let finished = std::sync::atomic::AtomicBool::new(false);
    let missing = Mutex::new(Vec::new());
    let mismatched = Mutex::new(Vec::new());
    let locals = Mutex::new(Vec::new());
    let pool = rayon::ThreadPoolBuilder::new().num_threads(opts.threads.max(1)).build()?;
    let pages = AtomicU64::new(0);
    let ok_files = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        if opts.progress {
            s.spawn(|| {
                let mut last = Instant::now();
                while !finished.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(200));
                    if last.elapsed() >= Duration::from_secs(5) {
                        last = Instant::now();
                        let d = done.load(Ordering::Relaxed);
                        eprintln!(
                            "verify {:.1}/{:.1} GiB {:.0} MB/s ok {}",
                            d as f64 / 1073741824.0,
                            total_bytes as f64 / 1073741824.0,
                            d as f64 / 1e6 / t0.elapsed().as_secs_f64().max(0.001),
                            ok_files.load(Ordering::Relaxed)
                        );
                    }
                }
            });
        }
        pool.install(|| {
            selected.par_iter().for_each(|&i| {
                let f = &m.files[i];
                match check_file(root, m, i, &done) {
                    Ok(one) => {
                        pages.fetch_add(one.pages, Ordering::Relaxed);
                        match one.problem {
                            None => {
                                ok_files.fetch_add(1, Ordering::Relaxed);
                                locals.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push((one.size, one.sha1));
                            }
                            Some(reason) => mismatched
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner)
                                .push(Mismatch { path: f.path.clone(), reason }),
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        missing.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(f.path.clone());
                    }
                    Err(e) => mismatched
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .push(Mismatch { path: f.path.clone(), reason: format!("read error: {e}") }),
                }
            });
        });
        finished.store(true, Ordering::Relaxed);
    });
    let mut missing = missing.into_inner().unwrap_or_default();
    let mut mismatched = mismatched.into_inner().unwrap_or_default();
    missing.sort();
    mismatched.sort_by(|a, b| a.path.cmp(&b.path));
    let locals = locals.into_inner().unwrap_or_default();
    let steam = match (&opts.steam_manifest, opts.only.is_empty()) {
        (Some(bytes), true) => Some(steam::match_steam(&steam::parse_depot_manifest(bytes)?, &locals)),
        _ => None,
    };
    let seconds = t0.elapsed().as_secs_f64();
    let bytes = done.load(Ordering::Relaxed);
    Ok(Report {
        manifest_id: m.manifest_id.clone(),
        build_id: m.build_id.clone(),
        root: root.display().to_string(),
        files: selected.len(),
        bytes,
        pages: pages.load(Ordering::Relaxed),
        files_ok: ok_files.load(Ordering::Relaxed),
        missing,
        mismatched,
        extra: if opts.only.is_empty() { extra_files(root, m) } else { Vec::new() },
        steam,
        seconds,
        mb_per_s: bytes as f64 / 1e6 / seconds.max(0.001),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::{self, ScanOptions};
    use crate::steam::Install;

    #[test]
    fn detects_missing_corrupt_and_extra_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("d")).unwrap();
        std::fs::write(root.join("d/a.bin"), vec![5u8; 9000]).unwrap();
        std::fs::write(root.join("b.bin"), vec![6u8; 100]).unwrap();
        std::fs::write(root.join("c.bin"), vec![7u8; 5000]).unwrap();
        let install = Install {
            app_id: 1, build_id: "1".into(), install_dir: "g".into(), root: root.to_path_buf(),
            acf: PathBuf::new(), depot_manifest: None, depot_manifest_file: None,
        };
        let m = scan::scan(&install, &ScanOptions { page_size: 4096, progress: false }, None).unwrap();
        let opts = Options { threads: 2, only: vec![], steam_manifest: None, progress: false };
        let r = run(root, &m, &opts).unwrap();
        assert!(r.clean());
        assert_eq!((r.files, r.files_ok, r.pages), (3, 3, 3 + 1 + 2));

        let mut flipped = vec![5u8; 9000];
        flipped[5000] = 0;
        std::fs::write(root.join("d/a.bin"), flipped).unwrap();
        std::fs::remove_file(root.join("b.bin")).unwrap();
        std::fs::write(root.join("log.txt"), b"x").unwrap();
        let r = run(root, &m, &opts).unwrap();
        assert!(!r.clean());
        assert_eq!(r.missing, vec!["b.bin"]);
        assert_eq!(r.mismatched.len(), 1);
        assert!(r.mismatched[0].reason.contains("1 of 3 pages"));
        assert_eq!(r.extra, vec!["log.txt"]);
    }
}
