//! The offscreen path of `nie term --screenshot`: the scene drawn on the CPU with sugarloaf's own
//! text rasteriser (the same shaping and glyph atlas code as the window), then written as PNG.
//!
//! sugarloaf's GPU contexts are bound to a window surface, so a headless host has no GPU frame to
//! read back; this canvas keeps the evidence independent of the display server and of the
//! driver, and is the reference the GPU output is checked against.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use nie_explore::term::Image;
use sugarloaf::font::FontLibrary;
use sugarloaf::text::{DrawOpts, Text};

use super::scene::Canvas;

/// An RGB buffer plus the text recorder drawn over it.
pub struct CpuCanvas {
    width: u32,
    height: u32,
    pixels: Vec<u32>,
    text: Text,
    font_size: f32,
    /// Vertical offset of the glyph origin inside a cell, in pixels.
    text_dy: f32,
}

impl CpuCanvas {
    /// A canvas of `width` x `height` pixels cleared to `bg`.
    pub fn new(width: u32, height: u32, bg: [u8; 3], fonts: &FontLibrary, font_size: f32, text_dy: f32) -> Self {
        let mut text = Text::new(fonts);
        text.init_cpu();
        text.set_scale_factor(1.0);
        let clear = pack(bg);
        Self {
            width,
            height,
            pixels: vec![clear; width as usize * height as usize],
            text,
            font_size,
            text_dy,
        }
    }

    /// Paints the recorded text and returns the finished RGB bytes.
    pub fn finish(mut self) -> (u32, u32, Vec<u8>) {
        self.text.render_cpu(&mut self.pixels, self.width, self.height);
        let mut rgb = Vec::with_capacity(self.pixels.len() * 3);
        for p in &self.pixels {
            rgb.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, *p as u8]);
        }
        (self.width, self.height, rgb)
    }
}

fn pack(c: [u8; 3]) -> u32 {
    u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2])
}

fn blend(dst: u32, src: [u8; 3], alpha: f32) -> u32 {
    let mix = |d: u32, s: u8| (d as f32 + (f32::from(s) - d as f32) * alpha).round().clamp(0.0, 255.0) as u32;
    mix(dst >> 16 & 0xff, src[0]) << 16 | mix(dst >> 8 & 0xff, src[1]) << 8 | mix(dst & 0xff, src[2])
}

impl Canvas for CpuCanvas {
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: [f32; 4]) {
        let (x0, y0) = (x.round().max(0.0) as u32, y.round().max(0.0) as u32);
        let (x1, y1) = (((x + w).round().max(0.0) as u32).min(self.width), ((y + h).round().max(0.0) as u32).min(self.height));
        let rgb = [(c[0] * 255.0).round() as u8, (c[1] * 255.0).round() as u8, (c[2] * 255.0).round() as u8];
        for py in y0..y1 {
            let row = py as usize * self.width as usize;
            for px in x0..x1 {
                let i = row + px as usize;
                self.pixels[i] = if c[3] >= 1.0 { pack(rgb) } else { blend(self.pixels[i], rgb, c[3]) };
            }
        }
    }

    fn text(&mut self, x: f32, y: f32, text: &str, color: [u8; 4], bold: bool, italic: bool) {
        let opts = DrawOpts {
            font_size: self.font_size,
            color,
            bold,
            italic,
            font_id: None,
        };
        self.text.draw(x, y + self.text_dy, text, &opts);
    }

    fn image(&mut self, image: &Arc<Image>, rect: [f32; 4], uv: [f32; 4], _z_index: i32) {
        let (x0, y0) = (rect[0].round() as i64, rect[1].round() as i64);
        let (w, h) = (rect[2].round() as i64, rect[3].round() as i64);
        if w <= 0 || h <= 0 || image.width == 0 || image.height == 0 {
            return;
        }
        let (iw, ih) = (image.width as f32, image.height as f32);
        for dy in 0..h {
            let py = y0 + dy;
            if py < 0 || py >= i64::from(self.height) {
                continue;
            }
            let v = uv[1] + (uv[3] - uv[1]) * ((dy as f32 + 0.5) / h as f32);
            let sy = ((v * ih) as i64).clamp(0, i64::from(image.height) - 1) as usize;
            for dx in 0..w {
                let px = x0 + dx;
                if px < 0 || px >= i64::from(self.width) {
                    continue;
                }
                let u = uv[0] + (uv[2] - uv[0]) * ((dx as f32 + 0.5) / w as f32);
                let sx = ((u * iw) as i64).clamp(0, i64::from(image.width) - 1) as usize;
                let s = (sy * image.width as usize + sx) * 4;
                let px_rgba = &image.rgba[s..s + 4];
                let i = py as usize * self.width as usize + px as usize;
                self.pixels[i] = blend(self.pixels[i], [px_rgba[0], px_rgba[1], px_rgba[2]], f32::from(px_rgba[3]) / 255.0);
            }
        }
    }
}

/// Writes RGB bytes as a PNG file.
pub fn write_png(path: &Path, width: u32, height: u32, rgb: &[u8]) -> Result<()> {
    let file = std::fs::File::create(path).with_context(|| format!("create {}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgb)?;
    writer.finish()?;
    Ok(())
}
