//! `nie video` - inventory, measure, remux and export the game cutscenes.
//!
//! The films are **USM / Sofdec2** containers: H.264 and CRI HCA interleaved in blocks, which no
//! player opens directly. This command is a text facade: the film record lives in
//! [`nie_explore::cinema`], shared with `nie-model-serve` and the explorer.
//!
//! It sits on three pure-Rust pieces:
//!
//! * [`nie_formats::usm`]: demux, `@UTF` metadata, one access unit per frame;
//! * [`nie_formats::mp4`]: MP4 remux of H.264 streams (bytes kept, Annex-B start codes become
//!   AVCC length prefixes);
//! * [`nie_formats::webm`]: WebM mux of VP9 streams, after stripping their IVF wrapper.
//!
//! The remux is lossless and smaller: only the USM interleaving is dropped (block headers,
//! alignment padding, and the audio track when exported separately). `info` and `export` report
//! the saving.
//!
//! The corpus mixes H.264, MPEG-2 and VP9 (see `nie video catalogue`). MPEG-2 (title screens and
//! logos) has no web container and is written as a raw `.m2v` elementary stream that VLC and mpv
//! play.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use nie_explore::cinema::{self, Film};
use nie_formats::usm::{self, Usm};
use nie_formats::vfs::Vfs;

/// `nie video` subcommands.
#[derive(clap::Subcommand, Debug)]
pub enum VideoCmd {
    /// Film metadata plus MP4 remux measurement (size, frame rate, duration, saving).
    Info {
        /// VFS path (`data/common/movie/ev01_00050.usm`) or short name (`ev01_00050`).
        chemin: String,
        /// JSON output.
        #[arg(long)]
        json: bool,
        /// Also dump the container header `@UTF` tables (format diagnostics).
        #[arg(long)]
        tables: bool,
    },

    /// List films with their real metadata.
    Liste {
        /// VFS prefix to scan.
        #[arg(long, default_value = cinema::DOSSIER_FILMS)]
        prefixe: String,
        /// JSON output.
        #[arg(long)]
        json: bool,
        /// Maximum number of films processed.
        #[arg(long)]
        limit: Option<usize>,
        /// Skip the remux measurement: duration, size and audio are still read, in constant memory.
        #[arg(long)]
        rapide: bool,
    },

    /// Export a film as MP4 (video track) and, with `--audio`, its soundtrack as WAV.
    Export {
        /// VFS path or short name of the film.
        chemin: String,
        /// Output `.mp4` file, or a folder (the film name is then used).
        #[arg(long)]
        out: PathBuf,
        /// Also write `<output>.wav`, the track decoded from HCA.
        #[arg(long)]
        audio: bool,
        /// Write the raw elementary video stream without a container (format diagnostics, or playback
        /// in VLC/mpv when no browser decodes the codec).
        #[arg(long)]
        brut: bool,
    },

    /// Full catalogue for the UI: categories, languages, durations, gamedata join.
    Catalogue {
        /// Output JSON file (default: standard output).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Skip the remux measurement: constant-memory catalogue, the one the CDN serves.
        #[arg(long)]
        rapide: bool,
    },

    /// Convert a WebM / VP9 video (e.g. a CloudFront super move) to a USM or IVF container.
    ConvertWebm {
        /// Source `.webm` file.
        source: PathBuf,
        /// Output file (`.usm` or `.ivf`).
        #[arg(long)]
        out: PathBuf,
        /// Logical film name in the USM metadata (optional).
        #[arg(long)]
        nom: Option<String>,
        /// Forced frame rate in frames per second (numerator). Default: 60.
        #[arg(long, default_value_t = 60)]
        fps: u32,
    },
}

/// Command entry point.
///
/// # Errors
///
/// Propagates VFS read, demux and file write failures.
pub fn run(op: &VideoCmd, vfs: &Vfs) -> Result<()> {
    match op {
        VideoCmd::Info {
            chemin,
            json,
            tables,
        } => info(vfs, chemin, *json, *tables),
        VideoCmd::Liste {
            prefixe,
            json,
            limit,
            rapide,
        } => liste(vfs, prefixe, *json, *limit, *rapide),
        VideoCmd::Export {
            chemin,
            out,
            audio,
            brut,
        } => export(vfs, chemin, out, *audio, *brut),
        VideoCmd::Catalogue { out, rapide } => catalogue(vfs, out.as_deref(), *rapide),
        VideoCmd::ConvertWebm {
            source,
            out,
            nom,
            fps,
        } => convert_webm(source, out, nom.as_deref(), *fps),
    }
}

