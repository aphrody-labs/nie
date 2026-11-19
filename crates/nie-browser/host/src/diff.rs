//! Pixel comparison of two PNGs.

use std::fs::File;

use anyhow::{Context, Result, bail};

/// A decoded image as RGBA8.
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

pub fn decode(path: &str) -> Result<Rgba> {
    let mut decoder = png::Decoder::new(std::io::BufReader::new(File::open(path).with_context(|| format!("open {path}"))?));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size().context("image too large")?];
    let info = reader.next_frame(&mut buf)?;
    buf.truncate(info.buffer_size());
    let data = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
        png::ColorType::Indexed => bail!("indexed PNG was not expanded"),
    };
    Ok(Rgba { width: info.width, height: info.height, data })
}

/// Result of comparing two images over their common area.
#[derive(Debug, PartialEq)]
pub struct Stats {
    pub width: u32,
    pub height: u32,
    pub differing: u64,
    pub total: u64,
    pub max_delta: u8,
    pub size_mismatch: bool,
}

/// Compare over the common top-left area; a pixel differs when any channel differs by more than `tolerance`.
pub fn compare(a: &Rgba, b: &Rgba, tolerance: u8) -> Stats {
    let width = a.width.min(b.width);
    let height = a.height.min(b.height);
    let (mut differing, mut max_delta) = (0u64, 0u8);
    for y in 0..height {
        for x in 0..width {
            let ia = ((y * a.width + x) * 4) as usize;
            let ib = ((y * b.width + x) * 4) as usize;
            let delta = (0..4).map(|c| a.data[ia + c].abs_diff(b.data[ib + c])).max().unwrap_or(0);
            max_delta = max_delta.max(delta);
            if delta > tolerance {
                differing += 1;
            }
        }
    }
    Stats {
        width,
        height,
        differing,
        total: u64::from(width) * u64::from(height),
        max_delta,
        size_mismatch: (a.width, a.height) != (b.width, b.height),
    }
}

pub fn run(args: &[String]) -> Result<()> {
    let [a, b, rest @ ..] = args else {
        bail!("usage: diff <a.png> <b.png> [--tolerance N]");
    };
    let tolerance: u8 = match rest {
        [flag, n] if flag == "--tolerance" => n.parse().context("tolerance")?,
        [] => 0,
        _ => bail!("usage: diff <a.png> <b.png> [--tolerance N]"),
    };
    let (ia, ib) = (decode(a)?, decode(b)?);
    let s = compare(&ia, &ib, tolerance);
    #[allow(clippy::cast_precision_loss)]
    let ratio = if s.total == 0 { 0.0 } else { s.differing as f64 / s.total as f64 };
    println!(
        "{{\"a\":[{},{}],\"b\":[{},{}],\"compared\":[{},{}],\"tolerance\":{tolerance},\"differing\":{},\"total\":{},\"ratio\":{ratio:.6},\"maxDelta\":{},\"sizeMismatch\":{}}}",
        ia.width, ia.height, ib.width, ib.height, s.width, s.height, s.differing, s.total, s.max_delta, s.size_mismatch
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, v: u8) -> Rgba {
        Rgba { width: w, height: h, data: vec![v; (w * h * 4) as usize] }
    }

    #[test]
    fn identical_images_do_not_differ() {
        let s = compare(&solid(4, 4, 9), &solid(4, 4, 9), 0);
        assert_eq!((s.differing, s.max_delta, s.size_mismatch), (0, 0, false));
    }

    #[test]
    fn tolerance_and_size_mismatch_are_reported() {
        let a = solid(4, 4, 10);
        let mut b = solid(5, 4, 10);
        b.data[0] = 14;
        let s = compare(&a, &b, 3);
        assert_eq!((s.differing, s.max_delta, s.size_mismatch, s.total), (1, 4, true, 16));
        assert_eq!(compare(&a, &b, 4).differing, 0);
    }

    #[test]
    fn decode_round_trips_an_rgba_png() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.png");
        let mut enc = png::Encoder::new(File::create(&path).unwrap(), 2, 1);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&[1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
        let img = decode(path.to_str().unwrap()).unwrap();
        assert_eq!((img.width, img.height, img.data), (2, 1, vec![1, 2, 3, 4, 5, 6, 7, 8]));
    }
}
