//! `nie avatar`: the avatar editor (`chara_edit`) catalog, resolved end to end.
//!
//! ## What the command joins
//!
//! The editor catalog lives in two `cfg.bin` files under `gamedata/character/` and three `cfg.bin`
//! files placed next to the models, in `chr/_face/20_EDIT/`. None is self-sufficient:
//!
//! | file | contribution |
//! |---|---|
//! | `chara_edit_<ver>.cfg.bin` | lists: parts, sliders, colors, voices, preset recipes |
//! | `chara_edit_parts_type_config_<ver>.cfg.bin` | base models per body type (face, accessories) |
//! | `20_EDIT/center.cfg.bin` | center `(u, v, w, h)` of each part texture in the face atlas |
//! | `20_EDIT/texPartsDefaultPose.cfg.bin` | default pose (translation/scale) of each part |
//! | `20_EDIT/editCharaMdlParts.cfg.bin` | model rules: face offset, ears hidden by hair |
//!
//! ## The three hash families and what they really name
//!
//! Everything is CRC-32 (`nie_formats::cfgbin::crc32`), but the targets differ and the field
//! names mislead:
//!
//! - `resourceName1` / `resourceName2`: the 3D model in the VFS (`hairF001` -> `_hairF/hairF001.g4md`
//!   + `.g4mg`). Also the key of the `20_EDIT` pose tables.
//! - `textureName`: not a model texture but the UI icon of the thumbnail in the editor grid
//!   (`icon_ava_face06_004`), resolved through `hash_name` in the knowledge base (source
//!   `vfs-ui`, see `nie seed-ui`).
//! - `presetID` of a recipe: the hash of a part name in the "preset" category
//!   (`preset_01_normal`); the same id is both a selectable thumbnail and a recipe.
//!
//! Category labels are not invented here: they are derived from the longest common prefix of the
//! category's resource names, as they appear in the files.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use nie_data::avatar::{
    asset_family, category_resource_prefix, category_screen, longest_known_label_run,
    numeric_table_by_hash, palette_grid, unique_known_labels,
};
use nie_data::chara_edit::{CharaEditConfig, parse_chara_edit, parse_chara_edit_parts_type_config};
use nie_formats::cfgbin;
use nie_formats::vfs::Vfs;
use serde_json::{Value as Json, json};

/// Root of the editor assets in the VFS.
const EDIT_ROOT: &str = "chr/_face/20_EDIT/";

/// The game statistics, in clockwise order around the radar of the statistics screen.
///
/// These hashes are `menu_text` keys, so labels are translated like the rest. They do not come
/// from the `chara_edit` constant pool: that screen borrows the game-wide statistic labels, and
/// the order below is the one captures show around the radar (strike on top, then clockwise). It
/// is the only list in this module whose screen membership comes from a capture rather than an
/// editor file.
const STATS_RADAR: [u32; 7] = [
    0x9676_CFF2, // Strike
    0xBF19_862F, // Control
    0x5E95_B812, // Pressure
    0xA9F1_F72B, // Physical
    0xE0F2_0046, // Agility
    0xAC95_CB90, // Intelligence
    0xD02A_224B, // Technique
];

/// Editor screen matching a category, by lexical matching.
///
/// Game screens are named `chara_edit_parts_menu_<what>` (`_eye`, `_mouth`, `_eyebrow`, `_ear`,
/// `_hair`, `_nose`, `_contour`, `_preset`...) and each category carries a resource prefix
/// (`eye_`, `mouth_`, `eyebrow_`, `hairF`...). When one contains the other the link is
/// mechanical: no hand-written table, and nothing is returned when no name matches.
fn ecran_de_categorie(prefixe: &str, ecrans: &[String]) -> Option<String> {
    category_screen(prefixe, ecrans)
}

/// Grid geometry of a palette, deduced from its color count.
///
/// The game names its palette screens after their grid (`chara_edit_color_menu_10x4_skin`,
/// `_12x5`, `_13x5`) and each palette's color count matches exactly: 40 = 10x4 (skin),
/// 60 = 12x5 (eye), 65 = 13x5 (hair). The geometry is therefore cross-checked by two independent
/// sources, not guessed. Other sizes return `None` rather than an invented layout.
fn geometrie_palette(n: usize) -> Option<(usize, usize)> {
    palette_grid(n)
}

/// A decoded image: width, height, RGBA bytes.
type Rgba = (u32, u32, Vec<u8>);

/// Collects, in order, the numeric constants of a Lua prototype and its nested ones.
fn constantes_num(proto: &nie_lua::bytecode::Prototype, out: &mut Vec<u32>) {
    for c in &proto.constants {
        if let nie_lua::bytecode::Constant::Number(n) = c {
            // Hashes are stored as `double`: only exact integers that fit in
            // 32 bits can be one.
            if n.fract() == 0.0 && *n >= 0.0 && *n <= f64::from(u32::MAX) {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                out.push(*n as u32);
            } else {
                out.push(0);
            }
        } else {
            out.push(0);
        }
    }
    for p in &proto.protos {
        constantes_num(p, out);
    }
}

/// The editor sections, in the order the game shows them.
///
/// The script of the list screen (`chara_edit_list_menu`) carries, in its constant pool, a
/// contiguous run of label hashes, and that run is the order of the editor's left column. It is
/// found mechanically: the longest consecutive sequence of numeric constants that are all
/// `menu_text` keys.
///
/// The order and the labels are established; the link between a section and a specific
/// `faceSettingType` is not (the menu code makes it, and the repo's Lua driver pairs no object
/// on this screen), so no mapping is invented here.
fn rubriques(vfs: &Vfs, textes: &BTreeMap<u32, String>) -> Vec<(u32, String)> {
    let Some(chemin) = vfs.iter().map(|(p, _)| p.to_string()).find(|p| {
        p.rsplit('/')
            .next()
            .is_some_and(|b| b.starts_with("chara_edit_list_menu") && b.ends_with(".lua.bin"))
    }) else {
        return Vec::new();
    };
    let Ok(octets) = vfs.read(&chemin) else {
        return Vec::new();
    };
    let Ok(chunk) = nie_lua::bytecode::parse(&octets) else {
        return Vec::new();
    };

    let mut nums = Vec::new();
    constantes_num(&chunk.main, &mut nums);

    longest_known_label_run(&nums, textes)
}

/// An editor panel: a screen script and the labels it shows, in order.
struct Panneau {
        /// Script base name, without version or `.lua.bin` (`chara_edit_parts_menu_contour`).
    nom: String,
        /// Labels resolved in `menu_text`, in the script's constant-pool order:
        /// `(hash, displayable text, label gaiji)`, see `nie_data::text::split_markup`.
    libelles: Vec<(u32, String, Vec<String>)>,
}

/// The labels of each editor panel, in the order of the script that shows them.
///
/// Same mechanism as [`rubriques`], applied to all `chara_edit*` scripts: a screen script's
/// constant pool carries the hashes of the labels it places, in order of first appearance in the
/// code, hence display order. Only hashes that are `menu_text` keys are kept: an arbitrary numeric
/// constant (an index, a color) is not one, and a 32-bit collision is unlikely.
///
/// Unlike [`rubriques`], no contiguity is required: a panel mixes its labels with layout
/// constants. The order is the pool's, not the drawing's: it shows membership in the panel, not
/// the on-screen position of a label.
fn panneaux(vfs: &Vfs, textes: &BTreeMap<u32, String>) -> Vec<Panneau> {
    let mut chemins: Vec<String> = vfs
        .iter()
        .filter_map(|(p, _)| {
            let base = p.rsplit('/').next()?;
            (base.starts_with("chara_edit") && base.ends_with(".lua.bin")).then(|| p.to_string())
        })
        .collect();
    chemins.sort();
    chemins.dedup();

    let mut out = Vec::new();
    for chemin in chemins {
        let Ok(octets) = vfs.read(&chemin) else {
            continue;
        };
        let Ok(chunk) = nie_lua::bytecode::parse(&octets) else {
            continue;
        };
        let mut nums = Vec::new();
        constantes_num(&chunk.main, &mut nums);

        let libelles: Vec<(u32, String, Vec<String>)> = unique_known_labels(&nums, textes)
            .into_iter()
            .map(|(h, brut)| {
                let (texte, gaiji) = nie_data::text::split_markup(&brut);
                (h, texte, gaiji)
            })
            .collect();
        if libelles.is_empty() {
            continue;
        }
        // `chara_edit_parts_menu_contour_1.03.71.00.lua.bin` → `chara_edit_parts_menu_contour`.
        let base = chemin.rsplit('/').next().unwrap_or(&chemin);
        let sans_ext = base.strip_suffix(".lua.bin").unwrap_or(base);
        let nom = match sans_ext.rsplit_once('_') {
            Some((tete, queue)) if queue.chars().next().is_some_and(|c| c.is_ascii_digit()) => tete,
            _ => sans_ext,
        };
        out.push(Panneau {
            nom: nom.to_string(),
            libelles,
        });
    }
    out
}