/// Read and demux a film. Also returns the raw container size.
fn charger(vfs: &Vfs, chemin: &str) -> Result<(Usm, u64)> {
    let brut = vfs
        .read(chemin)
        .with_context(|| format!("VFS read {chemin}"))?;
    let taille = brut.len() as u64;
    let u = usm::demuxer_nomme(&brut, usm::nom_fichier_de(chemin))
        .with_context(|| format!("demux {chemin}"))?;
    Ok((u, taille))
}

// -- Operations --

/// Dump the container header `@UTF` tables, column by column.
///
/// This is the only way to tell "this film does not demux" from "this film is encrypted": the
/// headers stay in clear even when the payload does not, and they declare the real size, frame
/// rate and codec.
fn vider_tables(vfs: &Vfs, chemin: &str) -> Result<()> {
    let (u, _) = charger(vfs, chemin)?;
    for t in &u.entetes {
        println!("-- {} ({} row(s)) --", t.name, t.row_count());
        for (i, ligne) in t.rows.iter().enumerate() {
            for (col, val) in t.columns.iter().zip(ligne) {
                let rendu = match val {
                    nie_formats::cpk::UtfValue::String(s) => s.clone(),
                    nie_formats::cpk::UtfValue::Bytes(b) => format!("<{} bytes>", b.len()),
                    autre => format!("{autre:?}"),
                };
                println!("  [{i}] {:<22} {rendu}", col.name);
            }
        }
    }
    println!();
    Ok(())
}

/// Print a film record as text, as read in a terminal.
fn afficher(f: &Film) {
    println!("{}", f.chemin);
    if let Some(e) = &f.erreur {
        println!("  error       {e}");
        return;
    }
    println!("  category    {}", f.rubrique);
    if let Some(l) = &f.langue {
        println!("  language    {l}");
    }
    if let Some(o) = &f.nom_origine {
        println!("  encoded name {o}");
    }
    if f.dechiffre {
        println!("  encryption  CRI envelope (key derived from the file name)");
    }
    println!(
        "  video       {}x{} {} - {} frames, {:.3} fps, {:.2} s",
        f.largeur,
        f.hauteur,
        f.codec,
        f.images,
        f.cadence.unwrap_or(0.0),
        f.duree.unwrap_or(0.0)
    );
    if let Some(c) = f.cles {
        println!("  keyframes   {c}");
    }
    for p in &f.audio {
        println!(
            "  audio {}     {} - {} Hz, {} channel(s), {} B",
            p.canal, p.codec, p.frequence, p.canaux, p.octets
        );
    }
    match &f.bande_son {
        Some(b) => println!(
            "  soundtrack  anime_stream / cue {} - {} {} Hz, {} channel(s), {:.2} s{}",
            b.cue,
            b.codec,
            b.frequence,
            b.canaux,
            f64::from(b.duree_ms) / 1000.0,
            if b.confirme_par_hash {
                " (confirmed by bgmName)"
            } else {
                ""
            },
        ),
        None if f.audio.is_empty() => {
            println!("  soundtrack  none (neither container nor anime_stream)");
        }
        None => {}
    }
    if let Some(m) = f.conteneur_octets {
        println!(
            "  remux       {} - {} B from {} B, {:.2} % smaller, no re-encode",
            f.conteneur.as_deref().unwrap_or("?"),
            m,
            f.octets,
            f.gain_remux.unwrap_or(0.0)
        );
    } else if let Some(e) = &f.remux_impossible {
        println!("  remux       impossible: {e}");
    }
    if let Some(g) = &f.gamedata {
        println!("  gamedata    {}", g.source);
        if let Some(s) = &g.subtitle_text_path {
            println!("  subtitles   {s}");
        }
    }
}

fn info(vfs: &Vfs, entree: &str, en_json: bool, tables: bool) -> Result<()> {
    let chemin = cinema::resoudre(vfs, entree);
    if tables {
        vider_tables(vfs, &chemin)?;
    }
    let jointure = cinema::jointure_gamedata(vfs);
    let f = cinema::complet(vfs, &chemin, Some(&jointure));
    if en_json {
        println!("{}", serde_json::to_string_pretty(&f)?);
        return Ok(());
    }
    afficher(&f);
    Ok(())
}

