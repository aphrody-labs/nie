//! `nie icons`: game icon index and on-demand extraction.
//!
//! ## Why an index and not a PNG folder
//!
//! The game's icon atlases are very large. Decoding all of them to PNG would produce far more
//! data than needed, since `nie-model-serve` already renders any named region on demand through
//! `/tex/<atlas>.g4tx/<region>.png`.
//!
//! This command produces the index instead: for each icon, its atlas, rectangle and size, so
//! icons can be listed and searched without materializing them.
//!
//! ## Outputs
//!
//! - `index` writes a JSON `{ "<name>": { atlas, x, y, w, h } }`;
//! - `extract` writes the PNGs of a given prefix into the disk dump, where the CDN serves them
//!   statically. Useful for always-visible icons, not for the whole set.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use nie_explore::menu_icons::{IconIndexPolicy, MenuIcon, MenuIconIndex, build_icon_index};
use nie_formats::cfgbin;
use nie_formats::vfs::Vfs;
use serde_json::json;

/// Reads an existing `key -> value` dictionary, or returns an empty table if it is missing.
///
/// A missing file is not an error: the dictionary is rebuilt. An unreadable file is not either,
/// but it must not be overwritten silently, hence the message.
fn charger_map(chemin: &Path) -> BTreeMap<String, String> {
    match std::fs::read_to_string(chemin) {
        Ok(txt) => match serde_json::from_str(&txt) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("  ({} unreadable: {e}; starting from scratch)", chemin.display());
                BTreeMap::new()
            }
        },
        Err(_) => BTreeMap::new(),
    }
}

/// `nie icons` subcommands.
#[derive(Debug, Clone, clap::Subcommand)]
pub enum IconsCmd {
    /// Write the icon index (name -> atlas + rectangle) as JSON.
    Index {
        /// VFS prefix to scan.
        #[arg(long, default_value = "menu/200_icon/")]
        prefix: String,
        /// Skipped families (subfolders), comma-separated.
        #[arg(long, default_value = "10_icon_chr,01_icon_emblem")]
        skip: String,
        /// Output file.
        #[arg(short, long, default_value = "var/icons-index.json")]
        out: PathBuf,
    },
    /// Write the hash and region dictionaries used by menu rendering to crop sprites.
    ///
    /// A runtime screen layout only knows a region's hash; resolution goes hash -> name -> atlas ->
    /// rectangle. Region names live in the `.g4tx` files, not in the Lua scripts. Without them the
    /// sprite is blitted as the whole atlas instead of its rectangle.
    Dict {
        /// VFS prefix to scan.
        #[arg(long, default_value = "menu/")]
        prefix: String,
        /// Skipped families (subfolders), comma-separated.
        #[arg(long, default_value = "")]
        skip: String,
        /// Directory of the reverse dictionaries.
        #[arg(long, default_value = "data/re")]
        out_dir: PathBuf,
        /// Write nothing; only report what the merge would add.
        #[arg(long)]
        dry_run: bool,
    },
    /// Decode the icons of a prefix to PNG in the disk dump served by the CDN.
    Extract {
        /// VFS prefix of the atlases to decode.
        #[arg(long)]
        prefix: String,
        /// Maximum number of icons written.
        #[arg(long, default_value_t = 500)]
        limit: usize,
    },
}

fn icon_index(vfs: &Vfs, prefix: &str, skip: &[String]) -> MenuIconIndex {
    let skipped: Vec<&str> = skip.iter().map(String::as_str).collect();
    let policy = IconIndexPolicy::new(prefix, ".g4tx", &skipped);
    build_icon_index(vfs, vfs.iter().map(|(path, _)| path), &policy)
}

fn icon_json(name: &str, icon: &MenuIcon) -> serde_json::Value {
    json!({
        "atlas": icon.atlas,
        "texture": icon.texture,
        "rect": icon.rect.map(|rect| json!([rect.x, rect.y, rect.w, rect.h])),
        "w": icon.width,
        "h": icon.height,
        "url": format!("/tex/{}/{}.png", icon.atlas.trim_start_matches("data/"), name),
    })
}

