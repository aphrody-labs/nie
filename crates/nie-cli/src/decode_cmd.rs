//! `nie decode` (file or tree -> JSON / PNG) and `nie format` (detection only).
//!
//! Format dispatch is not reimplemented here: it comes from [`nie_formats::decode`], shared with
//! the FFI. Parallelism comes from rayon.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, bail};
use rayon::prelude::*;

/// Extensions decoded to JSON.
const EXT_JSON: &[&str] = &[
    "bin", "objbin", "g4pkm", "lip", "p3lip", "mev", "mevbin", "g4md", "g4pk", "cfg",
];
/// Extensions decoded to PNG.
const EXT_PNG: &[&str] = &["g4tx"];

/// Decodes a buffer: returns `(bytes, output extension)`.
///
/// G4TX goes through the texture decoder (PNG); everything else goes through the JSON dispatch.
/// `basename` (the source file name without extension) disambiguates multi-texture containers;
/// without it, an `icon_item05.g4tx` would render one of its icons at random.
fn decode_bytes(
    data: &[u8],
    basename: &str,
    as_png: bool,
) -> Option<(Vec<u8>, &'static str, &'static str)> {
    if as_png {
        return nie_formats::g4tx_decode::decode_best_to_png(data, basename)
            .map(|png| (png, "png", "g4tx"));
    }
    // Lua bytecode is routed by `nie-formats` itself (feature `lua`), so the FFI and the MCP
    // decode it too. One dispatch table, as the header says.
    nie_formats::decode::decode(data).map(|d| (d.json, "json", d.format))
}

/// Decodes a single file to `out` (or next to the source if `out` is a directory).
///
/// # Errors
/// Unreadable input, unrecognized format, or write failure.
pub fn file(src: &Path, out: Option<&Path>, quiet: bool) -> anyhow::Result<()> {
    let data = fs::read(src).with_context(|| format!("reading {}", src.display()))?;
    let as_png = src
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXT_PNG.contains(&e.to_ascii_lowercase().as_str()));

    let basename = src.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    let Some((bytes, ext, format)) = decode_bytes(&data, basename, as_png) else {
        bail!(
            "undecoded format: {} ({})",
            src.display(),
            nie_formats::decode::format_name(&data)
        );
    };

    let dest = match out {
        Some(p) if p.is_dir() => p
            .join(src.file_name().unwrap_or_default())
            .with_extension(ext),
        Some(p) => p.to_path_buf(),
        None => src.with_extension(ext),
    };
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(&dest, &bytes).with_context(|| format!("writing {}", dest.display()))?;
    if !quiet {
        println!(
            "decode={} format={} out={} bytes={}",
            src.display(),
            format,
            dest.display(),
            bytes.len()
        );
    }
    Ok(())
}

/// Recursively lists the decodable files of a directory.
fn collect(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(collect(&p));
        } else if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            let ext = ext.to_ascii_lowercase();
            if EXT_JSON.contains(&ext.as_str()) || EXT_PNG.contains(&ext.as_str()) {
                out.push(p);
            }
        }
    }
    out
}

/// Recursively decodes a directory to `out`, in parallel (rayon).
///
/// The source tree is preserved under `out`. Files whose format is not recognized are counted,
/// not fatal: a full dump always contains unknown files.
///
/// # Errors
/// If the source is unreadable.
pub fn dir(src: &Path, out: &Path, quiet: bool) -> anyhow::Result<()> {
    let files = collect(src);
    if files.is_empty() {
        bail!("no decodable file under {}", src.display());
    }
    let ok = AtomicUsize::new(0);
    let skipped = AtomicUsize::new(0);

    files.par_iter().for_each(|p| {
        let Ok(data) = fs::read(p) else {
            skipped.fetch_add(1, Ordering::Relaxed);
            return;
        };
        let as_png = p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| EXT_PNG.contains(&e.to_ascii_lowercase().as_str()));
        let basename = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
        let Some((bytes, ext, _format)) = decode_bytes(&data, basename, as_png) else {
            skipped.fetch_add(1, Ordering::Relaxed);
            return;
        };
        let rel = p.strip_prefix(src).unwrap_or(p);
        let dest = out.join(rel).with_extension(ext);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).ok();
        }
        if fs::write(&dest, &bytes).is_ok() {
            ok.fetch_add(1, Ordering::Relaxed);
        } else {
            skipped.fetch_add(1, Ordering::Relaxed);
        }
    });

    if !quiet {
        println!(
            "decode-dir={} out={} ok={} skipped={} total={}",
            src.display(),
            out.display(),
            ok.load(Ordering::Relaxed),
            skipped.load(Ordering::Relaxed),
            files.len()
        );
    }
    Ok(())
}

