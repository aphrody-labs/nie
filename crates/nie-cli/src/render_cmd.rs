//! Visual check captures for GLB files assembled by NIE.
//!
//! This facade lets an agent or a human request a stable view or a full rotation without
//! knowing the internal render binary or assembling ffmpeg.

use std::fs;
use std::fs::File;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, Frame, ImageBuffer, Rgba};
use nie_render3d::{glb, render};

/// Maximum side of a capture: prevents an accidental oversized render.
const MAX_SIDE: u32 = 4096;
/// Maximum image area, even when each dimension stays below [`MAX_SIDE`].
const MAX_PIXELS: u64 = 16 * 1024 * 1024;
/// GIF guard: the output is meant to inspect a turn, not to encode a video.
const MAX_GIF_FRAMES: u32 = 240;

/// Parameters shared by a still capture and a turntable.
pub(crate) struct RenderConfig {
    pub(crate) width: u32,
    pub(crate) height: u32,
}

fn verifier_dimensions(width: u32, height: u32) -> Result<()> {
    ensure!(
        width > 0 && height > 0,
        "output must have a positive width and height"
    );
    ensure!(
        width <= MAX_SIDE
            && height <= MAX_SIDE
            && u64::from(width) * u64::from(height) <= MAX_PIXELS,
        "dimensions {width}x{height} out of range (maximum {MAX_SIDE}x{MAX_SIDE} and {MAX_PIXELS} pixels)"
    );
    Ok(())
}

fn charger(glb_path: &Path) -> Result<glb::Model> {
    let bytes =
        fs::read(glb_path).with_context(|| format!("reading GLB {}", glb_path.display()))?;
    glb::parse(&bytes).with_context(|| format!("invalid GLB {}", glb_path.display()))
}

/// CPU-only capture renderer. GPU and WebGPU backends stay outside this workspace.
fn encoder_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(std::io::Cursor::new(&mut out), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().context("PNG header")?;
        writer.write_image_data(rgba).context("PNG pixels")?;
    }
    Ok(out)
}

/// Renders a GLB from a reproducible view to a lossless PNG.
pub(crate) fn glb_png(glb_path: &Path, out: &Path, config: RenderConfig, angle: f32) -> Result<()> {
    verifier_dimensions(config.width, config.height)?;
    ensure!(angle.is_finite(), "angle must be a finite number");
    let model = charger(glb_path)?;
    let rgba = render::render(&model, angle, config.width, config.height);
    let png = encoder_png(&rgba, config.width, config.height)?;
    fs::write(out, png).with_context(|| format!("writing PNG {}", out.display()))?;
    println!(
        "png={} source={} dimensions={}x{} angle={angle:.6} renderer={}",
        out.display(),
        glb_path.display(),
        config.width,
        config.height,
        "cpu"
    );
    Ok(())
}

/// Renders a full turn as an animated GIF, with evenly spaced angles.
pub(crate) fn glb_gif(
    glb_path: &Path,
    out: &Path,
    config: RenderConfig,
    frames: u32,
    fps: u32,
) -> Result<()> {
    verifier_dimensions(config.width, config.height)?;
    ensure!(frames >= 2, "a turntable GIF needs at least 2 frames");
    ensure!(
        frames <= MAX_GIF_FRAMES,
        "a GIF is limited to {MAX_GIF_FRAMES} frames"
    );
    ensure!(
        fps > 0 && fps <= 120,
        "GIF frame rate must be between 1 and 120 fps"
    );

    let model = charger(glb_path)?;

    let file = File::create(out).with_context(|| format!("writing GIF {}", out.display()))?;
    let mut encoder = GifEncoder::new(file);
    encoder.set_repeat(Repeat::Infinite).context("GIF loop")?;
    let delay = Delay::from_numer_denom_ms(1000, fps);
    for index in 0..frames {
        let angle = std::f32::consts::TAU * index as f32 / frames as f32;
        let rgba = render::render(&model, angle, config.width, config.height);
        let buffer = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(config.width, config.height, rgba)
            .context("inconsistent GIF frame size")?;
        encoder
            .encode_frame(Frame::from_parts(buffer, 0, 0, delay))
            .context("frame GIF")?;
    }
    println!(
        "gif={} source={} dimensions={}x{} frames={frames} fps={fps} renderer={}",
        out.display(),
        glb_path.display(),
        config.width,
        config.height,
        "cpu"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions_reject_an_unbounded_capture() {
        assert!(verifier_dimensions(1024, 1024).is_ok());
        assert!(verifier_dimensions(0, 720).is_err());
        assert!(verifier_dimensions(4097, 4096).is_err());
    }

    #[test]
    fn gif_encoder_writes_an_animated_gif_header() {
        let mut out = Vec::new();
        {
            let mut encoder = GifEncoder::new(&mut out);
            encoder.set_repeat(Repeat::Infinite).expect("GIF loop");
            let buffer = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(2, 2, vec![255; 16])
                .expect("frame RGBA");
            encoder
                .encode_frame(Frame::from_parts(
                    buffer,
                    0,
                    0,
                    Delay::from_numer_denom_ms(1000, 12),
                ))
                .expect("frame GIF");
        }
        assert!(out.starts_with(b"GIF89a"));
    }
}
