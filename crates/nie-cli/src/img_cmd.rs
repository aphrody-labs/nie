//! `nie img`: command-line image editing.
//!
//! Built on [`image`] (PNG, JPEG, WebP, GIF, BMP, TIFF codecs and geometric operations), already
//! a dependency of this crate for menu sprite pre-decoding. Resizing uses Lanczos3 by default.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use image::imageops::FilterType;
use image::{GenericImageView, ImageReader};
use nie_formats::imgmetric::{self, Roi, RoiKind, ScoreRegion};

/// Loads an image, guessing the format from the content, not the extension.
fn load(src: &Path) -> Result<image::DynamicImage> {
    ImageReader::open(src)
        .with_context(|| format!("opening {}", src.display()))?
        .with_guessed_format()
        .with_context(|| format!("unknown format: {}", src.display()))?
        .decode()
        .with_context(|| format!("decoding {}", src.display()))
}

/// Saves, picking the encoder from the extension of `dst`.
fn save(img: &image::DynamicImage, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    img.save(dst)
        .with_context(|| format!("writing {}", dst.display()))
}

/// Maps a filter name to a [`FilterType`].
fn filter_of(name: &str) -> Result<FilterType> {
    Ok(match name.to_ascii_lowercase().as_str() {
        "nearest" => FilterType::Nearest,
        "triangle" => FilterType::Triangle,
        "catmullrom" | "catmull" => FilterType::CatmullRom,
        "gaussian" => FilterType::Gaussian,
        "lanczos3" | "lanczos" => FilterType::Lanczos3,
        other => {
            bail!("unknown filter \"{other}\" (nearest|triangle|catmullrom|gaussian|lanczos3)")
        }
    })
}

/// `nie img recolor`: applies the [`nie_formats::recolor`] color filter to an image.
///
/// The image is forced to RGBA8: the filter works on four bytes per pixel, so a grayscale or RGB
/// source needs an alpha channel first. Alpha comes out untouched; the filter only changes the
/// first three channels.
pub fn recolor(src: &Path, dst: &Path, filtre: &nie_formats::recolor::Recolor) -> Result<()> {
    if filtre.is_identity() {
        bail!("the filter changes nothing: set --teinte, --saturation, --valeur or --rampe");
    }
    let img = load(src)?;
    let (w, h) = img.dimensions();
    let mut rgba = img.to_rgba8();
    filtre.apply_rgba(rgba.as_mut());
    save(&image::DynamicImage::ImageRgba8(rgba), dst)?;
    println!("recolored  {} -> {} ({w}x{h})", src.display(), dst.display());
    Ok(())
}

/// `nie img info`: dimensions, format and color; writes nothing.
pub fn info(src: &Path) -> Result<()> {
    let reader = ImageReader::open(src)
        .with_context(|| format!("opening {}", src.display()))?
        .with_guessed_format()
        .with_context(|| format!("unknown format: {}", src.display()))?;
    let format = reader.format();
    let img = reader
        .decode()
        .with_context(|| format!("decoding {}", src.display()))?;
    let (w, h) = img.dimensions();
    let taille = std::fs::metadata(src).map(|m| m.len()).unwrap_or(0);
    println!("  file      {}", src.display());
    println!(
        "  format    {}",
        format.map_or("unknown", |f| f.extensions_str()[0])
    );
    println!("  dimensions {w}x{h}");
    println!("  color     {:?}", img.color());
    println!("  bytes     {taille}");
    Ok(())
}

/// `nie img resize`: resizes, preserving the ratio if only one dimension is given.
pub fn resize(
    src: &Path,
    dst: &Path,
    width: Option<u32>,
    height: Option<u32>,
    filter: &str,
    exact: bool,
) -> Result<()> {
    let img = load(src)?;
    let (w0, h0) = img.dimensions();
    let (tw, th) = match (width, height) {
        (Some(w), Some(h)) => (w, h),
        // One dimension only: the other follows the ratio, rounded to nearest.
        (Some(w), None) => (
            w,
            ((u64::from(h0) * u64::from(w)) as f64 / f64::from(w0)).round() as u32,
        ),
        (None, Some(h)) => (
            ((u64::from(w0) * u64::from(h)) as f64 / f64::from(h0)).round() as u32,
            h,
        ),
        (None, None) => bail!("set at least --width or --height"),
    };
    if tw == 0 || th == 0 {
        bail!("target dimensions are zero ({tw}x{th})");
    }
    let f = filter_of(filter)?;
    // `resize_exact` distorts; `resize` fits inside the box keeping the ratio.
    let out = if exact {
        img.resize_exact(tw, th, f)
    } else {
        img.resize(tw, th, f)
    };
    let (fw, fh) = out.dimensions();
    save(&out, dst)?;
    println!(
        "resize {w0}x{h0} -> {fw}x{fh} ({filter}) -> {}",
        dst.display()
    );
    Ok(())
}