/// Name fragments denoting content the repo does not reproduce yet.
const MOTIFS_DYNAMIQUES: [&str; 4] = ["model", "chara_3d", "cursor", "shadow"];

/// `nie avatar roi`: derives the measurement regions of a screen from its layout.
fn roi(ecran: &str, layouts: &Path, out: Option<&Path>) -> Result<()> {
    let chemin = layouts.join(format!("{ecran}.json"));
    let txt = std::fs::read_to_string(&chemin)
        .with_context(|| format!("reading {}", chemin.display()))?;
    let doc: Json = serde_json::from_str(&txt)
        .with_context(|| format!("invalid layout: {}", chemin.display()))?;
    let (cw, ch) = (
        doc["canvas"]["w"].as_f64().unwrap_or(1280.0),
        doc["canvas"]["h"].as_f64().unwrap_or(720.0),
    );

    let mut rois: Vec<Json> = Vec::new();
    let mut non_derivables: Vec<String> = Vec::new();

    for o in doc["objects"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        let nom = o["name"].as_str().unwrap_or_default();
        let bas = nom.to_ascii_lowercase();
        if !MOTIFS_DYNAMIQUES.iter().any(|m| bas.contains(m)) {
            continue;
        }
        let (sw, sh) = (o["sprite"]["w"].as_f64(), o["sprite"]["h"].as_f64());
        let t = &o["transform"];
        let (sx, sy) = (
            t["scaleX"].as_f64().unwrap_or(1.0),
            t["scaleY"].as_f64().unwrap_or(1.0),
        );
        let (x, y) = (
            t["x"].as_f64().unwrap_or(0.0),
            t["y"].as_f64().unwrap_or(0.0),
        );

        // Fallback signature: dummy sprite, or object parked at the exact center at
        // native scale. In both cases the layout geometry does not describe what the
        // capture shows: the avatar fills a large central area, not a 4 px square.
        let factice = sw.unwrap_or(0.0) <= 8.0 || sh.unwrap_or(0.0) <= 8.0;
        let parque = (sx - 1.0).abs() < 1e-6
            && (sy - 1.0).abs() < 1e-6
            && (x - cw / 2.0).abs() < 0.5
            && (y - ch / 2.0).abs() < 0.5;
        if factice || parque {
            non_derivables.push(String::from(nom));
            continue;
        }

        let (Some(sw), Some(sh)) = (sw, sh) else {
            continue;
        };
        let (bw, bh) = (sw * sx, sh * sy);
        let (bx, by) = ((x - bw / 2.0).max(0.0), (y - bh / 2.0).max(0.0));
        rois.push(json!({
            "nom": nom,
            "rect": [bx.round() as i64, by.round() as i64, bw.round() as i64, bh.round() as i64],
            "kind": "dynamique",
        }));
    }

    println!("{ecran}: {} region(s) derived from the layout", rois.len());
    for r in &rois {
        println!("  {} {:?}", r["nom"].as_str().unwrap_or(""), r["rect"]);
    }
    if !non_derivables.is_empty() {
        non_derivables.sort_unstable();
        non_derivables.dedup();
        println!(
            "  {} dynamic object(s) without usable geometry, no region produced for them:",
            non_derivables.len()
        );
        for n in &non_derivables {
            println!("    {n}");
        }
        println!(
            "  These objects are 3D slots: the layout gives them a dummy sprite parked at the\n  \
             center. Their real extent is in no file read here, so the measurement still counts\n  \
             them, and the `nie img diff` report shows a smaller excluded area."
        );
    }

    if let Some(out) = out {
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(out, serde_json::to_vec_pretty(&Json::Array(rois))?)
            .with_context(|| format!("writing {}", out.display()))?;
        println!("  wrote: {}", out.display());
    }
    Ok(())
}

/// Family of a sheet: the first path segment after the swept prefix.
///
/// `.../_facetex/04_eyebrow/eyebrow_00.g4tx` under the prefix `.../_facetex/` gives `04_eyebrow`. A
/// sheet placed directly under the prefix has no family and goes under `.`.
fn famille_de(chemin: &str, prefix: &str) -> String {
    asset_family(chemin, prefix)
}

/// Formats a fraction as a percentage with two decimals and a comma separator.
fn pct(part: f32) -> String {
    format!("{:.2} %", part * 100.0).replace('.', ",")
}

/// Formats a normalized extent `u[a;b] v[c;d]`, or `—` if the zone is empty.
fn fmt_emprise(e: Option<[f32; 4]>) -> String {
    e.map_or_else(
        || "—".to_string(),
        |[u0, v0, u1, v1]| format!("u[{u0:.3};{u1:.3}] v[{v0:.3};{v1:.3}]").replace('.', ","),
    )
}

/// Measurements of a sheet as JSON, as carried by [`nie_formats::planche::Mesures`].
fn mesures_json(m: &nie_formats::planche::Mesures) -> Json {
    use nie_formats::planche::Zone;
    let zones: serde_json::Map<String, Json> = Zone::toutes()
        .into_iter()
        .map(|z| {
            (
                z.nom().to_string(),
                json!({
                    "part": m.part(z),
                    "emprise": m.emprise(z).map(Vec::from),
                }),
            )
        })
        .collect();
    json!({
        "largeur": m.largeur,
        "hauteur": m.hauteur,
        "pixels": m.pixels,
        "zones": zones,
        "part_encre": m.part_encre,
        "emprise_encre": m.emprise_encre.map(Vec::from),
        "alpha_moyen": m.alpha_moyen,
        "alpha_min": m.alpha_min,
        "alpha_max": m.alpha_max,
        "canaux_constants": m.canaux_constants,
        "couleurs": m.couleurs,
        "couleurs_plafonnees": m.couleurs_plafonnees,
        "couleur_moyenne": m.couleur_moyenne,
    })
}

/// Side of the UV occupancy grid: 32 x 32 cells over the square `[0, 1]²`.
///
/// Fine enough to tell an unwrap covering the whole square from one occupying only a corner,
/// coarse enough for a mesh of a few hundred vertices to fill its cells.
const GRILLE_UV: usize = 32;

