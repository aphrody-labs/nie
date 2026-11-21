//! Mod workflow: init, add, get/set, texture, recolor, status, validate, install, uninstall.

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::Subcommand;
use nie_formats::cfgbin;
use nie_formats::g4tx_recolor::NouvelleCharge;
use nie_viola::manifeste::{self, Manifeste};
use serde_json::Value;

/// Name of the vanilla `cpk_list.cfg.bin` backup, stored next to it.
const SAUVEGARDE: &str = "cpk_list.cfg.bin.nie-vanilla";

/// Backup path, resolved next to the real `cpk_list.cfg.bin`.
///
/// `data/cpk_list.cfg.bin` is often a symlink to the Steam install; resolving the link first keeps
/// `install` and `uninstall` looking at the same location.
fn chemin_sauvegarde(cpk_list: &Path) -> PathBuf {
    std::fs::canonicalize(cpk_list)
        .unwrap_or_else(|_| cpk_list.to_path_buf())
        .with_file_name(SAUVEGARDE)
}

/// Above this entry count a `cpk_list` is considered already packed by a previous mod.
///
/// The game legitimately ships a few loose entries (intro videos, system configuration). The
/// threshold matches the one the GUI uses via [`nie_viola::PackReport::loose_avant`].
const SEUIL_DEJA_PACKE: usize = 64;

