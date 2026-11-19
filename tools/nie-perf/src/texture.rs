//! Texture path benchmark: BC7 DDS payload to the RGBA8 an in-process consumer uploads.
//!
//! - `via-png`: BC7 -> RGBA8 -> `nie_formats::g4tx_decode::encode_rgba_to_png` -> PNG decode -> RGBA8
//!   (the path of a GLB with embedded PNG atlases, default deflate level).
//! - `via-png-fast` (feature `fast-png`): the same through `encode_rgba_to_png_fast`.
//! - `direct`: BC7 -> RGBA8 handed over as is (the in-process path that skips PNG).
//!
//! The witness is the BLAKE3 of the final RGBA8 of every texture: equal in every mode.

use std::time::Instant;

use anyhow::{Context, Result, bail};
use nie_formats::g4tx_decode::{decode_dds_to_rgba, encode_rgba_to_png};
use serde_json::{Value, json};

use crate::Args;

/// A BC7 (DXGI 98) DDS with a DX10 header and mode-6 blocks. Blocks come from a pool of 64
/// pseudo-random blocks chosen per 32x32 region with a per-block index perturbation, so the
/// decoded image has flat regions and noise like an atlas, and deflate has something to find.
fn bc7_dds(side: u32, seed: u64) -> Vec<u8> {
    let blocks_per_row = side / 4;
    let mut state = 0x9E37_79B9_7F4A_7C15u64 ^ seed.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    let mut next = move || {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        state.wrapping_mul(0x2545_F491_4F6C_DD1D)
    };
    let pool: Vec<[u8; 16]> = (0..64)
        .map(|_| {
            let mut block = [0u8; 16];
            for chunk in block.chunks_mut(8) {
                chunk.copy_from_slice(&next().to_le_bytes());
            }
            block[0] = 0x40 | (block[0] & 0x80); // mode 6: lowest set bit is bit 6
            block
        })
        .collect();

    let mut dds = vec![0u8; 148];
    dds[0..4].copy_from_slice(b"DDS ");
    dds[4..8].copy_from_slice(&124u32.to_le_bytes());
    dds[8..12].copy_from_slice(&0x0000_1007u32.to_le_bytes());
    dds[12..16].copy_from_slice(&side.to_le_bytes());
    dds[16..20].copy_from_slice(&side.to_le_bytes());
    dds[28..32].copy_from_slice(&1u32.to_le_bytes());
    dds[76..80].copy_from_slice(&32u32.to_le_bytes());
    dds[80..84].copy_from_slice(&4u32.to_le_bytes());
    dds[84..88].copy_from_slice(b"DX10");
    dds[128..132].copy_from_slice(&98u32.to_le_bytes());
    dds[132..136].copy_from_slice(&3u32.to_le_bytes());
    dds[140..144].copy_from_slice(&1u32.to_le_bytes());
    for by in 0..blocks_per_row {
        for bx in 0..blocks_per_row {
            let region = ((by / 8) * 131 + (bx / 8) * 17) as usize % pool.len();
            let mut block = pool[region];
            let r = next();
            if r & 3 == 0 {
                block[8..].copy_from_slice(&r.to_le_bytes());
            }
            dds.extend_from_slice(&block);
        }
    }
    dds
}

/// What `nie_render3d::glb::decode_png` does for an RGBA PNG, without the budget checks.
fn decode_png(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().context("png read_info")?;
    let mut buf = vec![0u8; reader.output_buffer_size().context("png size")?];
    let info = reader.next_frame(&mut buf).context("png next_frame")?;
    if info.color_type != png::ColorType::Rgba {
        bail!("expected RGBA, got {:?}", info.color_type);
    }
    buf.truncate(info.buffer_size());
    Ok(buf)
}

pub fn run(args: &Args) -> Result<Value> {
    let mode = args.string("mode").unwrap_or("via-png").to_owned();
    let side = args.number("size", 1024u32)?;
    let count = args.number("count", 16usize)?;
    if !["via-png", "via-png-fast", "direct"].contains(&mode.as_str()) {
        bail!("--mode must be via-png, via-png-fast or direct");
    }
    let sources: Vec<Vec<u8>> = (0..count).map(|i| bc7_dds(side, i as u64 + 1)).collect();

    let start = Instant::now();
    let mut witness = blake3::Hasher::new();
    let mut png_bytes = 0usize;
    for dds in &sources {
        let (w, h, rgba) = decode_dds_to_rgba(dds).context("BC7 decode")?;
        let final_rgba = match mode.as_str() {
            "direct" => rgba,
            "via-png-fast" => {
                #[cfg(feature = "fast-png")]
                let png = nie_formats::g4tx_decode::encode_rgba_to_png_fast(&rgba, w as usize, h as usize);
                #[cfg(not(feature = "fast-png"))]
                let png: Option<Vec<u8>> = bail!("build with --features fast-png");
                let png = png.context("PNG encode")?;
                png_bytes += png.len();
                decode_png(&png)?
            }
            _ => {
                let png = encode_rgba_to_png(&rgba, w as usize, h as usize).context("PNG encode")?;
                png_bytes += png.len();
                decode_png(&png)?
            }
        };
        witness.update(&final_rgba);
    }
    let elapsed = start.elapsed();
    Ok(json!({
        "cmd": "texture",
        "mode": mode,
        "size": side,
        "count": count,
        "png_bytes": png_bytes,
        "elapsed_ms": elapsed.as_secs_f64() * 1e3,
        "ms_per_texture": elapsed.as_secs_f64() * 1e3 / count as f64,
        "blake3": witness.finalize().to_hex().to_string(),
    }))
}