/// `nie img crop`: crops a region, refusing to go outside the image.
pub fn crop(src: &Path, dst: &Path, x: u32, y: u32, w: u32, h: u32) -> Result<()> {
    let img = load(src)?;
    let (iw, ih) = img.dimensions();
    if x.saturating_add(w) > iw || y.saturating_add(h) > ih {
        bail!("region {x},{y} {w}x{h} is outside image {iw}x{ih}");
    }
    let out = img.crop_imm(x, y, w, h);
    save(&out, dst)?;
    println!("crop {iw}x{ih} @({x},{y}) {w}x{h} -> {}", dst.display());
    Ok(())
}

/// `nie img convert`: re-encodes to the format implied by the output extension.
pub fn convert(src: &Path, dst: &Path) -> Result<()> {
    let img = load(src)?;
    let (w, h) = img.dimensions();
    save(&img, dst)?;
    println!("convert {w}x{h} {} -> {}", src.display(), dst.display());
    Ok(())
}

/// `nie img composite`: overlays `overlay` on `base` at the given position (alpha respected).
///
/// This recomposes a game visual split into layers: a background and its localized text layer
/// live in two separate `.g4tx` files.
pub fn composite(base: &Path, overlay: &Path, dst: &Path, x: i64, y: i64) -> Result<()> {
    let mut bottom = load(base)?.to_rgba8();
    let top = load(overlay)?.to_rgba8();
    image::imageops::overlay(&mut bottom, &top, x, y);
    let out = image::DynamicImage::ImageRgba8(bottom);
    save(&out, dst)?;
    println!(
        "composite {} + {} @({x},{y}) -> {}",
        base.display(),
        overlay.display(),
        dst.display()
    );
    Ok(())
}

/// Assembles several images into a sprite sheet and writes its manifest.
///
/// The manifest is what makes a sheet reusable: without the rectangles the output can only be
/// looked at. For each cell it holds the name (source file name without extension) and the
/// rectangle of the image itself, never that of the cell.
///
/// No image is resized: smaller ones are centered in their cell.
pub fn planche(
    srcs: &[PathBuf],
    dst: &Path,
    manifeste: Option<&Path>,
    colonnes: u32,
    marge: u32,
    gouttiere: u32,
    fond: [u8; 4],
) -> Result<()> {
    if srcs.is_empty() {
        anyhow::bail!("no source image");
    }

    let mut cases = Vec::with_capacity(srcs.len());
    for src in srcs {
        let img = load(src)?.to_rgba8();
        let (w, h) = img.dimensions();
        cases.push(nie_formats::image_out::CasePlanche {
            // The file name is the cell name: it is what the caller will recognize in the
            // manifest, since they already chose it when naming their files.
            nom: src.file_stem().map_or_else(
                || "sans-nom".to_string(),
                |s| s.to_string_lossy().into_owned(),
            ),
            largeur: w,
            hauteur: h,
            rgba: img.into_raw(),
        });
    }

    let colonnes = if colonnes == 0 {
        cases.len() as u32
    } else {
        colonnes
    };
    let p = nie_formats::image_out::composer_planche(&cases, colonnes, marge, gouttiere, fond);

    let img = image::RgbaImage::from_raw(p.largeur, p.hauteur, p.rgba)
        .context("inconsistent composed sheet (dimensions vs buffer)")?;
    save(&image::DynamicImage::ImageRgba8(img), dst)?;

    if let Some(chemin) = manifeste {
        let cases_json: Vec<serde_json::Value> = p
            .cases
            .iter()
            .zip(srcs)
            .map(|(r, src)| {
                serde_json::json!({
                    "nom": r.nom,
                    "source": src.to_string_lossy(),
                    "rect": { "x": r.x, "y": r.y, "w": r.w, "h": r.h },
                })
            })
            .collect();
        let doc = serde_json::json!({
            "planche": dst.file_name().map(|n| n.to_string_lossy().into_owned()),
            "largeur": p.largeur,
            "hauteur": p.hauteur,
            "colonnes": colonnes,
            "marge": marge,
            "gouttiere": gouttiere,
            "redimensionnement": "aucun",
            "cases": cases_json,
        });
        if let Some(parent) = chemin.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        std::fs::write(chemin, serde_json::to_vec_pretty(&doc)?)
            .with_context(|| format!("writing {}", chemin.display()))?;
    }

    println!(
        "sheet {}x{}: {} cell(s), {} column(s) -> {}{}",
        p.largeur,
        p.hauteur,
        p.cases.len(),
        colonnes,
        dst.display(),
        manifeste.map_or_else(String::new, |m| format!(" + {}", m.display()))
    );
    Ok(())
}