/// Mod workflow operations.
#[derive(Subcommand)]
pub enum ModOp {
    /// Create a mod folder and its manifest.
    Init {
        /// Mod name, also used as the dependency id.
        #[arg(long)]
        nom: String,
        /// Author or team.
        #[arg(long)]
        auteur: String,
        /// Mod folder (created if missing).
        #[arg(long, short = 'd', default_value = ".")]
        dir: PathBuf,
        /// Free-form description.
        #[arg(long, default_value = "")]
        description: String,
    },
    /// Copy vanilla files into the mod, ready to edit.
    Add {
        /// VFS paths (`data/common/gamedata/...`); a folder prefix takes everything under it.
        #[arg(required = true, num_args = 1..)]
        chemins: Vec<String>,
        #[arg(long, short = 'd', default_value = ".")]
        dir: PathBuf,
        /// Overwrite files already in the mod (edits are lost).
        #[arg(long)]
        ecraser: bool,
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// Print a mod file (or its vanilla version) as JSON, whole or at a pointer.
    Get {
        /// VFS path.
        chemin: String,
        /// JSON Pointer (RFC 6901), e.g. `/entries/0/children/3/variables/2/value`.
        #[arg(long, short = 'p')]
        pointeur: Option<String>,
        /// Truncate output to N characters (0 = no truncation).
        #[arg(long, default_value_t = 4096)]
        limite: usize,
        #[arg(long, short = 'd', default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// Edit a field and re-encode the file in the mod. The pointer must match an existing node.
    Set {
        /// VFS path.
        chemin: String,
        /// JSON Pointer of the field; must match an existing node.
        pointeur: String,
        /// New value: text if the current node is a string, JSON otherwise.
        valeur: String,
        #[arg(long, short = 'd', default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// Replace a main texture with a PNG; other textures and atlas regions are copied unchanged.
    Texture {
        /// VFS path of the `.g4tx`.
        chemin: String,
        /// Source image.
        #[arg(long)]
        png: PathBuf,
        /// Main texture to replace; required when the container has several
        /// (portrait icons have two: `<code>_1_l00` and `<code>_2_l00`).
        #[arg(long)]
        texture: Option<String>,
        /// Encode as BC7 with a full mip chain (retail character texture format) instead of
        /// BGRA8 without mips. Lossy, about 3x smaller.
        #[arg(long)]
        bc7: bool,
        #[arg(long, short = 'd', default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// Recolor a `.g4tx` without touching its geometry: hue shift and/or luminance ramp.
    ///
    /// Unlike `texture`, it accepts atlas-region containers; recoloring keeps pixel dimensions, so
    /// region rectangles stay exact.
    Recolor {
        /// VFS path of the `.g4tx`.
        chemin: String,
        /// Hue shift in degrees (cyclic): `120` turns red into green.
        #[arg(long, default_value_t = 0.0, allow_negative_numbers = true)]
        teinte: f32,
        /// Saturation factor (`1` = unchanged, `0` = gray).
        #[arg(long, default_value_t = 1.0)]
        saturation: f32,
        /// Brightness factor (`1` = unchanged).
        #[arg(long, default_value_t = 1.0)]
        valeur: f32,
        /// Comma-separated luminance ramp `#RRGGBB[@position]`; three colors without positions
        /// map to shadow / midtone / highlight.
        #[arg(long)]
        rampe: Option<String>,
        /// Ramp blend strength, `0` (original) to `1` (pure ramp).
        #[arg(long, default_value_t = 1.0)]
        force: f32,
        /// Alpha below which a pixel is left untouched (transparent atlas fill).
        #[arg(long, default_value_t = 1)]
        alpha_min: u8,
        /// Restrict recoloring to one main texture; default: all.
        #[arg(long)]
        texture: Option<String>,
        #[arg(long, short = 'd', default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// Show what the mod changes, file by file, against the vanilla VFS.
    Status {
        #[arg(long, short = 'd', default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// Check the manifest, the tree, and that every file re-reads after encoding.
    Validate {
        #[arg(long, short = 'd', default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// Install the mod: back up `cpk_list`, rewrite it, copy the files.
    Install {
        /// Mod folders, from lowest to highest priority when several.
        #[arg(long, short = 'd', default_value = ".", num_args = 1..)]
        dir: Vec<PathBuf>,
        #[arg(long)]
        game_dir: Option<PathBuf>,
        /// Target Switch (`romfs/data/...`) instead of PC.
        #[arg(long)]
        switch: bool,
        /// Show what would be done without writing anything.
        #[arg(long)]
        a_blanc: bool,
    },
    /// Restore the game's original `cpk_list.cfg.bin`.
    Uninstall {
        #[arg(long)]
        game_dir: Option<PathBuf>,
        /// Also delete the mod files copied into the game.
        #[arg(long, short = 'd')]
        dir: Option<PathBuf>,
    },
}

/// A decoded `.cfg.bin`, with what is needed to re-encode it in its own variant.
///
/// RDBN needs its original lists as a template (the JSON form does not encode field types);
/// T2B does not.
enum Doc {
    /// T2B tree.
    T2b,
    /// RDBN lists, kept as the re-encoding template.
    Rdbn(Vec<cfgbin::RdbnList>),
}

/// Decode `.cfg.bin` bytes into the bridge JSON form, whatever the variant.
fn charger(octets: &[u8]) -> anyhow::Result<(Doc, Value)> {
    if cfgbin::is_rdbn(octets) {
        let brut = cfgbin::parse(octets).map_err(|e| anyhow::anyhow!("unreadable RDBN: {e}"))?;
        let listes = cfgbin::read_values(&brut, octets);
        let json = nie_explore::bridge::rdbn_to_json(&listes);
        return Ok((Doc::Rdbn(listes), json));
    }
    let cfg = cfgbin::cfgbin_parse(octets).map_err(|e| anyhow::anyhow!("unreadable T2B: {e}"))?;
    let json = nie_explore::bridge::t2b_to_json(&cfg);
    Ok((Doc::T2b, json))
}

/// Re-encode the JSON form into the original variant.
fn reencoder(doc: &Doc, json: &Value) -> anyhow::Result<Vec<u8>> {
    match doc {
        Doc::T2b => {
            let entries = nie_explore::bridge::json_to_t2b_entries(json)
                .map_err(|e| anyhow::anyhow!("JSON -> T2B: {e}"))?;
            Ok(cfgbin::encode_t2b(&entries))
        }
        Doc::Rdbn(original) => {
            let listes = nie_explore::bridge::json_to_rdbn_lists(original, json)
                .map_err(|e| anyhow::anyhow!("JSON -> RDBN: {e}"))?;
            cfgbin::encode_rdbn(&listes).map_err(|e| anyhow::anyhow!("RDBN encoding: {e}"))
        }
    }
}

/// Normalize a pointer to RFC 6901 form, accepting a missing leading `/`.
///
/// Under Git Bash, MSYS rewrites any argument starting with `/` into a Windows path, so
/// `/entries/0/...` reaches the program as `C:/Program Files/Git/entries/0/...`. Accepting
/// `entries/0/...` survives the shell intact.
fn normaliser_pointeur(p: &str) -> String {
    if p.is_empty() || p.starts_with('/') {
        p.to_string()
    } else {
        format!("/{p}")
    }
}

/// Count entries of a `cpk_list.cfg.bin` that are already outside the pack.
///
/// This is the only way to recognise a `cpk_list` an earlier mod already rewrote: the file
/// carries no marker, and starting from it would stack both mods. An unreadable `cpk_list`
/// returns `0`; `pack_mod` fails later with a better error.
fn compter_loose(cpk_list: &[u8]) -> usize {
    let Ok((cfg, _)) = nie_viola::decode_cpk_list(cpk_list) else {
        return 0;
    };
    cfg.entries.first().map_or(0, |racine| {
        racine
            .children
            .iter()
            .filter(|e| {
                e.variables.len() >= 5
                    && matches!(&e.variables[3], cfgbin::Value::String(s) if s.is_empty())
            })
            .count()
    })
}

/// Location of a VFS path inside a mod folder.
fn dans_le_mod(dir: &Path, vfs: &str) -> PathBuf {
    dir.join(vfs.trim_start_matches('/'))
}

/// Bytes of a file: the mod's copy if present, otherwise vanilla from the VFS.
///
/// Order matters: editing the same field twice must start from the first edit, not from vanilla.
fn octets_courants(dir: &Path, vfs: &str, game_dir: Option<PathBuf>) -> anyhow::Result<Vec<u8>> {
    let p = dans_le_mod(dir, vfs);
    if p.is_file() {
        return std::fs::read(&p).with_context(|| format!("read '{}'", p.display()));
    }
    let v = crate::open_vfs(game_dir)?;
    v.read(vfs)
        .map_err(|e| anyhow::anyhow!("'{vfs}' not found in the VFS: {e}"))
}

/// Write a file into the mod, creating the directory tree.
fn ecrire_dans_le_mod(dir: &Path, vfs: &str, octets: &[u8]) -> anyhow::Result<PathBuf> {
    let p = dans_le_mod(dir, vfs);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create '{}'", parent.display()))?;
    }
    std::fs::write(&p, octets).with_context(|| format!("write '{}'", p.display()))?;
    Ok(p)
}

/// Dispatch subcommands.
///
/// # Errors
/// Propagates any I/O, format or validation error.
pub fn executer(op: ModOp) -> anyhow::Result<()> {
    match op {
        ModOp::Init {
            nom,
            auteur,
            dir,
            description,
        } => init(&nom, &auteur, &dir, &description),
        ModOp::Add {
            chemins,
            dir,
            ecraser,
            game_dir,
        } => add(&chemins, &dir, ecraser, game_dir),
        ModOp::Get {
            chemin,
            pointeur,
            limite,
            dir,
            game_dir,
        } => get(&chemin, pointeur.as_deref(), limite, &dir, game_dir),
        ModOp::Set {
            chemin,
            pointeur,
            valeur,
            dir,
            game_dir,
        } => set(&chemin, &pointeur, &valeur, &dir, game_dir),
        ModOp::Texture {
            chemin,
            png,
            texture: nom,
            bc7,
            dir,
            game_dir,
        } => texture(&chemin, &png, nom.as_deref(), bc7, &dir, game_dir),
        ModOp::Recolor {
            chemin,
            teinte,
            saturation,
            valeur,
            rampe,
            force,
            alpha_min,
            texture: nom,
            dir,
            game_dir,
        } => recolor(
            &chemin,
            Filtre {
                teinte,
                saturation,
                valeur,
                rampe: rampe.as_deref(),
                force,
                alpha_min,
            },
            nom.as_deref(),
            &dir,
            game_dir,
        ),
        ModOp::Status { dir, game_dir } => status(&dir, game_dir),
        ModOp::Validate { dir, game_dir } => validate(&dir, game_dir).map(|_| ()),
        ModOp::Install {
            dir,
            game_dir,
            switch,
            a_blanc,
        } => install(&dir, game_dir, switch, a_blanc),
        ModOp::Uninstall { game_dir, dir } => uninstall(game_dir, dir.as_deref()),
    }
}

fn init(nom: &str, auteur: &str, dir: &Path, description: &str) -> anyhow::Result<()> {
    if Manifeste::chemin(dir).exists() {
        bail!(
            "'{}' already contains a mod; `init` does not overwrite it",
            dir.display()
        );
    }
    let mut m = Manifeste::gabarit(nom, auteur);
    m.description = description.to_string();
    m.ecrire(dir).map_err(anyhow::Error::msg)?;
    println!("mod       {nom} v{} by {auteur}", m.version);
    println!("manifest  {}", Manifeste::chemin(dir).display());
    println!("\nNext: `nie mod add <vfs-path>` to bring a game file into the mod.");
    Ok(())
}

fn add(
    chemins: &[String],
    dir: &Path,
    ecraser: bool,
    game_dir: Option<PathBuf>,
) -> anyhow::Result<()> {
    let vfs = crate::open_vfs(game_dir)?;
    // A folder prefix covers everything under it: adding a whole family to the mod is the common
    // case, and listing thousands of entries by hand is not.
    let mut cibles: Vec<String> = Vec::new();
    for c in chemins {
        let c = c.trim_matches('/');
        if vfs.iter().any(|(p, _)| p == c) {
            cibles.push(c.to_string());
            continue;
        }
        let prefixe = format!("{c}/");
        let sous: Vec<String> = vfs
            .iter()
            .map(|(p, _)| p)
            .filter(|p| p.starts_with(&prefixe))
            .map(String::from)
            .collect();
        if sous.is_empty() {
            bail!("'{c}' is neither a file nor a folder of the VFS");
        }
        cibles.extend(sous);
    }
    cibles.sort_unstable();
    cibles.dedup();

    let (mut ajoutes, mut sautes, mut echecs) = (0usize, 0usize, 0usize);
    for vfs_path in &cibles {
        if !ecraser && dans_le_mod(dir, vfs_path).is_file() {
            sautes += 1;
            continue;
        }
        match vfs.read(vfs_path) {
            Ok(octets) => {
                ecrire_dans_le_mod(dir, vfs_path, &octets)?;
                ajoutes += 1;
            }
            Err(e) => {
                eprintln!("  failed  {vfs_path}: {e}");
                echecs += 1;
            }
        }
    }
    println!("added     {ajoutes}");
    if sautes > 0 {
        println!("skipped   {sautes} (already in the mod; `--ecraser` to restart from vanilla)");
    }
    if echecs > 0 {
        println!("failed    {echecs}");
    }
    Ok(())
}

fn get(
    chemin: &str,
    pointeur: Option<&str>,
    limite: usize,
    dir: &Path,
    game_dir: Option<PathBuf>,
) -> anyhow::Result<()> {
    let octets = octets_courants(dir, chemin, game_dir)?;
    let (_, json) = charger(&octets)?;
    let pointeur = pointeur.map(normaliser_pointeur);
    let vue = match &pointeur {
        None => &json,
        Some(p) => json.pointer(p).ok_or_else(|| {
            anyhow::anyhow!("pointer '{p}' matches nothing in this file")
        })?,
    };
    let texte = serde_json::to_string_pretty(vue)?;
    if limite > 0 && texte.len() > limite {
        // Count characters, not bytes: cutting inside a multibyte character would produce an invalid
        // string, and these files hold Japanese text.
        let coupe: String = texte.chars().take(limite).collect();
        println!("{coupe}");
        println!(
            "... truncated to {limite} characters of {} (`--limite 0` shows everything)",
            texte.chars().count()
        );
    } else {
        println!("{texte}");
    }
    Ok(())
}

fn set(
    chemin: &str,
    pointeur: &str,
    valeur: &str,
    dir: &Path,
    game_dir: Option<PathBuf>,
) -> anyhow::Result<()> {
    let octets = octets_courants(dir, chemin, game_dir)?;
    let (doc, mut json) = charger(&octets)?;
    let pointeur = &normaliser_pointeur(pointeur);

    let cible = json.pointer_mut(pointeur).ok_or_else(|| {
        anyhow::anyhow!(
            "pointer '{pointeur}' matches nothing; `set` never creates a field, \
                 because an added field could not be re-encoded"
        )
    })?;

    let avant = cible.clone();
    // In the bridge form every variable value is a string, including integers and floats, which
    // the `nie-data` readers re-parse. Keeping the existing type avoids producing JSON the encoder
    // will reject.
    if cible.is_string() {
        *cible = Value::String(valeur.to_string());
    } else {
        *cible = serde_json::from_str(valeur).with_context(|| {
            format!("'{valeur}' is not JSON, and the target node is not a string")
        })?;
    }
    let apres = cible.clone();

    let nouveaux = reencoder(&doc, &json)?;
    // Re-read what was just written, immediately: an encoding that does not decode back must fail
    // here, not at install time, and above all not in the game.
    let (_, verif) =
        charger(&nouveaux).context("the re-encoded file does not re-read; edit refused")?;
    // Report the value read back, not just that it differs: without it a drifted pointer (missing
    // node) cannot be told from a reformatted value.
    match verif.pointer(pointeur) {
        Some(relu) if relu == &apres => {}
        Some(relu) => {
            bail!("the re-encoded file reads {relu} where {apres} was written; edit refused")
        }
        None => {
            // Say WHERE the path breaks: a lost pointer is not diagnosable without it. Walk down segment
            // by segment to the first missing one.
            let mut chemin = String::new();
            let mut rompu = String::from("(root)");
            for seg in pointeur.split('/').skip(1) {
                chemin.push('/');
                chemin.push_str(seg);
                if verif.pointer(&chemin).is_none() {
                    rompu = chemin.clone();
                    break;
                }
            }
            let avant_rupture = rompu.rsplit_once('/').map_or("", |(p, _)| p).to_string();
            let dispo = verif.pointer(&avant_rupture).map_or_else(
                || "—".to_string(),
                |v| match v {
                    Value::Array(a) => format!("array of {} elements", a.len()),
                    Value::Object(o) => {
                        format!(
                            "object {{{}}}",
                            o.keys().cloned().collect::<Vec<_>>().join(", ")
                        )
                    }
                    autre => format!("{autre}"),
                },
            );
            bail!(
                "after re-encoding, pointer '{pointeur}' breaks at '{rompu}'; \
                 '{avant_rupture}' contains: {dispo}; edit refused"
            )
        }
    }

    let p = ecrire_dans_le_mod(dir, chemin, &nouveaux)?;
    println!("file      {}", p.display());
    println!("pointer   {pointeur}");
    println!("before    {avant}");
    println!("after     {apres}");
    println!("bytes     {} -> {}", octets.len(), nouveaux.len());
    Ok(())
}

fn texture(
    chemin: &str,
    png: &Path,
    nom_texture: Option<&str>,
    bc7: bool,
    dir: &Path,
    game_dir: Option<PathBuf>,
) -> anyhow::Result<()> {
    let octets = octets_courants(dir, chemin, game_dir)?;
    let atlas = nie_formats::g4tx::parse(&octets)
        .map_err(|e| anyhow::anyhow!("'{chemin}' is not a readable G4TX: {e}"))?;
    if atlas.textures.is_empty() {
        bail!("{chemin}: no texture");
    }

    // Which main texture is replaced. A single-texture container needs no `--texture`; beyond that
    // the choice must be explicit rather than guessed: portrait icons carry two textures
    // (`<code>_1_l00` / `<code>_2_l00`) that are not interchangeable.
    let index = match nom_texture {
        Some(nom) => atlas
            .textures
            .iter()
            .position(|t| t.name == nom)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "'{chemin}' has no texture '{nom}'; available: {}",
                    noms_des_textures(&atlas)
                )
            })?,
        None if atlas.textures.len() == 1 => 0,
        None => bail!(
            "'{chemin}' has {} textures; pass --texture, one of: {}",
            atlas.textures.len(),
            noms_des_textures(&atlas)
        ),
    };

    // A texture split into atlas regions stays refused: several regions share one image, so
    // "replace" has no single meaning. Regions attached to the OTHER textures are copied unchanged.
    if !atlas.textures[index].sub_textures.is_empty() {
        bail!(
            "'{chemin}': texture '{}' has {} atlas region(s); replacing a texture split into regions \
                 is not defined",
            atlas.textures[index].name,
            atlas.textures[index].sub_textures.len()
        );
    }

    let image = std::fs::read(png).with_context(|| format!("read '{}'", png.display()))?;
    let (l, h, rgba) = nie_formats::g4tx_encode::decode_png_to_rgba8(&image)
        .map_err(|e| anyhow::anyhow!("'{}': {e}", png.display()))?;
    let dds = if bc7 {
        #[cfg(feature = "textures-encode")]
        {
            nie_formats::g4tx_encode::encode_dds_bc7_mipmapped(l, h, &rgba)
        }
        #[cfg(not(feature = "textures-encode"))]
        {
            Err(
                "BC7 encoding requires building nie-cli with `--features textures-encode`"
                    .to_string(),
            )
        }
    } else {
        nie_formats::g4tx_encode::encode_dds_bgra8(l, h, &rgba)
    }
    .map_err(|e| anyhow::anyhow!("DDS encoding: {e}"))?;
    let (li, hi) = (i16::try_from(l)?, i16::try_from(h)?);

    // Rebuilding the container (untouched payloads copied byte for byte, atlas regions carried
    // over, immediate re-read of the result) lives in `nie_formats::g4tx_recolor`. It is the single
    // source shared with `nie mod recolor`: two rebuilders would drift, and this one already has
    // four invariants to keep (names, ids, region counts, reparse).
    let mut charges: Vec<NouvelleCharge> = atlas
        .textures
        .iter()
        .map(|_| NouvelleCharge::Inchangee)
        .collect();
    charges[index] = NouvelleCharge::Dds {
        dds,
        width: li,
        height: hi,
    };
    let nouveaux = nie_formats::g4tx_recolor::reencode_with_payloads(&octets, &atlas, &charges)
        .map_err(|e| anyhow::anyhow!("'{chemin}': {e}; edit refused"))?;

    let p = ecrire_dans_le_mod(dir, chemin, &nouveaux)?;
    println!("texture   {} - {l}x{h}", atlas.textures[index].name);
    if atlas.textures.len() > 1 {
        println!(
            "kept       {}",
            atlas
                .textures
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != index)
                .map(|(_, t)| t.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    println!("file      {}", p.display());
    println!("bytes     {} -> {}", octets.len(), nouveaux.len());
    Ok(())
}

/// Settings of `nie mod recolor`, grouped: seven more arguments on `recolor` would trip
/// `clippy::too_many_arguments` and read as a row of anonymous floats.
struct Filtre<'a> {
    teinte: f32,
    saturation: f32,
    valeur: f32,
    rampe: Option<&'a str>,
    force: f32,
    alpha_min: u8,
}

fn recolor(
    chemin: &str,
    filtre: Filtre<'_>,
    nom_texture: Option<&str>,
    dir: &Path,
    game_dir: Option<PathBuf>,
) -> anyhow::Result<()> {
    let rampe = match filtre.rampe {
        Some(spec) => {
            let mut r = nie_formats::recolor::Ramp::parse(spec)
                .map_err(|e| anyhow::anyhow!("--rampe: {e}"))?;
            r.strength = filtre.force;
            Some(r)
        }
        None => None,
    };
    let f = nie_formats::recolor::Recolor {
        hue_shift: filtre.teinte,
        saturation: filtre.saturation,
        value: filtre.valeur,
        ramp: rampe,
        alpha_min: filtre.alpha_min,
    };

    let octets = octets_courants(dir, chemin, game_dir)?;
    let r = nie_formats::g4tx_recolor::recolour(&octets, &f, nom_texture)
        .map_err(|e| anyhow::anyhow!("'{chemin}': {e}"))?;

    let p = ecrire_dans_le_mod(dir, chemin, &r.octets)?;
    for (nom, l, h) in &r.recolorees {
        println!("recolored {nom} - {l}x{h}");
    }
    if !r.hors_cible.is_empty() {
        println!("off target {}", r.hors_cible.join(", "));
    }
    // An unreadable payload is COPIED, never replaced by nothing: say so, separately from
    // `off target`, or a half-recolored container passes for fully recolored and the only line
    // that would have flagged it blends in with a caller choice.
    if !r.illisibles.is_empty() {
        println!(
            "UNREADABLE {} - copied unchanged",
            r.illisibles.join(", ")
        );
    }
    println!("file      {}", p.display());
    println!("bytes     {} -> {}", octets.len(), r.octets.len());
    Ok(())
}

/// Main texture names of a container, for `--texture` error messages.
fn noms_des_textures(atlas: &nie_formats::g4tx::G4tx) -> String {
    atlas
        .textures
        .iter()
        .map(|t| t.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn status(dir: &Path, game_dir: Option<PathBuf>) -> anyhow::Result<()> {
    let m = Manifeste::lire(dir).map_err(anyhow::Error::msg)?;
    let fichiers = manifeste::fichiers(dir).map_err(anyhow::Error::msg)?;
    println!("mod       {} v{} by {}", m.nom, m.version, m.auteur);
    if !m.description.is_empty() {
        println!("           {}", m.description);
    }
    println!("priority  {}", m.priorite);
    println!("files     {}", fichiers.len());

    if fichiers.is_empty() {
        return Ok(());
    }
    let vfs = crate::open_vfs(game_dir)?;
    let (mut modifies, mut neufs, mut identiques) = (0usize, 0usize, 0usize);
    for f in &fichiers {
        let taille = std::fs::metadata(&f.absolu).map(|x| x.len()).unwrap_or(0);
        match vfs.read(&f.vfs) {
            Err(_) => {
                neufs += 1;
                println!(
                    "  new        {} ({taille} B) - absent from vanilla",
                    f.vfs
                );
            }
            Ok(vanilla) => {
                let a_jour = std::fs::read(&f.absolu).unwrap_or_default();
                if a_jour == vanilla {
                    identiques += 1;
                } else {
                    modifies += 1;
                    println!("  modified   {} ({} -> {taille} B)", f.vfs, vanilla.len());
                }
            }
        }
    }
    if identiques > 0 {
        // A file identical to vanilla changes nothing but still flips its entry to loose: not neutral,
        // and staying silent would yield a mod bigger than its effect.
        println!(
            "  {identiques} identical to vanilla; no effect, but installed anyway"
        );
    }
    println!("summary   {modifies} modified, {neufs} new, {identiques} identical");
    Ok(())
}

/// Validate a mod and return its files. Shared core of `validate` and `install`.
fn validate(dir: &Path, game_dir: Option<PathBuf>) -> anyhow::Result<Vec<manifeste::FichierMod>> {
    let m = Manifeste::lire(dir).map_err(anyhow::Error::msg)?;
    let mut fautes: Vec<String> = m.reproches();

    // A file outside `data/` would never be loaded, and never reported: the exact trap of a
    // flattened-name working folder.
    for f in manifeste::valider_arborescence(dir).map_err(anyhow::Error::msg)? {
        fautes.push(format!(
            "'{f}' is not under data/; the game would never load it"
        ));
    }

    let fichiers = manifeste::fichiers(dir).map_err(anyhow::Error::msg)?;
    let vfs = crate::open_vfs(game_dir).ok();
    for f in &fichiers {
        // A path missing from the VFS is not fatal (a mod can add a file), but it is almost always a
        // typo: say so without refusing.
        if let Some(v) = &vfs
            && !v.iter().any(|(p, _)| p == f.vfs)
        {
            println!(
                "  note     {} is absent from vanilla (added file?)",
                f.vfs
            );
        }
        if !f.vfs.ends_with(".cfg.bin") {
            continue;
        }
        // The real check: a mod `.cfg.bin` must decode. If it does not decode here, the game will not
        // read it either.
        let octets = std::fs::read(&f.absolu)
            .with_context(|| format!("read '{}'", f.absolu.display()))?;
        if let Err(e) = charger(&octets) {
            fautes.push(format!("{}: unreadable - {e}", f.vfs));
        }
    }

    if fautes.is_empty() {
        println!(
            "mod       {} v{} - {} file(s)",
            m.nom,
            m.version,
            fichiers.len()
        );
        println!("verdict   valid");
        return Ok(fichiers);
    }
    for f in &fautes {
        println!("  fault    {f}");
    }
    bail!("{} fault(s); mod invalid", fautes.len())
}

fn install(
    dirs: &[PathBuf],
    game_dir: Option<PathBuf>,
    switch: bool,
    a_blanc: bool,
) -> anyhow::Result<()> {
    let racine = crate::racine_jeu(game_dir.clone());
    let data = racine.join("data");
    let cpk_list = data.join("cpk_list.cfg.bin");
    let sauvegarde = chemin_sauvegarde(&cpk_list);
    if !cpk_list.is_file() {
        bail!(
            "'{}' not found; not a game install",
            cpk_list.display()
        );
    }

    // Validate BEFORE touching the game. A refused mod must leave no trace.
    let mut manifestes = Vec::with_capacity(dirs.len());
    for d in dirs {
        validate(d, game_dir.clone())?;
        manifestes.push(Manifeste::lire(d).map_err(anyhow::Error::msg)?);
    }
    // Apply order comes from dependencies then priority, never from the command-line order;
    // otherwise `enabled`/`priorite` would be decorative.
    let ordre = nie_viola::ordonner(&manifestes).map_err(anyhow::Error::msg)?;
    let sources: Vec<PathBuf> = ordre.iter().rev().map(|&i| dirs[i].clone()).collect();
    if sources.len() > 1 {
        println!(
            "order     {} (highest priority first)",
            sources
                .iter()
                .filter_map(|p| p.file_name())
                .map(|n| n.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" > ")
        );
    }

    // The `cpk_list` given to `pack_mod` must be vanilla: starting from an already packed one would
    // stack a previous mod's entries. The backup is therefore always the source.
    if !sauvegarde.exists() {
        let vanilla = std::fs::read(&cpk_list)?;
        let deja_loose = compter_loose(&vanilla);
        if deja_loose > SEUIL_DEJA_PACKE {
            bail!(
                "this cpk_list already has {deja_loose} entries outside the pack: it was packed by a \
                     previous mod and no backup exists. Restore the vanilla file (Steam file verification) \
                     before installing."
            );
        }
        if a_blanc {
            println!(
                "dry run   would back up {} -> {}",
                cpk_list.display(),
                sauvegarde.display()
            );
        } else {
            std::fs::copy(&cpk_list, &sauvegarde)?;
            println!("backup     {}", sauvegarde.display());
        }
    } else {
        println!(
            "backup     {} (already present, reused as vanilla)",
            sauvegarde.display()
        );
    }

    if a_blanc {
        for d in &sources {
            let n = manifeste::fichiers(d).map_err(anyhow::Error::msg)?.len();
            println!(
                "dry run   {}: {n} file(s) to {}",
                d.display(),
                racine.display()
            );
        }
        println!("dry run   nothing written");
        return Ok(());
    }

    // Several mods: merge them first, per field, so two mods touching different fields of one
    // file both survive.
    let temporaire = std::env::temp_dir().join(format!("nie-mod-fusion-{}", std::process::id()));
    let source_unique = if sources.len() == 1 {
        sources[0].clone()
    } else {
        let vfs = crate::open_vfs(game_dir.clone())?;
        let resoudre = |rel: &str| vfs.read(rel).ok();
        let r = nie_viola::merge_dirs(
            &sources,
            &temporaire,
            &nie_viola::MergeStrategy::Semantique(&resoudre),
        )
        .map_err(anyhow::Error::msg)?;
        println!(
            "merge     {} copied, {} merged, {} conflicts",
            r.copies,
            r.fusionnes,
            r.conflits.len()
        );
        for c in &r.conflits {
            println!(
                "  conflict {} - {} fields disagree",
                c.chemin, c.champs_en_desaccord
            );
        }
        temporaire.clone()
    };

    let _ = switch; // the platform does not change the byte patch: same format, same envelope.

    // Install by **byte patch**, never by re-encoding. `encode_t2b` is not faithful on this file
    // (measured: a no-op round trip changes the size and millions of bytes) and the game then
    // refuses the `cpk_list` (E-02000000), even when the mod is identical to vanilla. Here we
    // decrypt, write 4 bytes per file, and re-encrypt: the AES envelope is faithful.
    let vanilla = std::fs::read(&sauvegarde)?;
    let mut clair = nie_formats::cpk::decrypt_cpk_list(&vanilla)
        .map_err(|e| anyhow::anyhow!("cpk_list decryption: {e}"))?;
    let avant = clair.len();

    let fichiers = manifeste::fichiers(&source_unique).map_err(anyhow::Error::msg)?;
    let chemins: Vec<String> = fichiers.iter().map(|f| f.vfs.clone()).collect();
    let rapport =
        nie_viola::patch::patcher_clair(&mut clair, &chemins).map_err(anyhow::Error::msg)?;
    anyhow::ensure!(
        clair.len() == avant,
        "the patch changed the cpk_list size; aborting"
    );

    if !rapport.introuvables.is_empty() {
        // Adding an entry would shift every offset: out of reach of an in-place patch.
        bail!(
            "{} file(s) missing from the cpk_list; patch install can only REPLACE an existing file, \
                 not ADD a new one: {}",
            rapport.introuvables.len(),
            rapport.introuvables.join(", ")
        );
    }

    let rechiffre = nie_formats::cpk::encrypt_cpk_list(&clair);
    anyhow::ensure!(
        rechiffre.len() == vanilla.len(),
        "re-encryption changed the size ({} -> {}); aborting",
        vanilla.len(),
        rechiffre.len()
    );
    std::fs::write(&cpk_list, &rechiffre)?;

    // Copy the mod files where the game will now look for them.
    let mut copies = 0usize;
    for f in &fichiers {
        let dest = racine.join(&f.vfs);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&f.absolu, &dest)?;
        copies += 1;
    }
    std::fs::remove_dir_all(&temporaire).ok();

    println!("made loose  {}", rapport.rendus_loose.len());
    println!("already loose {}", rapport.deja_loose.len());
    println!(
        "bytes patched {} (of {} in the cpk_list)",
        rapport.octets_modifies, avant
    );
    println!("copied     {copies}");
    println!("installed in {}", racine.display());
    println!("\n`nie mod uninstall` restores the original cpk_list, byte for byte.");
    Ok(())
}

fn uninstall(game_dir: Option<PathBuf>, dir: Option<&Path>) -> anyhow::Result<()> {
    let racine = crate::racine_jeu(game_dir);
    let data = racine.join("data");
    let cpk_list = data.join("cpk_list.cfg.bin");
    let sauvegarde = chemin_sauvegarde(&cpk_list);
    if !sauvegarde.is_file() {
        bail!(
            "no backup at '{}'; nothing to restore (the mod was not installed by \
                 `nie mod install`)",
            sauvegarde.display()
        );
    }
    let vanilla = std::fs::read(&sauvegarde)?;
    std::fs::write(&cpk_list, &vanilla)?;
    // Verify rather than assert: it is cheap, and "restored" must mean restored.
    let relu = std::fs::read(&cpk_list)?;
    if relu != vanilla {
        bail!("the restore did not give back the same bytes; the game is in an uncertain state");
    }
    println!(
        "restored  {} ({} bytes, identical to the backup)",
        cpk_list.display(),
        relu.len()
    );

    if let Some(d) = dir {
        let fichiers = manifeste::fichiers(d).map_err(anyhow::Error::msg)?;
        let mut retires = 0usize;
        for f in &fichiers {
            let cible = racine.join(&f.vfs);
            if cible.is_file() && std::fs::remove_file(&cible).is_ok() {
                retires += 1;
            }
        }
        println!("removed   {retires} mod file(s)");
    } else {
        // Without the cpk_list that references them these files are inert: leaving them is the safe
        // choice, deleting them unasked is not.
        println!("note      the mod files stay on disk, now inert;");
        println!("          `--dir <mod>` removes them too.");
    }
    std::fs::remove_file(&sauvegarde).ok();
    Ok(())
}
