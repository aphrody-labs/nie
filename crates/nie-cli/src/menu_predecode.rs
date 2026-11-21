//! Menu sprite pre-decoding: g4tx -> DDS -> RGBA -> PNG written to the disk dump.
//!
//! # How it works
//!
//! For each g4tx path under `data/dx11/menu/**` indexed in Redis db3 (`iev:file:index`):
//!
//! 1. Read the file from the CPK (via `CpkReader` and the Redis index).
//! 2. Parse the G4TX container to extract the DDS payload.
//! 3. Decode the DDS (BC1/BC3/BC7/RGBA8) to RGBA via `image_dds`.
//! 4. Encode as PNG and write to `<dump_root>/<relative path without data/>.png` (idempotent).
//!
//! The CLI scans every indexed menu texture, covering localized and base variants.
//!
//! Concurrency: Rayon pool (threads = CPU count, saturated by BCn decoding).

#![allow(clippy::pedantic)]

use std::{
    collections::HashMap,
    io::Cursor,
    path::Path,
    sync::atomic::{AtomicU32, Ordering},
};

use anyhow::Context;
use rayon::prelude::*;

use image_dds::ddsfile::Dds;
use nie_explore::menu_predecode::plan;
use nie_formats::cpk::CpkReader;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

pub struct PredecodeStats {
    pub decoded: u32,
    pub skipped: u32,
    pub failed: u32,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Pre-decodes all targeted menu sprites and writes the PNGs under `dump_root`.
///
/// `priority_paths`: if non-empty, these paths are processed first; the rest of the menu
/// subtree is processed if `all_menu` is true.
pub fn run(
    dump_root: &Path,
    packs_dir: &Path,
    redis_url: &str,
    priority_paths: &[String],
    all_menu: bool,
) -> anyhow::Result<PredecodeStats> {
    // --- Redis connection ---
    let client =
        redis::Client::open(redis_url).with_context(|| format!("connecting to Redis {redis_url}"))?;
    let mut conn = client.get_connection().context("get_connection Redis")?;

    // --- Collect the paths to process ---
    let mut indexed = Vec::new();

    // Resolve explicitly prioritized paths first. The current CLI passes none;
    // library callers may still provide paths from their own index.
    for p in priority_paths {
        // A logical VFS path such as "dx11/menu/..." maps to the Redis key
        // "data/dx11/menu/...".
        let redis_key = format!("data/{p}");
        let cpk: Option<String> = redis::cmd("HGET")
            .arg("iev:file:index")
            .arg(&redis_key)
            .query(&mut conn)
            .ok()
            .flatten();
        if cpk.is_some() {
            indexed.push((redis_key, cpk.unwrap_or_default()));
        }
    }

    if all_menu {
        // HSCAN over every menu g4tx (all locales and base).
        let mut cursor: u64 = 0;
        loop {
            let result: redis::Value = redis::cmd("HSCAN")
                .arg("iev:file:index")
                .arg(cursor)
                .arg("MATCH")
                .arg("data/dx11/menu*g4tx")
                .arg("COUNT")
                .arg(10_000u32)
                .query(&mut conn)?;

            let (next_cursor, pairs) = parse_hscan_result(result)?;
            indexed.extend(pairs);
            cursor = next_cursor;
            if cursor == 0 {
                break;
            }
        }
    }

    let plan = plan(priority_paths, &indexed, all_menu, &["fr", "en"]);
    let total = plan.paths.len();
    eprintln!("predecode total={total} sprites to process");

    // --- Parallel processing ---
    let decoded = AtomicU32::new(0);
    let skipped = AtomicU32::new(0);
    let failed = AtomicU32::new(0);

    // CPK indexes already opened: loaded once per CPK (mmap-like via std::fs::read).
    // To avoid re-reading the same CPK several times, sprites are grouped by CPK.
    for path in &plan.missing_archive {
        eprintln!("warning: no CPK for {path}");
        failed.fetch_add(1, Ordering::Relaxed);
    }

    let cpk_groups: Vec<(String, Vec<String>)> = plan.by_archive.into_iter().collect();
    let total_cpks = cpk_groups.len();
    eprintln!("predecode {total_cpks} distinct CPKs");

    cpk_groups
        .into_par_iter()
        .for_each(|(cpk_name, sprite_paths)| {
            let cpk_path = packs_dir.join(&cpk_name);
            let cpk_data = match std::fs::read(&cpk_path) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("error: reading CPK {cpk_name}: {e}");
                    failed.fetch_add(sprite_paths.len() as u32, Ordering::Relaxed);
                    return;
                }
            };

            let reader = match CpkReader::new(&cpk_data, &cpk_name) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("error: parse CPK {cpk_name}: {e}");
                    failed.fetch_add(sprite_paths.len() as u32, Ordering::Relaxed);
                    return;
                }
            };

            // Index by logical path (directory + "/" + filename).
            let entry_map: HashMap<String, usize> = reader
                .entries
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    let logical = if e.directory.is_empty() {
                        e.filename.clone()
                    } else {
                        format!("{}/{}", e.directory, e.filename)
                    };
                    (logical, i)
                })
                .collect();

            // Sprites of one archive decode in parallel too: a few large archives would
            // otherwise leave most cores idle (BCn decode and PNG encode dominate).
            sprite_paths.par_iter().for_each(|sprite_path| {
                // sprite_path = "dx11/menu/..."
                let png_out =
                    dump_root.join(format!("{}.png", sprite_path.trim_end_matches(".g4tx")));

                // Idempotent: skip if the PNG exists and is non-empty.
                if png_out.exists() {
                    let meta = std::fs::metadata(&png_out).ok();
                    if meta.is_some_and(|m| m.len() > 0) {
                        skipped.fetch_add(1, Ordering::Relaxed);
                        return;
                    }
                }

                // Look up the entry in the CPK.
                // The path inside the CPK is "data/<sprite_path>".
                let cpk_inner = format!("data/{sprite_path}");
                let entry_idx = entry_map.get(&cpk_inner).copied().or_else(|| {
                    // Fallback: the path without "data/"
                    entry_map.get(sprite_path).copied()
                });

                let entry_idx = match entry_idx {
                    Some(i) => i,
                    None => {
                        eprintln!("warning: entry not found in CPK {cpk_name}: {cpk_inner}");
                        failed.fetch_add(1, Ordering::Relaxed);
                        return;
                    }
                };

                let entry = &reader.entries[entry_idx];
                let g4tx_bytes = match reader.extract(&cpk_data, entry) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("error: extraction {sprite_path}: {e}");
                        failed.fetch_add(1, Ordering::Relaxed);
                        return;
                    }
                };

                match g4tx_to_png(&g4tx_bytes, sprite_path) {
                    Ok(png_bytes) => {
                        if let Err(e) = write_png(&png_out, &png_bytes) {
                            eprintln!("error: writing PNG {}: {e}", png_out.display());
                            failed.fetch_add(1, Ordering::Relaxed);
                        } else {
                            decoded.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Err(e) => {
                        eprintln!("error: decoding {sprite_path}: {e}");
                        failed.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
        });

    Ok(PredecodeStats {
        decoded: decoded.load(Ordering::Relaxed),
        skipped: skipped.load(Ordering::Relaxed),
        failed: failed.load(Ordering::Relaxed),
    })
}

// ---------------------------------------------------------------------------
// G4TX -> PNG decoding
// ---------------------------------------------------------------------------

/// Decodes a g4tx file (DDS payload) to PNG.
/// Only the first main texture (mip 0) is exported.
fn g4tx_to_png(data: &[u8], debug_path: &str) -> anyhow::Result<Vec<u8>> {
    // Parse the G4TX container to find the DDS payload.
    let g4tx =
        nie_formats::g4tx::parse(data).with_context(|| format!("parse G4TX {debug_path}"))?;

    let tex = g4tx
        .textures
        .first()
        .ok_or_else(|| anyhow::anyhow!("empty g4tx (no texture): {debug_path}"))?;

    let payload = data
        .get(tex.data_offset..tex.data_offset + tex.data_size)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "DDS payload out of bounds offset={} size={} total={}: {debug_path}",
                tex.data_offset,
                tex.data_size,
                data.len()
            )
        })?;

    // Decode the DDS to an RgbaImage via image_dds.
    let dds = Dds::read(Cursor::new(payload)).with_context(|| format!("parse DDS {debug_path}"))?;

    let rgba_image =
        image_dds::image_from_dds(&dds, 0).with_context(|| format!("decode DDS {debug_path}"))?;

    // Encode as lossless PNG with fast deflate: these PNGs are a disk cache read back by the
    // service, so size matters less than pre-decoding time.
    let mut png_bytes: Vec<u8> = Vec::new();
    image::ImageEncoder::write_image(
        image::codecs::png::PngEncoder::new_with_quality(
            Cursor::new(&mut png_bytes),
            image::codecs::png::CompressionType::Fast,
            image::codecs::png::FilterType::Adaptive,
        ),
        rgba_image.as_raw(),
        rgba_image.width(),
        rgba_image.height(),
        image::ExtendedColorType::Rgba8,
    )
    .with_context(|| format!("encode PNG {debug_path}"))?;

    Ok(png_bytes)
}