/// Parses a hexadecimal `RRGGBB` or `RRGGBBAA` color.
///
/// Without alpha the color is opaque: a six-digit background is one the caller wants to see, not
/// a transparent one.
pub fn couleur_hex(s: &str) -> Result<[u8; 4]> {
    let t = s.trim_start_matches('#');
    let lire = |i: usize| -> Result<u8> {
        u8::from_str_radix(&t[i..i + 2], 16).with_context(|| format!("invalid color: {s}"))
    };
    match t.len() {
        6 => Ok([lire(0)?, lire(2)?, lire(4)?, 255]),
        8 => Ok([lire(0)?, lire(2)?, lire(4)?, lire(6)?]),
        _ => anyhow::bail!("expected RRGGBB or RRGGBBAA color, got: {s}"),
    }
}

/// `nie img` subcommands, resolved from `main`.
pub enum Op {
    Info {
        src: PathBuf,
    },
    Planche {
        srcs: Vec<PathBuf>,
        out: PathBuf,
        manifeste: Option<PathBuf>,
        colonnes: u32,
        marge: u32,
        gouttiere: u32,
        fond: String,
    },
    Resize {
        src: PathBuf,
        out: PathBuf,
        width: Option<u32>,
        height: Option<u32>,
        filter: String,
        exact: bool,
    },
    Recolor {
        src: PathBuf,
        out: PathBuf,
        filtre: nie_formats::recolor::Recolor,
    },
    Crop {
        src: PathBuf,
        out: PathBuf,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    },
    Convert {
        src: PathBuf,
        out: PathBuf,
    },
    Composite {
        base: PathBuf,
        overlay: PathBuf,
        out: PathBuf,
        x: i64,
        y: i64,
    },
    Diff {
        rendu: PathBuf,
        reference: PathBuf,
        roi: Option<PathBuf>,
        out: Option<PathBuf>,
        downscale_ref: bool,
        amplification: u8,
    },
}

/// Runs an image operation.
pub fn run(op: &Op) -> Result<()> {
    match op {
        Op::Info { src } => info(src),
        Op::Recolor { src, out, filtre } => recolor(src, out, filtre),
        Op::Resize {
            src,
            out,
            width,
            height,
            filter,
            exact,
        } => resize(src, out, *width, *height, filter, *exact),
        Op::Crop {
            src,
            out,
            x,
            y,
            w,
            h,
        } => crop(src, out, *x, *y, *w, *h),
        Op::Convert { src, out } => convert(src, out),
        Op::Composite {
            base,
            overlay,
            out,
            x,
            y,
        } => composite(base, overlay, out, *x, *y),
        Op::Planche {
            srcs,
            out,
            manifeste,
            colonnes,
            marge,
            gouttiere,
            fond,
        } => planche(
            srcs,
            out,
            manifeste.as_deref(),
            *colonnes,
            *marge,
            *gouttiere,
            couleur_hex(fond)?,
        ),
        Op::Diff {
            rendu,
            reference,
            roi,
            out,
            downscale_ref,
            amplification,
        } => diff(
            rendu,
            reference,
            roi.as_deref(),
            out.as_deref(),
            *downscale_ref,
            *amplification,
        ),
    }
}

/// Reads a regions file.
///
/// Shape: `[{"nom": "avatar3d", "rect": [x, y, w, h], "kind": "dynamique"}]`. `kind` is
/// `dynamique` (excluded from the measurement) or `nommee` (measured separately, still counted
/// in the global score).
fn charger_rois(chemin: &Path) -> Result<Vec<Roi>> {
    #[derive(serde::Deserialize)]
    struct Entree {
        nom: String,
        rect: [u32; 4],
        #[serde(default)]
        kind: Option<String>,
    }
    let txt =
        std::fs::read_to_string(chemin).with_context(|| format!("reading {}", chemin.display()))?;
    let brut: Vec<Entree> =
        serde_json::from_str(&txt).with_context(|| format!("parsing {}", chemin.display()))?;
    brut.into_iter()
        .map(|e| {
            let kind = match e.kind.as_deref().unwrap_or("nommee") {
                "dynamique" => RoiKind::Dynamique,
                "nommee" | "nommée" => RoiKind::Nommee,
                autre => bail!(
                    "unknown kind \"{autre}\" (dynamique|nommee) for \"{}\"",
                    e.nom
                ),
            };
            Ok(Roi {
                nom: e.nom,
                rect: (e.rect[0], e.rect[1], e.rect[2], e.rect[3]),
                kind,
            })
        })
        .collect()
}