fn liste(
    vfs: &Vfs,
    prefixe: &str,
    en_json: bool,
    limit: Option<usize>,
    rapide: bool,
) -> Result<()> {
    let mut chemins = cinema::chemins_films(vfs, prefixe);
    if let Some(n) = limit {
        chemins.truncate(n);
    }

    let jointure = cinema::jointure_gamedata(vfs);
    let fiches: Vec<Film> = chemins
        .iter()
        .map(|c| {
            if rapide {
                cinema::apercu(vfs, c, Some(&jointure))
            } else {
                cinema::complet(vfs, c, Some(&jointure))
            }
        })
        .collect();

    if en_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "films": fiches }))?
        );
        return Ok(());
    }

    println!(
        "{:<24} {:>10} {:>11} {:>8} {:>9}  {:<5} category",
        "film", "bytes", "size", "duration", "rate", "audio"
    );
    for f in &fiches {
        let definition = if f.largeur > 0 {
            format!("{}x{}", f.largeur, f.hauteur)
        } else {
            "-".to_string()
        };
        println!(
            "{:<24} {:>10} {:>11} {:>7.2}s {:>8.3}  {:<5} {}",
            f.nom,
            f.octets,
            definition,
            f.duree.unwrap_or(0.0),
            f.cadence.unwrap_or(0.0),
            if f.a_du_son() { "yes" } else { "-" },
            f.rubrique,
        );
    }
    let sonores = fiches.iter().filter(|f| f.a_du_son()).count();
    println!("\n{} film(s), {sonores} with soundtrack", fiches.len());
    Ok(())
}

fn export(vfs: &Vfs, entree: &str, out: &Path, avec_audio: bool, brut: bool) -> Result<()> {
    let chemin = cinema::resoudre(vfs, entree);
    let (u, taille) = charger(vfs, &chemin)?;

    // `--out` accepts a folder: the natural form for exporting several films.
    let cible = if out.is_dir() {
        out.join(format!("{}.mp4", usm::radical_de(&chemin)))
    } else {
        out.to_path_buf()
    };
    if let Some(parent) = cible.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
    }

    if u.codec.lisible_par_navigateur() && !brut {
        let c = u
            .en_conteneur_web()
            .map_err(|e| anyhow::anyhow!("remux: {e}"))?;
        // The extension follows the container chosen by the codec, not the requested one: a WebM under
        // a `.mp4` name would mislead every player that trusts the extension.
        let cible = cible.with_extension(if c.mime == "video/webm" {
            "webm"
        } else {
            "mp4"
        });
        std::fs::write(&cible, &c.octets)
            .with_context(|| format!("write {}", cible.display()))?;
        println!(
            "{} - {}x{} {}, {} frames ({} keyframes), {:.2} s, {} B (USM: {} B, {:.2} % smaller)",
            cible.display(),
            c.largeur,
            c.hauteur,
            u.codec.nom(),
            c.images,
            c.cles,
            c.secondes,
            c.octets.len(),
            taille,
            100.0 - (c.octets.len() as f64 * 100.0 / taille as f64),
        );
    } else {
        // MPEG-2 (or VP9): an `avc1` MP4 would be a lie. Write the raw elementary stream, which VLC/mpv
        // play directly, rather than a container nothing decodes.
        let cible = cible.with_extension(u.codec.extension());
        let flux = u.flux_brut();
        std::fs::write(&cible, &flux).with_context(|| format!("write {}", cible.display()))?;
        println!(
            "{} - {}x{} {}, {} frames, {:.2} s, {} B - elementary stream (no MP4 remux: \
                 no browser decodes this codec)",
            cible.display(),
            u.entete.largeur,
            u.entete.hauteur,
            u.codec.nom(),
            u.images.len(),
            u.duree().unwrap_or(0.0),
            flux.len(),
        );
    }

    if avec_audio {
        // Container first, then the `anime_stream` bank, where most soundtracks live. The archive is
        // extracted once into `var/audio-cache`, then only the wanted entry is re-read. Resolution is
        // the catalogue's, not a second implementation: the gamedata `bgmName` confirms the cue.
        let jointure = cinema::jointure_gamedata(vfs);
        let film = cinema::fiche_de_usm(vfs, &chemin, taille, &u, Some(&jointure));
        if !film.a_du_son() {
            println!("  (no soundtrack: neither in the container nor in anime_stream)");
            return Ok(());
        }
        let cache = Path::new("var/audio-cache");
        let wav = cinema::wav_bande_son(vfs, cache, &film)
            .map_err(|e| anyhow::anyhow!("soundtrack: {e}"))?;
        let provenance = match (&film.bande_son, film.audio.first()) {
            (Some(b), _) => format!("anime_stream, cue {}, {}", b.cue, b.codec),
            (None, Some(p)) => format!("container, {}", p.codec),
            (None, None) => "unknown".to_string(),
        };
        let cible_wav = cible.with_extension("wav");
        std::fs::write(&cible_wav, &wav)
            .with_context(|| format!("write {}", cible_wav.display()))?;
        println!("{} - {} B ({provenance})", cible_wav.display(), wav.len());
    }
    Ok(())
}