/// `nie avatar depliage`: reports which part of the UV square each submesh samples.
///
/// This is the missing link between a sheet and the mesh that reads it. A sheet is measured
/// (see [`nie_formats::planche`]), a mesh is unwrapped, and a composition is only right if the
/// two overlap. The grouping of families by material, [`nie_formats::assemble::face_layer_slot`],
/// rests on "disjoint" unwraps recorded once by hand; this command makes them regenerable.
///
/// Two measurements per submesh, because the bounding box lies on a sparse unwrap: its extent
/// (u and v bounds) and its occupancy, the share of cells of a [`GRILLE_UV`] x [`GRILLE_UV`]
/// grid that hold at least one vertex.
fn depliage(game_dir: &Path, modele: &str, limit: usize) -> Result<()> {
    let mut vfs = Vfs::new();
    vfs.init(game_dir.join("data")).context("init VFS")?;
    let mut chemins: Vec<String> = vfs
        .iter()
        .filter(|(p, _)| p.contains(EDIT_ROOT) && p.ends_with(".g4md") && p.contains(modele))
        .map(|(p, _)| p.to_string())
        .collect();
    chemins.sort();
    chemins.dedup();
    if chemins.is_empty() {
        println!("no .g4md under {EDIT_ROOT} contains \"{modele}\"");
        return Ok(());
    }
    let total = chemins.len();
    chemins.truncate(limit);
    println!("{total} model(s), {} shown", chemins.len());

    for chemin in &chemins {
        let Ok(brut_md) = vfs.read(chemin) else {
            continue;
        };
        let Ok(md) = nie_formats::g4md::parse(&brut_md) else {
            println!("  {} — unreadable g4md", court(chemin));
            continue;
        };
        let chemin_mg = format!("{}.g4mg", chemin.trim_end_matches(".g4md"));
        let Ok(brut_mg) = vfs.read(&chemin_mg) else {
            println!("  {} — .g4mg missing", court(chemin));
            continue;
        };
        let geos = nie_formats::g4mg::extract_geometry(&brut_mg, &md);
        println!("  {} — {} submesh(es)", court(chemin), geos.len());
        for geo in &geos {
            let materiau = nie_formats::g4mg::material_base_name(&md, geo)
                .map_or("(unnamed)", String::as_str);
            if geo.uv0.is_empty() {
                println!(
                    "    #{} {materiau:<18} {:>6} vtx.  no UV0",
                    geo.index, geo.vertex_count
                );
                continue;
            }
            let (mut u0, mut v0, mut u1, mut v1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            let mut cases = alloc_grille();
            for uv in &geo.uv0 {
                if !uv.u.is_finite() || !uv.v.is_finite() {
                    continue;
                }
                u0 = u0.min(uv.u);
                v0 = v0.min(uv.v);
                u1 = u1.max(uv.u);
                v1 = v1.max(uv.v);
                // A UV outside [0, 1] is legal (tiling): fold it for occupancy, without
                // touching the extent, which must stay truthful even when it overflows.
                let cx = ((uv.u.rem_euclid(1.0)) * GRILLE_UV as f32) as usize;
                let cy = ((uv.v.rem_euclid(1.0)) * GRILLE_UV as f32) as usize;
                cases[cy.min(GRILLE_UV - 1) * GRILLE_UV + cx.min(GRILLE_UV - 1)] = true;
            }
            let occupees = cases.iter().filter(|c| **c).count();
            println!(
                "    #{} {materiau:<18} {:>6} vtx.  u[{u0:.3};{u1:.3}] v[{v0:.3};{v1:.3}]  \
                 occupancy {}",
                geo.index,
                geo.vertex_count,
                pct(occupees as f32 / (GRILLE_UV * GRILLE_UV) as f32)
            );
        }
    }
    Ok(())
}

/// Empty occupancy grid.
fn alloc_grille() -> Vec<bool> {
    vec![false; GRILLE_UV * GRILLE_UV]
}

/// Writes an RGBA buffer as PNG, optionally downscaled to at most `cote` pixels per side.
fn ecrire_png(dir: &Path, nom: &str, w: u32, h: u32, rgba: &[u8], cote: u32) -> Result<()> {
    let (w, h, pixels) = if cote > 0 {
        nie_formats::image_out::reduire_rgba(rgba, w, h, cote)
            .map_err(|e| anyhow::anyhow!("resizing {nom}: {e}"))?
    } else {
        (w, h, rgba.to_vec())
    };
    let png = nie_formats::g4tx_decode::encode_rgba_to_png(&pixels, w as usize, h as usize)
        .with_context(|| format!("PNG encoding of {nom}"))?;
    std::fs::write(dir.join(nom), png).with_context(|| format!("writing {nom}"))?;
    Ok(())
}

/// Writes a sheet, its mask, and the trace its convention derives, as PNG.
///
/// The trace is not tinted: channel tinting depends on the color recipe chosen in the editor,
/// which is out of scope here. Only the alpha comes from the convention, and it is what we want to
/// see: an eyebrow is recognized by its shape, not its color.
///
/// A silent sheet has no trace, and an undetermined convention does not invent one: both cases
/// write only the color and the mask. Returns the number of files written.
fn extraire_planche(
    dir: &Path,
    vfs: &Vfs,
    chemin: &str,
    famille: &str,
    fiche: &nie_formats::planche::Fiche,
    cote: u32,
) -> Result<usize> {
    use nie_formats::image_out;
    use nie_formats::planche::Convention;

    let brut = vfs
        .read(chemin)
        .with_context(|| format!("reading {chemin}"))?;
    let Some((w, h, rgba)) = nie_formats::g4tx_decode::decode_named_to_rgba(&brut, &fiche.nom)
    else {
        return Ok(0);
    };
    let masque = fiche
        .nom_masque
        .as_ref()
        .and_then(|nm| nie_formats::g4tx_decode::decode_named_to_rgba(&brut, nm))
        .map(|(_, _, m)| m);

    let base = format!("{famille}__{}", fiche.nom);
    let mut ecrits = 0usize;
    ecrire_png(dir, &format!("{base}.png"), w, h, &rgba, cote)?;
    ecrits += 1;
    if let Some(m) = &masque {
        ecrire_png(dir, &format!("{base}.msk.png"), w, h, m, cote)?;
        ecrits += 1;
    }

    let trace = match (fiche.convention, masque.as_ref()) {
        (Convention::FondRouge, Some(m)) => image_out::decouper_par_zones(w, h, &rgba, m),
        (Convention::TraceVert, Some(m)) => image_out::decouper_oeil(w, h, &rgba, m),
        (Convention::Decoupe, Some(m)) => {
            // Gray mask: its red channel is the opacity.
            let mut copie = rgba.clone();
            for i in (0..copie.len()).step_by(4) {
                copie[i + 3] = m[i];
            }
            Some(copie)
        }
        (Convention::SansMasque, _) => Some(rgba.clone()),
        _ => None,
    };
    if let Some(t) = trace {
        ecrire_png(dir, &format!("{base}.trace.png"), w, h, &t, cote)?;
        ecrits += 1;
    }
    Ok(ecrits)
}

/// Options of `nie avatar planches`, as the command line carries them.
struct OptionsPlanches<'a> {
        /// Swept VFS prefix.
    prefix: &'a str,
        /// Fragment the path must contain.
    filtre: Option<&'a str>,
        /// Detail sheet by sheet.
    detail: bool,
        /// JSON file for the full record.
    out: Option<&'a Path>,
        /// PNG extraction directory.
    extraire: Option<&'a Path>,
        /// Maximum side of extracted PNGs; 0 = original size.
    vignette: u32,
        /// Maximum number of containers analyzed.
    limit: usize,
}

