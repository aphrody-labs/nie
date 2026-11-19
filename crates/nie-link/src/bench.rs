//! Cold and warm read benchmarks: pages straight from each host, from the local page cache, and bytes
//! of a file read through the linked folder.
use crate::client::Upstream;
use crate::fsio::read_full_at;
use crate::store::PageStore;
use crate::{Manifest, PageId};
use anyhow::Result;
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stat {
    pub label: String,
    pub pages: usize,
    pub bytes: u64,
    pub first_byte_ms_p50: f64,
    pub first_byte_ms_p95: f64,
    pub mb_per_s: f64,
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

/// `count` pages spread evenly over the manifest (distinct ids, so nothing is cached by accident).
fn sample(m: &Manifest, count: usize, salt: usize) -> Vec<(u32, PageId)> {
    let n = m.page_count();
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    let step = (n / count.max(1)).max(1);
    let mut o = salt % step.max(1);
    while out.len() < count && o < n {
        let id = *m.page_id(o as u32).unwrap_or(&[0; 32]);
        if seen.insert(id) {
            out.push((o as u32, id));
        }
        o += step;
    }
    out
}

/// Cold fetches from one host: sequential for latency, then `parallel` connections for throughput.
pub fn host(up: &Upstream, m: &Manifest, count: usize, parallel: usize) -> Result<Vec<Stat>> {
    let seq = sample(m, count, 0);
    let mut firsts = Vec::new();
    let mut bytes = 0u64;
    let t0 = Instant::now();
    for (_, id) in &seq {
        let (b, ttfb) = up.page(id)?;
        bytes += b.len() as u64;
        firsts.push(ttfb.as_secs_f64() * 1000.0);
    }
    let secs = t0.elapsed().as_secs_f64();
    firsts.sort_by(f64::total_cmp);
    let single = Stat {
        label: format!("{} cold, 1 connection", up.name),
        pages: seq.len(),
        bytes,
        first_byte_ms_p50: percentile(&firsts, 0.5),
        first_byte_ms_p95: percentile(&firsts, 0.95),
        mb_per_s: bytes as f64 / 1e6 / secs,
    };

    let par = sample(m, count * parallel, 1);
    let next = AtomicUsize::new(0);
    let total = std::sync::atomic::AtomicU64::new(0);
    let t0 = Instant::now();
    std::thread::scope(|s| {
        for _ in 0..parallel {
            s.spawn(|| {
                while let Some((_, id)) = par.get(next.fetch_add(1, Ordering::Relaxed)) {
                    if let Ok((b, _)) = up.page(id) {
                        total.fetch_add(b.len() as u64, Ordering::Relaxed);
                    }
                }
            });
        }
    });
    let secs = t0.elapsed().as_secs_f64();
    let bytes = total.load(Ordering::Relaxed);
    let multi = Stat {
        label: format!("{} cold, {parallel} connections", up.name),
        pages: par.len(),
        bytes,
        first_byte_ms_p50: single.first_byte_ms_p50,
        first_byte_ms_p95: single.first_byte_ms_p95,
        mb_per_s: bytes as f64 / 1e6 / secs,
    };
    Ok(vec![single, multi])
}

/// Warm reads: the same pages from a fresh on-disk page store.
pub fn warm(up: &Upstream, m: &Manifest, count: usize, dir: &Path) -> Result<Stat> {
    let store = PageStore::open(dir, u64::MAX / 2)?;
    let pages = sample(m, count, 2);
    for (_, id) in &pages {
        store.put(id, &up.page(id)?.0)?;
    }
    let mut lat = Vec::new();
    let mut bytes = 0u64;
    let t0 = Instant::now();
    for (_, id) in &pages {
        let t = Instant::now();
        let b = store.get(id).ok_or_else(|| anyhow::anyhow!("page vanished"))?;
        lat.push(t.elapsed().as_secs_f64() * 1000.0);
        bytes += b.len() as u64;
    }
    let secs = t0.elapsed().as_secs_f64();
    lat.sort_by(f64::total_cmp);
    Ok(Stat {
        label: "page cache warm (read and BLAKE3 check)".into(),
        pages: pages.len(),
        bytes,
        first_byte_ms_p50: percentile(&lat, 0.5),
        first_byte_ms_p95: percentile(&lat, 0.95),
        mb_per_s: bytes as f64 / 1e6 / secs,
    })
}

/// Read up to `limit` bytes of `path` in `chunk`-byte reads: latency of open plus first read, then throughput.
pub fn file(path: &Path, limit: u64, chunk: usize) -> Result<Stat> {
    let t0 = Instant::now();
    let f = std::fs::File::open(path)?;
    let mut buf = vec![0u8; chunk];
    let mut got = read_full_at(&f, &mut buf, 0)?;
    let first = t0.elapsed().as_secs_f64() * 1000.0;
    let mut off = got as u64;
    let mut total = got as u64;
    while total < limit && got == chunk {
        got = read_full_at(&f, &mut buf, off)?;
        off += got as u64;
        total += got as u64;
    }
    let secs = t0.elapsed().as_secs_f64();
    Ok(Stat {
        label: path.display().to_string(),
        pages: total.div_ceil(chunk as u64) as usize,
        bytes: total,
        first_byte_ms_p50: first,
        first_byte_ms_p95: first,
        mb_per_s: total as f64 / 1e6 / secs,
    })
}