/// One table row for a score.
fn ligne_score(s: &ScoreRegion) -> String {
    format!(
        "  {:<22} {:>10} px   T0 {:>6.2} %   ΔE≤1 {:>6.2} %   ΔE mean {:>6.2}   p99 {:>6.2}   SSIM {:>6.4}",
        s.nom, s.px, s.exact_pct, s.de1_pct, s.de_moyen, s.de_p99, s.ssim
    )
}

/// `nie img diff`: compares a render to a capture of the real game.
///
/// Output is per region: a global score always mixes a correct area with a wrong one without
/// saying which. Regions marked `dynamique` (3D character, particles, cursor) are excluded from
/// the measurement, and the surface share removed this way is printed.
pub fn diff(
    rendu: &Path,
    reference: &Path,
    roi: Option<&Path>,
    out: Option<&Path>,
    downscale_ref: bool,
    amplification: u8,
) -> Result<()> {
    let a = load(rendu)?.to_rgba8();
    let b = load(reference)?.to_rgba8();
    let (aw, ah) = (a.width(), a.height());
    let (mut bw, mut bh) = (b.width(), b.height());
    let mut bpix = b.into_raw();

    if downscale_ref {
        let (nw, nh, px) = imgmetric::downscale_lineaire_2x(bw, bh, &bpix);
        bw = nw;
        bh = nh;
        bpix = px;
    }
    if (aw, ah) != (bw, bh) {
        let piste = if (bw, bh) == (aw * 2, ah * 2) {
            "; the reference is exactly double size: `--downscale-ref` reduces it in linear \
             light, but rendering natively at its size measures better"
        } else {
            ""
        };
        bail!("incompatible dimensions: render {aw}x{ah}, reference {bw}x{bh}{piste}");
    }

    let rois = match roi {
        Some(p) => charger_rois(p)?,
        None => Vec::new(),
    };
    let rapport = imgmetric::comparer(aw, ah, &a, &bpix, &rois);

    println!(
        "{} vs {}  ({aw}×{ah})",
        rendu.display(),
        reference.display()
    );
    println!("{}", ligne_score(&rapport.global));
    for r in &rapport.regions {
        println!("{}", ligne_score(r));
    }
    println!(
        "  excluded surface (dynamic regions): {:.2} %   |   render opaque on {:.2} % of measured pixels",
        rapport.surface_exclue_pct, rapport.couverture_opaque_pct
    );
    if rapport.couverture_opaque_pct < 99.99 {
        println!(
            "  warning: the render shows the canvas; part of the score covers empty space, not pixels."
        );
    }

    let Some(dir) = out else { return Ok(()) };
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;

    let json = serde_json::json!({
        "rendu": rendu.display().to_string(),
        "reference": reference.display().to_string(),
        "largeur": aw,
        "hauteur": ah,
        "surfaceExcluePct": rapport.surface_exclue_pct,
        "couvertureOpaquePct": rapport.couverture_opaque_pct,
        "global": json_score(&rapport.global),
        "regions": rapport.regions.iter().map(json_score).collect::<Vec<_>>(),
    });
    std::fs::write(dir.join("rapport.json"), serde_json::to_vec_pretty(&json)?)?;

    let (hw, hh, heat) = imgmetric::heatmap_rgba(&rapport);
    ecrire_png(&dir.join("heatmap.png"), hw, hh, &heat)?;
    let delta = imgmetric::delta_rgba(aw, ah, &a, &bpix, &rois, amplification);
    ecrire_png(&dir.join("delta.png"), aw, ah, &delta)?;
    println!(
        "  written: {}/rapport.json | heatmap.png | delta.png",
        dir.display()
    );
    Ok(())
}

/// Serializes a score.
fn json_score(s: &ScoreRegion) -> serde_json::Value {
    serde_json::json!({
        "nom": s.nom,
        "px": s.px,
        "exactPct": s.exact_pct,
        "de1Pct": s.de1_pct,
        "canal2Pct": s.canal2_pct,
        "deMoyen": s.de_moyen,
        "deP99": s.de_p99,
        "deMax": s.de_max,
        "ssim": s.ssim,
    })
}

/// Writes an RGBA8 buffer as PNG.
fn ecrire_png(dst: &Path, w: u32, h: u32, rgba: &[u8]) -> Result<()> {
    let buf: image::RgbaImage = image::ImageBuffer::from_raw(w, h, rgba.to_vec())
        .ok_or_else(|| anyhow::anyhow!("RGBA buffer has the wrong size for {w}x{h}"))?;
    image::DynamicImage::ImageRgba8(buf)
        .save(dst)
        .with_context(|| format!("writing {}", dst.display()))
}