/// Entry point of `nie icons`.
pub fn run(cmd: &IconsCmd, game_dir: &Path) -> Result<()> {
    let mut vfs = Vfs::new();
    vfs.init(game_dir.join("data")).context("init VFS")?;

    match cmd {
        IconsCmd::Index { prefix, skip, out } => {
            let skip: Vec<String> = skip
                .split(',')
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
            let index = icon_index(&vfs, prefix, &skip);
            let doc: BTreeMap<&String, serde_json::Value> = index
                .icons
                .iter()
                .map(|(name, icon)| (name, icon_json(name, icon)))
                .collect();
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            std::fs::write(out, serde_json::to_vec_pretty(&doc)?)
                .with_context(|| format!("writing {}", out.display()))?;
                println!("indexed {} icons -> {}", index.icons.len(), out.display());
        }

        IconsCmd::Dict {
            prefix,
            skip,
            out_dir,
            dry_run,
        } => {
            let skip: Vec<String> = skip
                .split(',')
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
            let index = icon_index(&vfs, prefix, &skip);

            let chemin_crc = out_dir.join("menu-crc32-dictionary.json");
            let chemin_reg = out_dir.join("menu-region-index.json");
            let mut crc: BTreeMap<String, String> = charger_map(&chemin_crc);
            let mut reg: BTreeMap<String, String> = charger_map(&chemin_reg);
            let (crc_avant, reg_avant) = (crc.len(), reg.len());

            // A hash collision is information, not a detail to overwrite silently:
            // two distinct names for one CRC-32 would make resolution ambiguous.
            let mut collisions = 0usize;
            for (nom, ic) in &index.icons {
                let cle = format!("0x{:08X}", cfgbin::crc32(nom.as_bytes()));
                match crc.get(&cle) {
                    Some(deja) if deja != nom => collisions += 1,
                    Some(_) => {}
                    None => {
                        crc.insert(cle, nom.clone());
                    }
                }
                reg.entry(nom.clone()).or_insert_with(|| ic.atlas.clone());
            }

            println!(
                "{} region names scanned under \"{prefix}\"\n  \
                 crc32   {crc_avant} -> {crc_apres} (+{crc_delta})\n  \
                 regions {reg_avant} -> {reg_apres} (+{reg_delta})\n  \
                 hash collisions: {collisions}",
                index.icons.len(),
                crc_apres = crc.len(),
                crc_delta = crc.len() - crc_avant,
                reg_apres = reg.len(),
                reg_delta = reg.len() - reg_avant,
            );
            if *dry_run {
                println!("  (--dry-run: nothing written)");
                return Ok(());
            }
            std::fs::create_dir_all(out_dir).ok();
            std::fs::write(&chemin_crc, serde_json::to_vec_pretty(&crc)?)
                .with_context(|| format!("writing {}", chemin_crc.display()))?;
            std::fs::write(&chemin_reg, serde_json::to_vec_pretty(&reg)?)
                .with_context(|| format!("writing {}", chemin_reg.display()))?;
            println!(
                "  written: {} | {}",
                chemin_crc.display(),
                chemin_reg.display()
            );
        }

        IconsCmd::Extract { prefix, limit } => {
            let index = icon_index(&vfs, prefix, &[]);
            let mut ecrits = 0usize;
            let mut echecs = 0usize;
            for (nom, ic) in index.icons.iter().take(*limit) {
                let Ok(raw) = vfs.read(&ic.atlas) else {
                    echecs += 1;
                    continue;
                };
                // The ICON name, not its carrier texture: `decode_named_to_png` crops the region
                // rectangle when the name designates one. Passing the texture wrote the whole atlas
                // under every icon name.
                let Some(png) = nie_formats::g4tx_decode::decode_named_to_png(&raw, nom) else {
                    echecs += 1;
                    continue;
                };
                // Next to the atlas in the dump, where the CDN serves `/dx11/...` statically.
                let dossier = game_dir.join(
                    Path::new(&ic.atlas)
                        .parent()
                        .and_then(Path::to_str)
                        .unwrap_or("data"),
                );
                if std::fs::create_dir_all(&dossier).is_err() {
                    echecs += 1;
                    continue;
                }
                if std::fs::write(dossier.join(format!("{nom}.png")), png).is_ok() {
                    ecrits += 1;
                } else {
                    echecs += 1;
                }
            }
            println!(
                "{ecrits} PNGs written, {echecs} failure(s); {} indexed",
                index.icons.len()
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::icon_json;
    use nie_explore::menu_icons::{IconRect, MenuIcon};

    #[test]
    fn cli_json_keeps_the_existing_shape_around_the_shared_icon() {
        let icon = MenuIcon {
            name: "ds01001".to_owned(),
            atlas: "data/dx11/menu/200_icon/20_icon_deco/icon_deco.g4tx".to_owned(),
            texture: "icon_deco".to_owned(),
            rect: Some(IconRect {
                x: 8,
                y: 9,
                w: 80,
                h: 81,
            }),
            width: 80,
            height: 81,
        };
        assert_eq!(
            icon_json(&icon.name, &icon),
            serde_json::json!({
                "atlas": icon.atlas,
                "texture": icon.texture,
                "rect": [8, 9, 80, 81],
                "w": 80,
                "h": 81,
                "url": "/tex/dx11/menu/200_icon/20_icon_deco/icon_deco.g4tx/ds01001.png",
            })
        );
    }
}
