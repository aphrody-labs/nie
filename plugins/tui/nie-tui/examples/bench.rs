//! Timings of the explorer on a real game: the VFS tree, a directory listing, the fuzzy search and
//! the first preview of each kind of file. Prints one JSON document.
//!
//! ```sh
//! NIE_GAME_SOURCE=ssh://host/path cargo run --release -p nie-tui --example bench -- 15
//! ```

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use nie_explore::fm::preview::{self, Context, PreviewRequest};
use nie_explore::fm::search::Searcher;
use nie_explore::fm::thumbs::ThumbCache;
use nie_explore::fm::{TreeMode, VfsTree};

fn ms(d: Duration) -> f64 {
    (d.as_secs_f64() * 1000.0 * 100.0).round() / 100.0
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let at = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[at]
}

fn main() -> Result<()> {
    let per_kind: usize = std::env::args().nth(1).and_then(|n| n.parse().ok()).unwrap_or(10);
    let t = Instant::now();
    let vfs = nie_tui::app::open_vfs(None)?;
    let mount = t.elapsed();

    let t = Instant::now();
    let tree = Arc::new(VfsTree::from_vfs(&vfs, TreeMode::Path));
    let tree_build = t.elapsed();
    let t = Instant::now();
    let cpk_tree = VfsTree::from_vfs(&vfs, TreeMode::Cpk);
    let cpk_build = t.elapsed();

    let t = Instant::now();
    let menu = tree.resolve("data/common/menu");
    let listing = menu.map(|id| tree.node(id).children.len());
    let listing_time = t.elapsed();

    let t = Instant::now();
    let mut searcher = Searcher::new(Arc::clone(&tree), Arc::new(|| {}));
    searcher.set_query("mainmenu cfg");
    let mut first_hit = None;
    let loop_started = Instant::now();
    let settled = loop {
        let progress = searcher.tick(Duration::from_millis(20));
        if first_hit.is_none() && progress.matched > 0 {
            first_hit = Some(t.elapsed());
        }
        if !progress.running && progress.total == tree.len() as u32 - 1 {
            break (t.elapsed(), progress);
        }
        if loop_started.elapsed() > Duration::from_secs(60) {
            break (t.elapsed(), progress);
        }
    };
    let top = searcher.hits(3).iter().map(|id| tree.path(*id)).collect::<Vec<_>>();

    let cache_dir = tempfile_dir();
    let cache = ThumbCache::open_in(cache_dir.clone(), 1 << 30, Some(vfs.game_data_dir()))?;
    #[allow(clippy::type_complexity)]
    let kinds: [(&str, &dyn Fn(&str) -> bool); 7] = [
        ("texture", &|p| p.ends_with(".g4tx")),
        ("table", &|p| p.ends_with(".cfg.bin") && p.contains("/menu/")),
        ("event", &|p| p.contains("/event_cfg/evt/") && p.ends_with(".cfg.bin")),
        ("lua", &|p| p.ends_with(".lua.bin")),
        ("video", &|p| p.ends_with(".usm")),
        ("model", &|p| p.ends_with(".g4md")),
        ("hex", &|p| p.ends_with(".bin") && !p.ends_with(".cfg.bin") && !p.ends_with(".lua.bin")),
    ];
    let mut paths: Vec<(String, u64)> = vfs.iter_all().map(|e| (e.path.to_owned(), e.size)).collect();
    paths.sort();
    let never = || false;
    let mut report = BTreeMap::new();
    for (kind, matches) in kinds {
        let pool: Vec<&(String, u64)> = paths.iter().filter(|(p, _)| matches(p)).collect();
        let step = (pool.len() / per_kind.max(1)).max(1);
        let sample: Vec<&&(String, u64)> = pool.iter().step_by(step).take(per_kind).collect();
        let (mut cold, mut warm, mut built, mut bodies) = (Vec::new(), Vec::new(), Vec::new(), BTreeMap::<String, usize>::new());
        for (path, size) in sample.iter().map(|p| (&p.0, p.1)) {
            let ctx = Context {
                vfs: &vfs,
                cache: Some(&cache),
                cancelled: &never,
            };
            let request = PreviewRequest::new(path.clone(), size);
            let t = Instant::now();
            let first = preview::build(&ctx, &request);
            cold.push(ms(t.elapsed()));
            built.push(ms(first.took));
            *bodies
                .entry(match &first.body {
                    preview::Body::Image(_) => "image",
                    preview::Body::Frames(_) => "frames",
                    preview::Body::Text(_) => "text",
                    preview::Body::Table(_) => "table",
                    preview::Body::Hex { .. } => "hex",
                    preview::Body::Message(_) => "message",
                }
                .to_owned())
                .or_default() += 1;
            let t = Instant::now();
            let _ = preview::build(&ctx, &request);
            warm.push(ms(t.elapsed()));
        }
        cold.sort_by(f64::total_cmp);
        warm.sort_by(f64::total_cmp);
        report.insert(
            kind,
            serde_json::json!({
                "candidates": pool.len(),
                "sampled": cold.len(),
                "bodies": bodies,
                "first_ms": { "p50": percentile(&cold, 0.5), "p95": percentile(&cold, 0.95), "max": cold.last() },
                "repeat_ms": { "p50": percentile(&warm, 0.5), "p95": percentile(&warm, 0.95) },
            }),
        );
    }
    let all: Vec<f64> = {
        let mut v: Vec<f64> = report
            .values()
            .filter_map(|k| k["first_ms"]["p50"].as_f64())
            .collect();
        v.sort_by(f64::total_cmp);
        v
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "entries": tree.file_count(),
            "cpks": tree.cpks().len(),
            "mount_ms": ms(mount),
            "tree_path_ms": ms(tree_build),
            "tree_cpk_ms": ms(cpk_build),
            "cpk_top_level": cpk_tree.node(VfsTree::ROOT).children.len(),
            "listing_data_common_menu": { "children": listing, "ms": ms(listing_time) },
            "search": {
                "query": "mainmenu cfg",
                "first_hit_ms": first_hit.map(ms),
                "settled_ms": ms(settled.0),
                "matched": settled.1.matched,
                "injected": settled.1.total,
                "top": top,
            },
            "preview_by_kind": report,
            "kind_median_of_medians_ms": percentile(&all, 0.5),
        }))?
    );
    let _ = std::fs::remove_dir_all(cache_dir);
    Ok(())
}

fn tempfile_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("nie-fm-bench-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    dir
}