/// Recursively lists the `*.cfg.bin` files of a directory (RDBN/T2B family only; textures and
/// Lua have no tagged form).
#[cfg(feature = "host")]
fn collect_cfg_bin(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(collect_cfg_bin(&p));
        } else if p.to_string_lossy().ends_with(".cfg.bin") {
            out.push(p);
        }
    }
    out
}

/// Regenerates, next to each `*.cfg.bin` under `dir`, the `*.cfg.bin.json` in **tagged** form
/// (`nie_formats::cfgbin::to_tagged_json`, RDBN `{lists}` / T2B `{entries}`). This is the form
/// read by the typed parsers of `nie-data` (golden tests, `export_*`), as opposed to the raw
/// structural JSON that `nie decode` emits without `--typed`.
///
/// `force=false` skips a `.json` already newer than its `.cfg.bin` (same convention as
/// `DataPathExporter.cs`), so the call is idempotent on an up-to-date corpus.
///
/// # Errors
/// If `dir` contains no `.cfg.bin`.
#[cfg(feature = "host")]
pub fn refresh_typed(dir: &Path, force: bool, quiet: bool) -> anyhow::Result<()> {
    let files = collect_cfg_bin(dir);
    if files.is_empty() {
        bail!("no .cfg.bin under {}", dir.display());
    }
    let ok = AtomicUsize::new(0);
    let skipped_fresh = AtomicUsize::new(0);
    let skipped_unparsed = AtomicUsize::new(0);

    files.par_iter().for_each(|p| {
        let dest = {
            let mut s = p.as_os_str().to_owned();
            s.push(".json");
            PathBuf::from(s)
        };
        if !force
            && let (Ok(src_meta), Ok(dst_meta)) = (fs::metadata(p), fs::metadata(&dest))
            && let (Ok(src_t), Ok(dst_t)) = (src_meta.modified(), dst_meta.modified())
            && dst_t >= src_t
        {
            skipped_fresh.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let Ok(data) = fs::read(p) else {
            skipped_unparsed.fetch_add(1, Ordering::Relaxed);
            return;
        };
        let Some(json) = nie_formats::cfgbin::to_tagged_json(&data) else {
            skipped_unparsed.fetch_add(1, Ordering::Relaxed);
            return;
        };
        let Ok(bytes) = serde_json::to_vec(&json) else {
            skipped_unparsed.fetch_add(1, Ordering::Relaxed);
            return;
        };
        if fs::write(&dest, &bytes).is_ok() {
            ok.fetch_add(1, Ordering::Relaxed);
        } else {
            skipped_unparsed.fetch_add(1, Ordering::Relaxed);
        }
    });

    if !quiet {
        println!(
            "refresh-typed={} total={} ok={} skipped_fresh={} skipped_unparsed={}",
            dir.display(),
            files.len(),
            ok.load(Ordering::Relaxed),
            skipped_fresh.load(Ordering::Relaxed),
            skipped_unparsed.load(Ordering::Relaxed)
        );
    }
    Ok(())
}

/// Reports the format of a file or of every file in a tree, writing nothing.
///
/// Two distinct columns: `detect` is what the magic bytes say, `decode` is the parser that
/// actually succeeds. They diverge on T2B containers, which detection reports as `unknown`.
///
/// # Errors
/// If the source is unreadable or a directory contains no decodable file.
pub fn format(src: &Path) -> anyhow::Result<()> {
    let files = if src.is_dir() {
        let f = collect(src);
        if f.is_empty() {
            bail!("no decodable file under {}", src.display());
        }
        f
    } else {
        vec![src.to_path_buf()]
    };

    for p in &files {
        let data = fs::read(p).with_context(|| format!("reading {}", p.display()))?;
        let detect = nie_formats::decode::format_name(&data);
        let decode = nie_formats::decode::decode(&data).map_or("-", |d| d.format);
        println!(
            "path={} detect={} decode={} bytes={}",
            p.display(),
            detect,
            decode,
            data.len()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_extensions_png_et_json_sont_disjointes() {
        // The same suffix in both lists would make the output non-deterministic.
        for e in EXT_PNG {
            assert!(!EXT_JSON.contains(e), "{e} is in both lists");
        }
    }

    #[test]
    fn un_tampon_de_bruit_n_est_pas_decode() {
        assert!(decode_bytes(&[0xAB; 64], "bruit", false).is_none());
    }

    #[test]
    fn collect_ignore_les_extensions_inconnues() {
        let dir = std::env::temp_dir().join("nie-decode-test");
        let _ = fs::create_dir_all(&dir);
        let _ = fs::write(dir.join("a.txt"), b"x");
        assert!(collect(&dir).iter().all(|p| p.extension().is_some()));
        assert!(!collect(&dir).iter().any(|p| p.ends_with("a.txt")));
    }
}