fn catalogue(vfs: &Vfs, out: Option<&Path>, rapide: bool) -> Result<()> {
    // `common/movie` only: `dx11/movie` carries the same films at higher resolution and the VFS
    // switches between them on its own. Two entries per film would duplicate.
    let cat = cinema::catalogue(vfs, cinema::DOSSIER_FILMS, !rapide);
    match out {
        // On screen we read; in a file we serve. `--out` writes exactly what `nie-model-serve` publishes
        // at `/video/catalog.json`: compact, fingerprint included.
        None => println!("{}", serde_json::to_string_pretty(&cat)?),
        Some(p) => {
            let texte = serde_json::to_string(&cat)?;
            if let Some(parent) = p.parent()
                && !parent.as_os_str().is_empty()
            {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(p, &texte).with_context(|| format!("write {}", p.display()))?;
            let sonores = cat.films.iter().filter(|f| f.a_du_son()).count();
            println!(
                "{} - {} film(s), {} categories, {sonores} with soundtrack",
                p.display(),
                cat.films.len(),
                cat.rubriques.len()
            );
        }
    }
    Ok(())
}

fn convert_webm(source: &Path, out: &Path, nom: Option<&str>, fps: u32) -> Result<()> {
    let brut = std::fs::read(source).with_context(|| format!("read {}", source.display()))?;
    let demux = nie_formats::webm::demuxer_webm_vp9(&brut)
        .with_context(|| format!("WebM demux {}", source.display()))?;

    let trames_ref: Vec<&[u8]> = demux.trames.iter().map(|t| t.octets.as_slice()).collect();
    let ext = out.extension().and_then(|e| e.to_str()).unwrap_or("usm");

    if let Some(parent) = out.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
    }

    match ext {
        "ivf" => {
            let ivf_bytes = nie_formats::ivf::emballer_ivf(
                demux.info.largeur as u16,
                demux.info.hauteur as u16,
                (fps, 1),
                &trames_ref,
            );
            std::fs::write(out, &ivf_bytes)
                .with_context(|| format!("IVF write {}", out.display()))?;
            println!(
                "{} - {}x{} VP9, {} frames ({} keyframes), {:.2} s, {} bytes (standalone IVF)",
                out.display(),
                demux.info.largeur,
                demux.info.hauteur,
                demux.info.total_images,
                demux.info.total_cles,
                demux.info.duree_secondes,
                ivf_bytes.len(),
            );
        }
        _ => {
            let nom_final = nom.unwrap_or_else(|| {
                source
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("video.usm")
            });
            let usm_bytes = nie_formats::usm::muxer_usm_vp9(
                nom_final,
                &trames_ref,
                (fps, 1),
                demux.info.largeur,
                demux.info.hauteur,
            );
            std::fs::write(out, &usm_bytes)
                .with_context(|| format!("USM write {}", out.display()))?;
            println!(
                "{} - {}x{} VP9, {} frames ({} keyframes), {:.2} s, {} bytes (USM/Sofdec2 container)",
                out.display(),
                demux.info.largeur,
                demux.info.hauteur,
                demux.info.total_images,
                demux.info.total_cles,
                demux.info.duree_secondes,
                usm_bytes.len(),
            );
        }
    }
    Ok(())
}