/// `nie avatar planches`: measures the sheets under a VFS prefix and aggregates by family.
///
/// The command only observes: it modifies no game data. Its value is contrast: a family whose
/// sheets all give the same convention justifies the rule coded for it, a family giving two says
/// the per-family rule is wrong, and a silent sheet (no trace in the color, no shape in the mask)
/// explains a missing model piece without suspecting the compositor.
///
/// With `--extraire`, it also writes what it measured: the sheet, its mask, and the trace the
/// convention derives. It is the only way to check a convention other than by percentages: an
/// eyebrow trace is recognized by eye, not by its share of green.
fn planches(game_dir: &Path, o: &OptionsPlanches) -> Result<()> {
    let &OptionsPlanches {
        prefix,
        filtre,
        detail,
        out,
        extraire,
        vignette,
        limit,
    } = o;
    use nie_formats::planche::{self, Role, Zone};

    let mut vfs = Vfs::new();
    vfs.init(game_dir.join("data")).context("init VFS")?;
    let mut chemins: Vec<String> = vfs
        .iter()
        .filter(|(p, _)| p.contains(prefix) && p.ends_with(".g4tx"))
        .map(|(p, _)| p.to_string())
        .collect();
    chemins.sort();
    chemins.dedup();
    if let Some(f) = filtre {
        chemins.retain(|p| p.contains(f));
    }
    chemins.truncate(limit);
    println!(
        "{} .g4tx container(s) under {prefix}{}",
        chemins.len(),
        filtre.map_or(String::new(), |f| format!(" — filter \"{f}\""))
    );

        /// What is kept for a family.
    #[derive(Default)]
    struct Agregat {
        conteneurs: usize,
        planches: usize,
        roles: BTreeMap<&'static str, usize>,
        conventions: BTreeMap<&'static str, usize>,
        muettes: Vec<String>,
    }

    let mut par_famille: BTreeMap<String, Agregat> = BTreeMap::new();
    let mut releve: Vec<Json> = Vec::new();
    let (mut illisibles, mut sans_planche, mut png_ecrits) = (0usize, 0usize, 0usize);

    for chemin in &chemins {
        let Ok(raw) = vfs.read(chemin) else {
            illisibles += 1;
            continue;
        };
        let fiches = planche::analyser(&raw);
        if fiches.is_empty() {
            sans_planche += 1;
            continue;
        }
        let famille = famille_de(chemin, prefix);
        let agr = par_famille.entry(famille.clone()).or_default();
        agr.conteneurs += 1;

        for f in &fiches {
            agr.planches += 1;
            *agr.roles.entry(f.role.nom()).or_default() += 1;
            *agr.conventions.entry(f.convention.nom()).or_default() += 1;
            if f.est_muette() {
                agr.muettes.push(format!("{}:{}", court(chemin), f.nom));
            }
            if detail {
                let zones: Vec<String> = f
                    .couleur
                    .zones_presentes()
                    .into_iter()
                    .map(|(z, p)| format!("{} {}", z.nom(), pct(p)))
                    .collect();
                println!(
                    "  {:<44} {:<16} {}×{}  {:<6} {:<12} ink {:<8} [{}]",
                    court(chemin),
                    f.nom,
                    f.couleur.largeur,
                    f.couleur.hauteur,
                    f.role.nom(),
                    f.convention.nom(),
                    pct(f.couleur.part_encre),
                    zones.join(", ")
                );
                if let (Some(nm), Some(m), Some(r)) = (&f.nom_masque, &f.masque, f.role_masque) {
                    let zones: Vec<String> = m
                        .zones_presentes()
                        .into_iter()
                        .map(|(z, p)| format!("{} {}", z.nom(), pct(p)))
                        .collect();
                    println!(
                        "  {:<44} {:<16} {:<6} {:<6} green {} [{}]",
                        "",
                        nm,
                        "msk",
                        r.nom(),
                        fmt_emprise(m.emprise(Zone::Vert)),
                        zones.join(", ")
                    );
                }
            }
            if out.is_some() {
                releve.push(json!({
                    "chemin": chemin,
                    "famille": famille,
                    "planche": f.nom,
                    "role": f.role.nom(),
                    "convention": f.convention.nom(),
                    "muette": f.est_muette(),
                    "couleur": mesures_json(&f.couleur),
                    "masque": f.nom_masque.as_ref().map(|nm| json!({
                        "nom": nm,
                        "role": f.role_masque.map(Role::nom),
                        "mesures": f.masque.as_ref().map(mesures_json),
                    })),
                }));
            }
            if let Some(dir) = extraire {
                std::fs::create_dir_all(dir)?;
                png_ecrits += extraire_planche(dir, &vfs, chemin, &famille, f, vignette)?;
            }
        }
    }

    if illisibles > 0 || sans_planche > 0 {
        println!("  {illisibles} unreadable, {sans_planche} without a color sheet");
    }
    if let Some(dir) = extraire {
        println!("  {png_ecrits} PNG written to {}", dir.display());
    }
    println!();
    println!(
        "  {:<14} {:>5} {:>5}  {:<28} conventions",
        "family", "cont.", "tex.", "roles"
    );
    for (famille, agr) in &par_famille {
        let liste = |m: &BTreeMap<&'static str, usize>| -> String {
            let mut v: Vec<(&&str, &usize)> = m.iter().collect();
            v.sort_by(|a, b| b.1.cmp(a.1));
            v.iter()
                .map(|(k, n)| format!("{k} {n}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        println!(
            "  {:<14} {:>5} {:>5}  {:<28} {}",
            famille,
            agr.conteneurs,
            agr.planches,
            liste(&agr.roles),
            liste(&agr.conventions)
        );
    }

    let muettes: usize = par_famille.values().map(|a| a.muettes.len()).sum();
    if muettes > 0 {
        println!();
        println!(
            "  {muettes} silent sheet(s) — no trace in the color, no shape in the mask:"
        );
        for (famille, agr) in &par_famille {
            if agr.muettes.is_empty() {
                continue;
            }
            println!(
                "    {famille:<14} {}/{} — {}",
                agr.muettes.len(),
                agr.planches,
                agr.muettes
                    .iter()
                    .take(4)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }

    // A minority convention is the useful signal: it shows that a rule coded per family
    // does not cover the whole family.
    let mixtes: Vec<&String> = par_famille
        .iter()
        .filter(|(_, a)| a.conventions.len() > 1)
        .map(|(f, _)| f)
        .collect();
    if !mixtes.is_empty() {
        println!();
        println!(
            "  families with more than one convention: {}",
            mixtes
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    if let Some(fichier) = out {
        if let Some(parent) = fichier.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(fichier, serde_json::to_vec_pretty(&releve)?)
            .with_context(|| format!("writing {}", fichier.display()))?;
        println!();
        println!(
            "  {} sheet(s) recorded → {}",
            releve.len(),
            fichier.display()
        );
    }
    Ok(())
}

/// Path shortened to what tells one sheet from another: family and file name.
fn court(chemin: &str) -> String {
    let mut segments: Vec<&str> = chemin.rsplit('/').take(2).collect();
    segments.reverse();
    segments.join("/")
}

/// Subcommands of `nie avatar`.
#[derive(Debug, Clone, clap::Subcommand)]
pub enum AvatarCmd {
        /// Overview: the lists, the categories and their resolution rate.
    Catalog,
        /// Parts of a category (all if `--category` is absent), with models and icon.
    Parts {
                /// Target `faceSettingType`.
        #[arg(long)]
        category: Option<i64>,
                /// Maximum number of rows shown.
        #[arg(long, default_value_t = 40)]
        limit: usize,
    },
        /// Preset faces; with an argument, the decoded recipe of a preset.
    Preset {
                /// Name (`preset_01_normal`) or hash (`0x06DE46FB`) of the preset.
        id: Option<String>,
    },
        /// Write the full resolved catalog as JSON.
    Export {
                /// Output file.
        #[arg(short, long, default_value = "var/avatar-resolved.json")]
        out: std::path::PathBuf,
    },
        /// Derive the measurement regions of a screen from its layout, for `nie img diff`.
        ///
        /// A rectangle drawn on a capture is tuned to the reference image: it raises the score
        /// without proving anything and cannot be regenerated. Here the regions come from the
        /// layout objects, hence from game files. When an object has no usable geometry, the command
        /// declares it non-derivable instead of inventing a box.
    Roi {
                /// Screen (layout name, without `.json`).
        ecran: String,
                /// Directory of exported layouts.
        #[arg(long, default_value = "var/avatar-ui/layouts")]
        layouts: std::path::PathBuf,
                /// Output file; without it, the command only prints what it finds.
        #[arg(short, long)]
        out: Option<std::path::PathBuf>,
    },
        /// Measure the editor texture sheets: zones, role, composition convention.
        ///
        /// Face composition conventions were first set by hand, sheet by sheet, on one or two
        /// textures each, then coded per family (`if rel.starts_with("01_eye")`). This command
        /// measures them across the whole corpus without looking at family names: a convention that
        /// truly holds for `01_eye` must be readable in the bytes of all its sheets. See
        /// `nie_formats::planche`.
    Planches {
                /// Swept VFS prefix.
        #[arg(long, default_value = "chr/_face/20_EDIT/_facetex/")]
        prefix: String,
                /// Keep only paths containing this fragment (a family, a sheet name).
        #[arg(long)]
        filtre: Option<String>,
                /// Detail sheet by sheet instead of only the per-family aggregate.
        #[arg(long)]
        detail: bool,
                /// Write the full record as JSON.
        #[arg(short, long)]
        out: Option<std::path::PathBuf>,
                /// Extract as PNG, into this directory, the sheet, its mask and the trace the
                /// measured convention derives.
        #[arg(long)]
        extraire: Option<std::path::PathBuf>,
                /// Maximum side of extracted PNGs; 0 keeps the original size.
        #[arg(long, default_value_t = 0)]
        vignette: u32,
                /// Maximum number of containers analyzed.
        #[arg(long, default_value_t = usize::MAX)]
        limit: usize,
    },
        /// Measure the UV unwrap of a `20_EDIT` model, submesh by submesh.
        ///
        /// Answers the question sheet measurement leaves open: which mesh reads which sheet. A
        /// full-frame sheet on a mesh that occupies only a corner of the square cannot render what it
        /// draws.
    Depliage {
                /// Model name or path fragment (`face51_nose01`, `_facebase/face51`).
        modele: String,
                /// Maximum number of models shown.
        #[arg(long, default_value_t = 8)]
        limit: usize,
    },
        /// Locate the thumbnail icons in the UI atlases, and extract them as PNG.
    Icons {
                /// Output directory; without it, the command only locates.
        #[arg(short, long)]
        out: Option<std::path::PathBuf>,
                /// VFS prefix of the swept atlases.
        #[arg(long, default_value = "menu/200_icon/21_icon_avatar/")]
        atlas_prefix: String,
                /// Maximum number of icons extracted.
        #[arg(long, default_value_t = usize::MAX)]
        limit: usize,
    },
}

/// The editor files, already decoded to tagged-form JSON.
struct Sources {
        /// `chara_edit_<ver>.cfg.bin`: the lists.
    catalogue: CharaEditConfig,
        /// `chara_edit_parts_type_config_<ver>.cfg.bin`: base models per body type.
    types: nie_data::chara_edit::CharaEditPartsTypeConfig,
        /// `20_EDIT/center.cfg.bin`: face atlas.
    center: Json,
        /// `20_EDIT/texPartsDefaultPose.cfg.bin`: default poses.
    pose: Json,
        /// `20_EDIT/editCharaMdlParts.cfg.bin`: model rules.
    rules: Json,
        /// `stem` -> VFS paths, for everything under `20_EDIT/`.
    assets: BTreeMap<String, Vec<String>>,
        /// `menu_text` of the requested locale: hash -> label shown by the game.
    textes: BTreeMap<u32, String>,
        /// Game root, kept for passes that reopen the VFS (icon extraction).
    game_dir: std::path::PathBuf,
        /// Editor sections in game order, `(hash, label)`, see [`rubriques`].
    rubriques: Vec<(u32, String)>,
        /// Editor shaders, `(file name, size)`, see [`shaders_editeur`].
    shaders: Vec<(String, u64)>,
        /// Game `chara_edit*` screens, as the VFS names them.
    ecrans: Vec<String>,
        /// Labels of each editor panel, see [`panneaux`].
    panneaux: Vec<Panneau>,
}

/// The editor shaders: the `chr_edit_toon` family of `dx11/shader/`.
///
/// The game stores shaders by name, and those of the character editor form a clear family:
/// `chr_edit_toon` and its variants `_hair`, `_metal`, `_metal_adv`, `_cutout`, with the
/// `_eff_status(_tex)` suffixes. Each program comes as a triplet: `.fxbin` (descriptor),
/// `.vfxo` (vertex), `.pfxo` (pixel).
///
/// Identification is by name, not content: the bytecode is compiled DXBC, which the repo does
/// not disassemble. Nothing here claims to say what these shaders compute.
fn shaders_editeur(vfs: &Vfs) -> Vec<(String, u64)> {
    let mut out: Vec<(String, u64)> = vfs
        .iter()
        .filter_map(|(chemin, entree)| {
            let base = chemin.rsplit('/').next()?;
            if !chemin.contains("/shader/") || !base.starts_with("chr_edit_toon") {
                return None;
            }
            Some((base.to_string(), u64::from(entree.file_size)))
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Finds the unique file whose base name starts with `prefix` and ends with `.cfg.bin`.
fn find_one(vfs: &Vfs, contains: &str, prefix: &str) -> Result<String> {
    let mut hits: Vec<String> = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| {
            p.contains(contains)
                && p.rsplit('/')
                    .next()
                    .is_some_and(|b| b.starts_with(prefix) && b.ends_with(".cfg.bin"))
        })
        .collect();
    hits.sort();
    hits.into_iter()
        .next()
        .with_context(|| format!("{prefix}*.cfg.bin not found in the VFS"))
}

/// Reads a `cfg.bin` from the VFS and returns it as tagged-form JSON (list-style RDBN or T2B, either).
fn read_json(vfs: &Vfs, path: &str) -> Result<Json> {
    let raw = vfs.read(path).with_context(|| format!("reading {path}"))?;
    cfgbin::to_tagged_json(&raw).with_context(|| format!("decoding cfg.bin {path}"))
}

impl Sources {
        /// Opens the VFS and loads the five files plus the asset index of `20_EDIT/`.
    fn load(game_dir: &Path) -> Result<Self> {
        let mut vfs = Vfs::new();
        vfs.init(game_dir.join("data"))
            .context("init VFS from cpk_list.cfg.bin")?;

        let p_cat = find_one(&vfs, "/gamedata/character/", "chara_edit_")?;
        // `chara_edit_parts_type_config_*` also starts with `chara_edit_`: alphabetical order
        // puts the catalog first (`chara_edit_1.` < `chara_edit_p`), but the second one is
        // targeted by its own prefix so as not to depend on that order.
        let p_types = find_one(&vfs, "/gamedata/character/", "chara_edit_parts_type_config")?;
        eprintln!("  catalog   : {p_cat}");
        eprintln!("  types     : {p_types}");

        let catalogue = parse_chara_edit(&read_json(&vfs, &p_cat)?);
        let types = parse_chara_edit_parts_type_config(&read_json(&vfs, &p_types)?);

        let in_edit = |base: &str| -> Result<Json> {
            let path = vfs
                .iter()
                .map(|(p, _)| p.to_string())
                .find(|p| p.contains(EDIT_ROOT) && p.ends_with(base))
                .with_context(|| format!("{base} not found under {EDIT_ROOT}"))?;
            read_json(&vfs, &path)
        };
        let center = in_edit("center.cfg.bin")?;
        let pose = in_edit("texPartsDefaultPose.cfg.bin")?;
        let rules = in_edit("editCharaMdlParts.cfg.bin")?;

        let mut assets: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (path, _) in vfs.iter() {
            if !path.contains(EDIT_ROOT) {
                continue;
            }
            let Some(base) = path.rsplit('/').next() else {
                continue;
            };
            let stem = base.split_once('.').map_or(base, |(s, _)| s);
            assets
                .entry(stem.to_string())
                .or_default()
                .push(path.to_string());
        }
        for v in assets.values_mut() {
            v.sort();
        }

        // `menu_text` holds the labels the game shows for personalities and
        // avatar files; the catalog references them only by CRC-32.
        let mut textes = BTreeMap::new();
        if let Some(path) = vfs
            .iter()
            .map(|(p, _)| p.to_string())
            .find(|p| p.contains("/text/fr/") && p.ends_with("menu_text.cfg.bin"))
            && let Ok(json) = read_json(&vfs, &path)
        {
            for (hash, texte) in nie_data::text::parse_text_file(&json) {
                textes.insert(hash.get(), texte);
            }
        }

        let mut ecrans: Vec<String> = vfs
            .iter()
            .filter_map(|(p, _)| {
                let base = p.rsplit('/').next()?;
                let nom = base.strip_suffix("_setting.cfg.bin")?;
                nom.starts_with("chara_edit").then(|| nom.to_string())
            })
            .collect();
        ecrans.sort();
        ecrans.dedup();

        let rubriques = rubriques(&vfs, &textes);
        let shaders = shaders_editeur(&vfs);
        let panneaux = panneaux(&vfs, &textes);

        Ok(Self {
            catalogue,
            types,
            center,
            pose,
            rules,
            assets,
            textes,
            game_dir: game_dir.to_path_buf(),
            rubriques,
            shaders,
            ecrans,
            panneaux,
        })
    }
}

/// A thumbnail icon located in a UI atlas.
struct IconLoc {
        /// VFS path of the `g4tx` atlas.
    atlas: String,
        /// Name of the main texture to decode.
    texture: String,
        /// Region rectangle `(x, y, w, h)`, if the icon is an atlas region.
    rect: Option<(i16, i16, i16, i16)>,
}

/// Indexes the texture and region names of the `g4tx` files under `prefix`: name -> where to decode.
///
/// Editor thumbnails are mostly regions of a large atlas (`avatar01_00.g4tx`): the name looked
/// up is not that of a whole texture.
fn index_atlases(
    vfs: &Vfs,
    prefix: &str,
    voulus: &BTreeMap<u32, String>,
) -> BTreeMap<String, IconLoc> {
    let mut out: BTreeMap<String, IconLoc> = BTreeMap::new();
    let noms: std::collections::BTreeSet<&str> = voulus.values().map(String::as_str).collect();
    let paths: Vec<String> = vfs
        .iter()
        .map(|(p, _)| p.to_string())
        .filter(|p| p.contains(prefix) && p.ends_with(".g4tx"))
        .collect();
    for path in paths {
        let Ok(raw) = vfs.read(&path) else { continue };
        let Ok(tx) = nie_formats::g4tx::parse(&raw) else {
            continue;
        };
        for tex in &tx.textures {
            if noms.contains(tex.name.as_str()) {
                out.entry(tex.name.clone()).or_insert(IconLoc {
                    atlas: path.clone(),
                    texture: tex.name.clone(),
                    rect: None,
                });
            }
            for sub in &tex.sub_textures {
                if noms.contains(sub.name.as_str()) {
                    out.entry(sub.name.clone()).or_insert(IconLoc {
                        atlas: path.clone(),
                        texture: tex.name.clone(),
                        rect: Some((sub.x, sub.y, sub.width, sub.height)),
                    });
                }
            }
        }
    }
    out
}

/// Crops a rectangle from an RGBA buffer, clamped to the real dimensions.
fn crop_rgba(
    rgba: &[u8],
    w: u32,
    h: u32,
    rect: (i16, i16, i16, i16),
) -> Option<(u32, u32, Vec<u8>)> {
    let (rx, ry, rw, rh) = rect;
    let (x0, y0) = (
        u32::try_from(rx.max(0)).ok()?,
        u32::try_from(ry.max(0)).ok()?,
    );
    let (cw, ch) = (
        u32::try_from(rw.max(0)).ok()?,
        u32::try_from(rh.max(0)).ok()?,
    );
    if cw == 0 || ch == 0 || x0 >= w || y0 >= h {
        return None;
    }
    let cw = cw.min(w - x0);
    let ch = ch.min(h - y0);
    let mut out = Vec::with_capacity((cw * ch * 4) as usize);
    for y in 0..ch {
        let start = (((y0 + y) * w + x0) * 4) as usize;
        out.extend_from_slice(&rgba[start..start + (cw * 4) as usize]);
    }
    Some((cw, ch, out))
}

/// `hash -> name` dictionary read from `hash_name` in the knowledge base.
///
/// Without a database the dictionary is empty: icons stay hashes, everything else works.
fn icon_dict(db_path: &Path) -> BTreeMap<u32, String> {
    let mut out = BTreeMap::new();
    let Ok(db) = nie_index::Db::open(db_path) else {
        eprintln!(
            "  (database {} unreadable: icons unresolved)",
            db_path.display()
        );
        return out;
    };
    let Ok(mut stmt) = db
        .conn()
        .prepare("SELECT hash, name FROM hash_name WHERE kind='texture'")
    else {
        return out;
    };
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .ok();
    if let Some(rows) = rows {
        for (hash, name) in rows.flatten() {
            out.insert(hash as u32, name);
        }
    }
    out
}

/// VFS paths of a named resource (model, mesh, texture).
fn resource_paths<'a>(src: &'a Sources, name: &str) -> &'a [String] {
    src.assets.get(name).map_or(&[][..], Vec::as_slice)
}

/// Centers of `TEX_PARTS_CENTER_INFO`, resolved to texture sheet names.
///
/// The table is keyed by `crc32`, and these hashes do NOT name part `resourceName`s, contrary to
/// what the rest of the catalog assumes: they are the sub-texture names of the `_facetex/`
/// containers, found by hashing the names the `.g4tx` files declare themselves. Every entry
/// then resolves (`eyebrow_R_01`, `eyebrow_L_01`, `eye_R_01`, `eye_L_01`, `mouth_00`).
///
/// This is facial anatomy: eyebrows on top, eyes below, mouth at the bottom, left/right pairs
/// mirrored exactly around `u = 0.5`. It is also the same extent measured on the control grid
/// in `nie_formats::image_out`, so the file already said what a capture was used to recover.
///
/// The table is a reference, not one entry per sheet. Nothing here extrapolates to the other
/// variants.
fn centres_de_planches(game_dir: &Path, center: &Json) -> BTreeMap<String, Vec<f64>> {
    let table = numeric_table_by_hash(center, "TEX_PARTS_CENTER_INFO");
    let mut vfs = Vfs::new();
    if vfs.init(game_dir.join("data")).is_err() {
        return BTreeMap::new();
    }
    let conteneurs: Vec<String> = vfs
        .iter()
        .filter(|(p, _)| p.contains("20_EDIT/_facetex/") && p.ends_with(".g4tx"))
        .map(|(p, _)| p.to_string())
        .collect();
    let mut out = BTreeMap::new();
    for chemin in conteneurs {
        let Ok(brut) = vfs.read(&chemin) else {
            continue;
        };
        let Ok(conteneur) = nie_formats::g4tx::parse(&brut) else {
            continue;
        };
        for tex in &conteneur.textures {
            if let Some(rect) = table.get(&cfgbin::crc32(tex.name.as_bytes())) {
                out.insert(tex.name.clone(), rect.clone());
            }
        }
    }
    out
}

/// Resolved view of a part: what the catalog says, plus what the VFS and the database confirm.
fn part_json(
    src: &Sources,
    icons: &BTreeMap<u32, String>,
    trans: &BTreeMap<u32, Vec<f64>>,
    scales: &BTreeMap<u32, Vec<f64>>,
    p: &nie_data::chara_edit::CharaEditPartsData,
) -> Json {
    let name1 = &p.resource_name_str1;
    let hash1 = p.resource_name1.get();
    let hash2 = p.resource_name2.get();
    let tex = p.texture_name.get();
    json!({
        "id": p.id.to_hex_x8(),
        "itemNo": p.item_no,
        "viewNo": p.view_no,
        "gender": p.gender,
        "resource": name1,
        "resourceHash": p.resource_name1.to_hex_x8(),
        "resource2": (!p.resource_name_str2.starts_with("0x")).then(|| p.resource_name_str2.clone()),
        "modeles": resource_paths(src, name1),
        "modeles2": if !p.resource_name_str2.starts_with("0x") { resource_paths(src, &p.resource_name_str2) } else { &[] },
        "icone": icons.get(&tex),
        "iconeHash": p.texture_name.to_hex_x8(),
        // No atlas center here: `TEX_PARTS_CENTER_INFO` is keyed by SHEET name, not by part
        // `resourceName`. The resolved centers live separately, see [`centres_de_planches`].
        // `TEX_PARTS_DEFAULT_TRANS_INFO` = (hash, integer mode, then the components): the mode
        // is split off here, otherwise it reads as a translation of 3.0 units.
        "poseTransMode": trans.get(&hash1).and_then(|v| v.first()).map(|m| *m as i64),
        "poseTrans": trans.get(&hash1).map(|v| v.get(1..).unwrap_or_default()),
        "poseScale": scales.get(&hash1),
        "resource2Connu": hash2 != 0 && src.assets.contains_key(&p.resource_name_str2),
    })
}

/// Entry point of `nie avatar`.
pub fn run(cmd: &AvatarCmd, game_dir: &Path, db_path: &Path) -> Result<()> {
    // Sheet analysis reads only textures: serving it before the catalog avoids
    // decoding five `cfg.bin` files and the screen bytecode for nothing, and keeps it usable even
    // where the catalog does not resolve.
    if let AvatarCmd::Depliage { modele, limit } = cmd {
        return depliage(game_dir, modele, *limit);
    }
    if let AvatarCmd::Planches {
        prefix,
        filtre,
        detail,
        out,
        extraire,
        vignette,
        limit,
    } = cmd
    {
        return planches(
            game_dir,
            &OptionsPlanches {
                prefix,
                filtre: filtre.as_deref(),
                detail: *detail,
                out: out.as_deref(),
                extraire: extraire.as_deref(),
                vignette: *vignette,
                limit: *limit,
            },
        );
    }
    let src = Sources::load(game_dir)?;
    let icons = icon_dict(db_path);
    let centres = centres_de_planches(game_dir, &src.center);
    let trans = numeric_table_by_hash(&src.pose, "TEX_PARTS_DEFAULT_TRANS_INFO");
    let scales = numeric_table_by_hash(&src.pose, "TEX_PARTS_DEFAULT_SCALE_INFO");
    let cfg = &src.catalogue;

    match cmd {
        AvatarCmd::Catalog => {
            println!("chara_edit — avatar editor catalog");
            println!(
                "  {} parts, {} categories, {} sliders, {} colors, {} recipes ({} rows)",
                cfg.parts.len(),
                cfg.parts_info.len(),
                cfg.params.len(),
                cfg.colors.len(),
                cfg.preset_info.len(),
                cfg.preset_data.len()
            );
            println!(
                "  {} voices, {} personalities, {} outfits, {} avatar files",
                cfg.voices.len(),
                cfg.personalities.len(),
                cfg.fashions.len(),
                cfg.preset_files.len()
            );
            println!(
                "  share code: {} bits over an alphabet of {} characters ({} slots)",
                cfg.code_bit_width(),
                cfg.codes.len(),
                cfg.recipes.len()
            );
            println!(
                "  base models: {} faces × body type, {} accessories",
                src.types.face_data.len(),
                src.types.body_data.len()
            );
            println!(
                "  20_EDIT tables: {} sheet centers, {} translations, {} scales",
                centres.len(),
                trans.len(),
                scales.len()
            );
            // The centers are few and show where each part sits on the face square:
            // listing them is better than counting them.
            for (nom, rect) in &centres {
                if let [u, v, w, h] = rect[..] {
                    println!("    {nom:<14} u {u:.4}  v {v:.4}  w {w:.4}  h {h:.4}");
                }
            }
            println!("\n  cat  parts   models   icons  resource prefix");
            for info in &cfg.parts_info {
                let parts = cfg.parts_of(info.face_setting_type);
                let with_model = parts
                    .iter()
                    .filter(|p| !resource_paths(&src, &p.resource_name_str1).is_empty())
                    .count();
                let with_icon = parts
                    .iter()
                    .filter(|p| icons.contains_key(&(p.texture_name.get())))
                    .count();
                println!(
                    "  {:>3}  {:>5}  {:>7}  {:>6}  {}",
                    info.face_setting_type,
                    parts.len(),
                    with_model,
                    with_icon,
                    category_resource_prefix(cfg, info.face_setting_type)
                );
            }
            let total_model = cfg
                .parts
                .iter()
                .filter(|p| !resource_paths(&src, &p.resource_name_str1).is_empty())
                .count();
            let total_icon = cfg
                .parts
                .iter()
                .filter(|p| icons.contains_key(&(p.texture_name.get())))
                .count();
            println!(
                "\n  resolved: {total_model}/{} models in the VFS, {total_icon}/{} icons in hash_name",
                cfg.parts.len(),
                cfg.parts.len()
            );
            if src.rubriques.is_empty() {
                println!("  sections: no label run found in the list script");
            } else {
                println!(
                    "\n  editor sections, in game order ({}):",
                    src.rubriques.len()
                );
                for (i, (h, libelle)) in src.rubriques.iter().enumerate() {
                    println!("   {:>2}. {libelle}  (0x{h:08X})", i + 1);
                }
            }
            if !src.shaders.is_empty() {
                let octets: u64 = src.shaders.iter().map(|(_, t)| *t).sum();
                println!(
                    "  shaders: {} `chr_edit_toon*` files ({:.1} KiB), identified by name, DXBC not disassembled",
                    src.shaders.len(),
                    octets as f64 / 1024.0
                );
            }
            let libelles = cfg
                .personalities
                .iter()
                .filter(|p| src.textes.contains_key(&p.view_text_id.get()))
                .count();
            println!(
                "  labels: {libelles}/{} personalities named by menu_text (fr)",
                cfg.personalities.len()
            );
        }

        AvatarCmd::Parts { category, limit } => {
            let sel: Vec<&nie_data::chara_edit::CharaEditPartsData> = match category {
                Some(c) => cfg.parts_of(*c).iter().collect(),
                None => cfg.parts.iter().collect(),
            };
            println!("{} part(s)", sel.len());
            for p in sel.iter().take(*limit) {
                let paths = resource_paths(&src, &p.resource_name_str1);
                println!(
                    "  {:>3} {:<24} {:<26} {}",
                    p.item_no,
                    if p.resource_name_str1.is_empty() {
                        p.resource_name1.to_hex_x8()
                    } else {
                        p.resource_name_str1.clone()
                    },
                    icons
                        .get(&(p.texture_name.get()))
                        .cloned()
                        .unwrap_or_else(|| p.texture_name.to_hex_x8()),
                    paths.first().map_or("(no model)", String::as_str)
                );
            }
            if sel.len() > *limit {
                println!("  … {} more (--limit)", sel.len() - *limit);
            }
        }

        AvatarCmd::Preset { id } => match id {
            None => {
                println!("{} preset face(s)", cfg.preset_info.len());
                for pi in &cfg.preset_info {
                    let nom = cfg
                        .parts
                        .iter()
                        .find(|p| p.resource_name1 == pi.preset_id)
                        .map(|p| p.resource_name_str1.clone())
                        .unwrap_or_default();
                    let morph: Vec<&str> = nie_data::chara_edit::BODY_TYPES
                        .iter()
                        .zip(pi.apply)
                        .filter_map(|(bt, ok)| ok.then_some(*bt))
                        .collect();
                    println!(
                        "  {} {:<20} {:>3} rows  body types: {}",
                        pi.preset_id.to_hex_x8(),
                        nom,
                        pi.data_count,
                        if morph.len() == 8 {
                            String::from("all")
                        } else {
                            morph.join(",")
                        }
                    );
                }
            }
            Some(arg) => {
                let hash = if let Some(hex) = arg.strip_prefix("0x") {
                    u32::from_str_radix(hex, 16).context("invalid preset hash")?
                } else {
                    cfgbin::crc32(arg.as_bytes())
                };
                let key = nie_data::hash::HashId::from_i64(i64::from(hash));
                let recipe = cfg.recipe_of(key);
                anyhow::ensure!(
                    !recipe.is_empty(),
                    "no recipe for {arg} (0x{hash:08X})"
                );
                println!("recipe {arg} (0x{hash:08X}) — {} rows", recipe.len());
                println!("   slot  cat  val    color  part");
                for line in recipe {
                    let r = cfg
                        .recipes
                        .iter()
                        .find(|r| r.recipe_type == line.recipe_type);
                    let part = cfg.part(line.parts_id);
                    println!(
                        "  {:>5}  {:>3}  {:>3}  {:>7}  {}",
                        line.recipe_type,
                        r.map_or(-1, |r| r.category),
                        line.recipe_no,
                        line.color_value,
                        part.map_or_else(
                            || if line.parts_id.is_zero() {
                                String::from("—")
                            } else {
                                line.parts_id.to_hex_x8()
                            },
                            |p| if p.resource_name_str1.is_empty() {
                                p.resource_name1.to_hex_x8()
                            } else {
                                p.resource_name_str1.clone()
                            }
                        )
                    );
                }
            }
        },

        AvatarCmd::Export { out } => {
            let categories: Vec<Json> = cfg
                .parts_info
                .iter()
                .map(|info| {
                    json!({
                        "faceSettingType": info.face_setting_type,
                        "prefixe": category_resource_prefix(cfg, info.face_setting_type),
                        "parts": cfg.parts_of(info.face_setting_type).iter()
                            .map(|p| part_json(&src, &icons, &trans, &scales, p))
                            .collect::<Vec<_>>(),
                        "couleurs": cfg.colors_of(info.face_setting_type).iter()
                            .map(|h| h.to_hex_x8()).collect::<Vec<_>>(),
                        // Palette grid, cross-checked against the game screen names.
                        "ecran": ecran_de_categorie(&category_resource_prefix(cfg, info.face_setting_type), &src.ecrans),
                        "grillePalette": geometrie_palette(cfg.colors_of(info.face_setting_type).len())
                            .map(|(c, l)| json!({ "colonnes": c, "lignes": l })),
                    })
                })
                .collect();

            let presets: Vec<Json> = cfg
                .preset_info
                .iter()
                .map(|pi| {
                    json!({
                        "presetID": pi.preset_id.to_hex_x8(),
                        "nom": cfg.parts.iter().find(|p| p.resource_name1 == pi.preset_id)
                            .map(|p| p.resource_name_str1.clone()),
                        "morphologies": nie_data::chara_edit::BODY_TYPES.iter().zip(pi.apply)
                            .filter_map(|(bt, ok)| ok.then_some(*bt)).collect::<Vec<_>>(),
                        "recette": cfg.recipe_of(pi.preset_id).iter().map(|l| json!({
                            "emplacement": l.recipe_type,
                            "valeur": l.recipe_no,
                            "couleur": l.color_value,
                            "part": cfg.part(l.parts_id).map(|p| p.resource_name_str1.clone()),
                            "partId": (!l.parts_id.is_zero()).then(|| l.parts_id.to_hex_x8()),
                        })).collect::<Vec<_>>(),
                    })
                })
                .collect();

            let doc = json!({
                "source": "chara_edit + chara_edit_parts_type_config + 20_EDIT/{center,texPartsDefaultPose,editCharaMdlParts}",
                // Where each sheet sits on the face square, as a texture fraction.
                // Keyed by SHEET name, which is what the table hashes, not a part
                // `resourceName`. See `centres_de_planches`.
                "centresDePlanches": centres.iter().map(|(nom, r)| json!({
                    "planche": nom,
                    "u": r.first(), "v": r.get(1), "largeur": r.get(2), "hauteur": r.get(3),
                })).collect::<Vec<_>>(),
                "codePartage": {
                    "bits": cfg.code_bit_width(),
                    "alphabet": cfg.codes.iter().map(|c| c.code_char.clone()).collect::<Vec<_>>(),
                    "emplacements": cfg.recipes.iter().map(|r| json!({
                        "emplacement": r.recipe_type, "bits": r.bit_num, "valeurs": r.num,
                        "categorie": r.category, "param": r.category_param, "paramSub": r.category_param_sub,
                    })).collect::<Vec<_>>(),
                },
                // Order and labels are established; the link to a `faceSettingType` is not.
                "shaders": src.shaders.iter().map(|(nom, taille)| json!({
                    "fichier": nom, "octets": taille,
                })).collect::<Vec<_>>(),
                "rubriques": src.rubriques.iter().map(|(h, libelle)| json!({
                    "hash": format!("{h:08X}"), "libelle": libelle,
                })).collect::<Vec<_>>(),
                // The statistics radar axes, in drawing order.
                "statsRadar": STATS_RADAR.iter().map(|h| json!({
                    "hash": format!("{h:08X}"),
                    "libelle": src.textes.get(h).cloned().unwrap_or_default(),
                })).collect::<Vec<_>>(),
                // The labels each panel shows, read from its script: this is the source
                // of section and slider titles, none is written by hand.
                "panneaux": src.panneaux.iter().map(|p| json!({
                    "nom": p.nom,
                    "libelles": p.libelles.iter().map(|(h, l, g)| json!({
                        "hash": format!("{h:08X}"), "libelle": l, "gaiji": g,
                    })).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
                "categories": categories,
                "presets": presets,
                "avatarsFichiers": cfg.preset_files.iter().map(|f| json!({
                    "nom": f.id_string, "viewNo": f.view_no,
                    "charaId": f.chara_id.to_hex_x8(),
                    "modeles": resource_paths(&src, &f.id_string),
                    "libelle": src.textes.get(&f.view_text_id.get()),
                })).collect::<Vec<_>>(),
                "voix": cfg.voices.iter().map(|v| json!({
                    "banque": v.chara_se_name, "genre": v.gender,
                    "personnalite": v.personality, "ton": v.voice_type, "itemNo": v.item_no,
                })).collect::<Vec<_>>(),
                "personnalites": cfg.personalities.iter().map(|p| json!({
                    "type": p.personality_type, "presentation": p.performance_type,
                    "texte": p.view_text_id.to_hex_x8(),
                    "libelle": src.textes.get(&p.view_text_id.get()),
                })).collect::<Vec<_>>(),
                "tenues": cfg.fashions.iter().map(|f| json!({
                    "id": f.id, "hash": f.fashion_name_crc.to_hex_x8(),
                    "morphologies": cfg.body_types_of_fashion(f.fashion_name_crc),
                })).collect::<Vec<_>>(),
                "curseurs": cfg.params_info.iter().map(|gi| json!({
                    "partsType": gi.parts_type,
                    "params": cfg.params_of(gi.parts_type).iter().map(|p| json!({
                        "part": p.parts_id.to_hex_x8(), "axe": p.param_type,
                        "defaut": p.param_default, "min": p.param_min, "max": p.param_max,
                        "morphologies": nie_data::chara_edit::BODY_TYPES.iter().zip(p.apply)
                            .filter_map(|(bt, ok)| ok.then_some(*bt)).collect::<Vec<_>>(),
                    })).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
                "modelesDeBase": {
                    "morphologies": nie_data::chara_edit::BODY_TYPES,
                    "visages": src.types.face_data.iter().map(|f| json!({
                        "noseType": f.nose_type, "resources": f.resource,
                    })).collect::<Vec<_>>(),
                    "accessoires": src.types.body_data.iter().map(|b| json!({
                        "presetID": b.preset_id.to_hex_x8(), "resources": b.resource,
                    })).collect::<Vec<_>>(),
                },
                "reglesModele": src.rules,
            });

            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            std::fs::write(out, serde_json::to_vec_pretty(&doc)?)
                .with_context(|| format!("writing {}", out.display()))?;
            println!(
                "wrote {} — {} categories, {} parts, {} presets",
                out.display(),
                cfg.parts_info.len(),
                cfg.parts.len(),
                cfg.preset_info.len()
            );
        }

        AvatarCmd::Roi {
            ecran,
            layouts,
            out,
        } => return roi(ecran, layouts, out.as_deref()),

        // Already handled at the top, before the sources are loaded.
        AvatarCmd::Planches { .. } | AvatarCmd::Depliage { .. } => {}

        AvatarCmd::Icons {
            out,
            atlas_prefix,
            limit,
        } => {
            // The wanted names: the icon of each part, resolved from `hash_name`.
            let voulus: BTreeMap<u32, String> = cfg
                .parts
                .iter()
                .filter_map(|p| {
                    let h = p.texture_name.get();
                    icons.get(&h).map(|n| (h, n.clone()))
                })
                .collect();
            println!("{} named icon(s) to locate", voulus.len());

            let mut vfs = Vfs::new();
            vfs.init(src.game_dir.join("data")).context("init VFS")?;
            let localisees = index_atlases(&vfs, atlas_prefix, &voulus);
            println!(
                "  {}/{} located in atlases under {atlas_prefix}",
                localisees.len(),
                voulus.len()
            );
            let manquantes: Vec<&String> = voulus
                .values()
                .filter(|n| !localisees.contains_key(n.as_str()))
                .collect();
            if !manquantes.is_empty() {
                println!(
                    "  {} not located, e.g. {}",
                    manquantes.len(),
                    manquantes
                        .iter()
                        .take(5)
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }

            let Some(dir) = out else {
                for (nom, loc) in localisees.iter().take(20) {
                    println!(
                        "  {nom:<26} {} [{}]{}",
                        loc.texture,
                        loc.atlas.rsplit('/').next().unwrap_or(&loc.atlas),
                        loc.rect
                            .map_or(String::new(), |(x, y, w, h)| format!(" {x},{y} {w}×{h}"))
                    );
                }
                return Ok(());
            };
            std::fs::create_dir_all(dir)?;
            // An atlas is decoded only once, whatever the number of icons.
            let mut cache: BTreeMap<(String, String), Option<Rgba>> = BTreeMap::new();
            let (mut ecrits, mut echecs) = (0usize, 0usize);
            for (nom, loc) in localisees.iter().take(*limit) {
                let key = (loc.atlas.clone(), loc.texture.clone());
                if !cache.contains_key(&key) {
                    let decoded = vfs.read(&loc.atlas).ok().and_then(|raw| {
                        nie_formats::g4tx_decode::decode_named_to_rgba(&raw, &loc.texture)
                    });
                    cache.insert(key.clone(), decoded);
                }
                let Some(Some((w, h, rgba))) = cache.get(&key) else {
                    echecs += 1;
                    continue;
                };
                let png = match loc.rect {
                    Some(rect) => crop_rgba(rgba, *w, *h, rect).and_then(|(cw, ch, buf)| {
                        nie_formats::g4tx_decode::encode_rgba_to_png(&buf, cw as usize, ch as usize)
                    }),
                    None => {
                        nie_formats::g4tx_decode::encode_rgba_to_png(rgba, *w as usize, *h as usize)
                    }
                };
                match png {
                    Some(bytes) => {
                        std::fs::write(dir.join(format!("{nom}.png")), bytes)?;
                        ecrits += 1;
                    }
                    None => echecs += 1,
                }
            }
            println!(
                "  {ecrits} PNG written to {}, {echecs} failure(s)",
                dir.display()
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod adapter_tests {
    use nie_data::avatar::numeric_table_by_hash;
    use serde_json::json;

    #[test]
    fn avatar_adapter_uses_shared_t2b_table_contract() {
        let converted = json!({
            "name": "TEX_PARTS_DEFAULT_SCALE_INFO_12",
            "variables": [{"value": 42}, {"value": 1.25}]
        });
        assert_eq!(
            numeric_table_by_hash(&converted, "TEX_PARTS_DEFAULT_SCALE_INFO").get(&42),
            Some(&vec![1.25])
        );
    }
}
