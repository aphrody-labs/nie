//! Read every file of the linked folder once so that ProjFS stores all of it locally. ProjFS asks its
//! provider for a whole file on the first read of any byte, so this is the way to pay the transfer
//! before the game starts instead of at its first access to each archive.
use crate::Manifest;
use crate::fsio::read_full_at;
use anyhow::Result;
use rayon::prelude::*;
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub files: usize,
    pub bytes: u64,
    pub failed: Vec<String>,
    pub seconds: f64,
    pub mb_per_s: f64,
}

pub fn run(root: &Path, m: &Manifest, threads: usize, only: &[String], progress: bool) -> Result<Summary> {
    let t0 = Instant::now();
    let mut order: Vec<usize> = (0..m.files.len())
        .filter(|&i| only.is_empty() || only.iter().any(|p| m.files[i].path.starts_with(p.as_str())))
        .collect();
    // Small files first: the executable and the tables the game opens first arrive early.
    order.sort_by_key(|&i| m.files[i].size);
    let total: u64 = order.iter().map(|&i| m.files[i].size).sum();
    let done = AtomicU64::new(0);
    let files_done = AtomicUsize::new(0);
    let finished = std::sync::atomic::AtomicBool::new(false);
    let failed = std::sync::Mutex::new(Vec::new());
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads.max(1)).build()?;
    std::thread::scope(|s| {
        if progress {
            s.spawn(|| {
                let mut last = Instant::now();
                while !finished.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(200));
                    if last.elapsed() >= Duration::from_secs(10) {
                        last = Instant::now();
                        let d = done.load(Ordering::Relaxed);
                        eprintln!(
                            "hydrate {}/{} files {:.1}/{:.1} GiB {:.1} MB/s",
                            files_done.load(Ordering::Relaxed),
                            order.len(),
                            d as f64 / 1073741824.0,
                            total as f64 / 1073741824.0,
                            d as f64 / 1e6 / t0.elapsed().as_secs_f64().max(0.001)
                        );
                    }
                }
            });
        }
        pool.install(|| {
            order.par_iter().for_each(|&i| {
                let f = &m.files[i];
                let path = f.path.split('/').fold(root.to_path_buf(), |p, part| p.join(part));
                let r = (|| -> std::io::Result<()> {
                    let file = std::fs::File::open(&path)?;
                    let mut buf = vec![0u8; 4 << 20];
                    let mut off = 0u64;
                    while off < f.size {
                        let n = read_full_at(&file, &mut buf, off)?;
                        if n == 0 {
                            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "short file"));
                        }
                        off += n as u64;
                        done.fetch_add(n as u64, Ordering::Relaxed);
                    }
                    Ok(())
                })();
                files_done.fetch_add(1, Ordering::Relaxed);
                if let Err(e) = r {
                    failed.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(format!("{}: {e}", f.path));
                }
            });
        });
        finished.store(true, Ordering::Relaxed);
    });
    let seconds = t0.elapsed().as_secs_f64();
    let bytes = done.load(Ordering::Relaxed);
    Ok(Summary {
        files: order.len(),
        bytes,
        failed: failed.into_inner().unwrap_or_default(),
        seconds,
        mb_per_s: bytes as f64 / 1e6 / seconds.max(0.001),
    })
}