// ---------------------------------------------------------------------------
// PNG writing (creates parent directories)
// ---------------------------------------------------------------------------

fn write_png(out: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating directories {}", parent.display()))?;
    }
    std::fs::write(out, bytes).with_context(|| format!("writing {}", out.display()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Redis helpers
// ---------------------------------------------------------------------------

/// Parses a Redis HSCAN result into (next_cursor, Vec<(field, value)>).
fn parse_hscan_result(val: redis::Value) -> anyhow::Result<(u64, Vec<(String, String)>)> {
    use redis::Value;
    if let Value::Array(outer) = val {
        if outer.len() != 2 {
            anyhow::bail!("HSCAN: unexpected response format (len={})", outer.len());
        }
        let cursor_val = &outer[0];
        let items_val = &outer[1];

        let cursor: u64 = match cursor_val {
            Value::BulkString(b) => String::from_utf8_lossy(b).parse().unwrap_or(0),
            Value::Int(n) => *n as u64,
            _ => 0,
        };

        let mut pairs = Vec::new();
        if let Value::Array(items) = items_val {
            let mut i = 0;
            while i + 1 < items.len() {
                let field = redis_value_to_string(&items[i]);
                let value = redis_value_to_string(&items[i + 1]);
                if let (Some(f), Some(v)) = (field, value) {
                    pairs.push((f, v));
                }
                i += 2;
            }
        }
        Ok((cursor, pairs))
    } else {
        anyhow::bail!("HSCAN: response is not an Array");
    }
}

fn redis_value_to_string(val: &redis::Value) -> Option<String> {
    use redis::Value;
    match val {
        Value::BulkString(b) => Some(String::from_utf8_lossy(b).into_owned()),
        Value::SimpleString(s) => Some(s.clone()),
        _ => None,
    }
}
