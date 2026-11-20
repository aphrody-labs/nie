//! `nie-browser-host`: native host of the kitesurf engine.
//!
//! * `shot <file.html> <out.png> [width height]` renders a page through blitz, stylo, taffy, parley
//!   and vello_cpu, with the viewport clipped to `width` x `height` (default 1280 x 720).
//! * `diff <a.png> <b.png> [--tolerance N]` compares two PNGs pixel by pixel and prints JSON.

use std::{fs, path::Path};

use anyhow::{Context, Result, anyhow, bail};

mod diff;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("shot") => shot(&args[1..]),
        Some("diff") => diff::run(&args[1..]),
        _ => bail!("usage: nie-browser-host shot <file.html> <out.png> [width height] | diff <a.png> <b.png> [--tolerance N]"),
    }
}

fn shot(args: &[String]) -> Result<()> {
    let [html_path, out, rest @ ..] = args else {
        bail!("usage: shot <file.html> <out.png> [width height]");
    };
    let width: u32 = rest.first().map_or(Ok(1280), |s| s.parse()).context("width")?;
    let height: u32 = rest.get(1).map_or(Ok(720), |s| s.parse()).context("height")?;
    let html = fs::read_to_string(html_path).with_context(|| format!("read {html_path}"))?;
    let base = Path::new(html_path)
        .canonicalize()
        .ok()
        .and_then(|p| url_of(&p))
        .unwrap_or_default();
    let png = kitesurf_core::render_html_to_image(&html, &base, width, height, "png", 90)
        .map_err(|e| anyhow!("render {html_path}: {e}"))?;
    fs::write(out, &png).with_context(|| format!("write {out}"))?;
    println!("{{\"out\":\"{out}\",\"width\":{width},\"height\":{height},\"bytes\":{}}}", png.len());
    Ok(())
}

fn url_of(path: &Path) -> Option<String> {
    let s = path.to_str()?.replace('\\', "/");
    let s = s.trim_start_matches("//?/");
    Some(if s.starts_with('/') { format!("file://{s}") } else { format!("file:///{s}") })
}
