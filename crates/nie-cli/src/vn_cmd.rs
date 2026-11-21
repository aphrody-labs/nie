//! `nie vn` - feed a visual novel project with the real game assets.
//!
//! The VN (`nie-vn-engine`, a Ren'Py fork) ships no asset: it reads a **catalogue** produced here,
//! on the user's machine, from their own game install. Nothing this command writes is meant for
//! a repository; the output goes to a folder the Ren'Py project ignores.
//!
//! Two operations:
//!
//! * [`VnCmd::Casting`]: list which characters are **voiced**, by decreasing bank size (main
//!   roles have the most recorded lines).
//! * [`VnCmd::Export`]: extract voices (HCA to WAV), expression atlases (G4TX to PNG) and music,
//!   then write `catalogue.json`, the only interface the Ren'Py side knows.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use nie_explore::audio;
use nie_explore::vn::{
    CastingOptions, CharacterIdentity, VoiceBank, casting_entries, discover_voice_banks,
};
use nie_explore::vn::{
    ExportOptions, capitalize_name as capitaliser, catalogue as construire_catalogue,
    collect_texts as collecter_textes, dialogue_references, identities_from_tagged, plan_export,
    sanitize_filename as assainir, t2b_to_tagged as t2b_vers_tagged,
    usable_dialogue as replique_utilisable,
};
use nie_formats::vfs::Vfs;

/// `nie vn` subcommands.
#[derive(clap::Subcommand, Debug)]
pub enum VnCmd {
    /// List voiced characters, largest bank first.
    Casting {
        /// Voice language (`ja`, `en`, ...).
        #[arg(long, default_value = "ja")]
        langue: String,
        /// Number of characters shown.
        #[arg(long, default_value_t = 30)]
        limit: usize,
        /// JSON output instead of a table.
        #[arg(long)]
        json: bool,
        /// Keep only characters whose name contains this pattern (accent- and case-insensitive).
        #[arg(long)]
        chercher: Option<String>,
        /// Language of displayed names (`fr`, `en`, `ja`, ...).
        #[arg(long, default_value = "fr")]
        langue_noms: String,
        /// Keep one gender: `m` or `f`.
        #[arg(long)]
        genre: Option<String>,
    },

    /// Extract voices, portraits and music into a folder the Ren'Py project consumes.
    Export {
        /// Output folder, typically `<renpy-project>/game/nie`.
        #[arg(long)]
        out: PathBuf,
        /// Internal codes to extract (`c01000010,c01000020`). Default: the best voiced.
        #[arg(long, value_delimiter = ',')]
        casting: Vec<String>,
        /// Names to extract, resolved against the local master table (`--noms "Kazemaru,Byron"`).
        #[arg(long, value_delimiter = ',')]
        noms: Vec<String>,
        /// Language of resolved names (`fr`, `en`, `ja`, ...).
        #[arg(long, default_value = "fr")]
        langue_noms: String,
        /// Voice language.
        #[arg(long, default_value = "ja")]
        langue: String,
        /// Maximum lines extracted per character.
        #[arg(long, default_value_t = 24)]
        voix_max: usize,
        /// Maximum tracks extracted from `bgm.acb` (0 = none).
        #[arg(long, default_value_t = 6)]
        bgm_max: usize,
        /// Maximum textures extracted per character.
        #[arg(long, default_value_t = 4)]
        textures_max: usize,
        /// Maximum event lines collected per character (0 = none).
        #[arg(long, default_value_t = 60)]
        dialogues_max: usize,
        /// Event line language (`fr`, `en`, `ja`, ...).
        #[arg(long, default_value = "fr")]
        langue_dialogues: String,
    },
}

/// Entry point of `nie vn`.
pub fn run(cmd: &VnCmd, vfs: &Vfs) -> Result<()> {
    match cmd {
        VnCmd::Casting {
            langue,
            limit,
            json,
            chercher,
            langue_noms,
            genre,
        } => casting(
            vfs,
            langue,
            *limit,
            *json,
            chercher.as_deref(),
            langue_noms,
            genre.as_deref(),
        ),
        VnCmd::Export {
            out,
            casting: codes,
            noms,
            langue_noms,
            langue,
            voix_max,
            bgm_max,
            textures_max,
            dialogues_max,
            langue_dialogues,
        } => export(
            vfs,
            out,
            codes,
            noms,
            langue_noms,
            langue,
            *voix_max,
            *bgm_max,
            *textures_max,
            *dialogues_max,
            langue_dialogues,
        ),
    }
}

// ---------------------------------------------------------------------------
// Identities: internal code <-> name, read from the local install
// ---------------------------------------------------------------------------

/// Read a T2B `cfg.bin` from the VFS and return it in `tagged` form.
fn lire_t2b(vfs: &Vfs, chemin: &str) -> Option<serde_json::Value> {
    let raw = vfs.read(chemin).ok()?;
    if nie_formats::cfgbin::is_rdbn(&raw) {
        return None;
    }
    let cfg = nie_formats::cfgbin::cfgbin_parse(&raw).ok()?;
    Some(serde_json::json!({ "entries": t2b_vers_tagged(&cfg.entries) }))
}

/// First VFS path starting with `prefixe` and ending with `suffixe`.
fn chercher_chemin(vfs: &Vfs, prefixe: &str, suffixe: &str) -> Option<String> {
    let mut trouves: Vec<String> = vfs
        .iter()
        .filter(|(p, _)| p.starts_with(prefixe) && p.ends_with(suffixe))
        .map(|(p, _)| p.to_string())
        .collect();
    trouves.sort();
    trouves.pop()
}

/// Build the identity index from the master table and the localized texts.
///
/// Returns an empty list, without error, when the install lacks those tables: the pipeline
/// must stay usable with internal codes alone.
fn identites(vfs: &Vfs, langue_noms: &str) -> Vec<CharacterIdentity> {
    let Some(chemin_base) = chercher_chemin(
        vfs,
        "data/common/gamedata/character/chara_base_",
        ".cfg.bin",
    ) else {
        return Vec::new();
    };
    let chemin_texte = format!("data/common/text/{langue_noms}/chara_text.cfg.bin");

    let (Some(base_json), Some(texte_json)) =
        (lire_t2b(vfs, &chemin_base), lire_t2b(vfs, &chemin_texte))
    else {
        return Vec::new();
    };

    identities_from_tagged(&base_json, &texte_json)
}

// ---------------------------------------------------------------------------
// Bank discovery
// ---------------------------------------------------------------------------

/// All `sound_asset/<langue>/c########.acb` banks, sorted by decreasing weight.
fn banques(vfs: &Vfs, langue: &str) -> Vec<VoiceBank> {
    discover_voice_banks(
        vfs.iter()
            .map(|(path, entry)| (path.to_owned(), u64::from(entry.file_size))),
        langue,
    )
}

fn casting(
    vfs: &Vfs,
    langue: &str,
    limit: usize,
    json: bool,
    chercher: Option<&str>,
    langue_noms: &str,
    genre: Option<&str>,
) -> Result<()> {
    let banques = banques(vfs, langue);
    if banques.is_empty() {
        anyhow::bail!("no voice bank under data/common/sound_asset/{langue}/");
    }

    let index: BTreeMap<String, CharacterIdentity> = identites(vfs, langue_noms)
        .into_iter()
        .map(|f| (f.code.clone(), f))
        .collect();

    let retenues = casting_entries(
        banques.iter().cloned(),
        index.values().cloned(),
        &CastingOptions {
            limit,
            search: chercher.map(str::to_owned),
            gender: genre.map(str::to_owned),
        },
    );

    if json {
        let liste: Vec<_> = retenues
            .iter()
            .map(|b| {
                serde_json::json!({
                    "code": b.code,
                    "nom": b.name,
                    "genre": b.gender,
                    "acb": b.acb,
                    "awb_octets": b.awb_bytes,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&liste)?);
        return Ok(());
    }

    println!(
        "{} voice bank(s) in '{langue}', {} shown\n",
        banques.len(),
        retenues.len()
    );
    for b in retenues {
        let nom = b.name.as_deref().unwrap_or("-");
        let genre = b.gender.as_deref().unwrap_or(" ");
        println!("  {:>10}  {genre}  {:>12} B  {nom}", b.code, b.awb_bytes);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Event lines
// ---------------------------------------------------------------------------

/// Collect, for each target code, the lines the game makes that character say.
///
/// The link between a character and its text rests on two rules:
///
/// * in `event_cfg/evt/<ev>.cfg.bin`, the command with opcode [`OP_REPLIQUE`] carries the line
///   id (`ev01_01200_010_010`) and, as its last string, the internal code of the speaker;
/// * in `text/<langue>/event/<ev>.cfg.bin`, the line is indexed by the `crc32` of that id.
///
/// Scripts are walked once for the whole cast. An event without a text table in the requested
/// language is skipped silently: not every language covers every event.
fn relever_dialogues(
    vfs: &Vfs,
    codes: &BTreeSet<String>,
    langue: &str,
    max: usize,
) -> BTreeMap<String, Vec<serde_json::Value>> {
    use nie_formats::cfgbin;

    let mut scripts: Vec<String> = vfs
        .iter()
        .filter(|(p, _)| p.starts_with("data/common/event_cfg/evt/") && p.ends_with(".cfg.bin"))
        .map(|(p, _)| p.to_string())
        .collect();
    scripts.sort();

    let balises = table_des_balises(vfs, langue);
    let mut out: BTreeMap<String, Vec<serde_json::Value>> = BTreeMap::new();

    for chemin in &scripts {
        if !out.is_empty()
            && codes
                .iter()
                .all(|c| out.get(c).is_some_and(|v| v.len() >= max))
        {
            break;
        }
        let Some(evenement) = chemin
            .rsplit('/')
            .next()
            .and_then(|f| f.strip_suffix(".cfg.bin"))
        else {
            continue;
        };
        let Ok(raw) = vfs.read(chemin) else { continue };
        let Ok(cfg) = cfgbin::cfgbin_parse(&raw) else {
            continue;
        };

        let attendues = dialogue_references(&cfg.entries, codes);
        if attendues.is_empty() {
            continue;
        }

        let chemin_texte = format!("data/common/text/{langue}/event/{evenement}.cfg.bin");
        let Ok(brut_texte) = vfs.read(&chemin_texte) else {
            continue;
        };
        let Ok(table) = cfgbin::cfgbin_parse(&brut_texte) else {
            continue;
        };
        let mut textes: BTreeMap<u32, String> = BTreeMap::new();
        collecter_textes(&table.entries, &mut textes);

        for (code, ligne) in attendues {
            let seau = out.entry(code).or_default();
            if seau.len() >= max {
                continue;
            }
            let cle = cfgbin::crc32(ligne.as_bytes());
            let Some(texte) = textes.get(&cle) else {
                continue;
            };
            let Some(propre) = replique_utilisable(texte, &balises) else {
                continue;
            };
            seau.push(serde_json::json!({
                "evenement": evenement,
                "ligne": ligne,
                "texte": propre,
            }));
        }
    }

    out
}

/// Table of name tags used in event text.
///
/// The game does not write names in clear: it places `<FLC:YASHIMA>`, `<FUL:RAIKA>`, where only
/// the segment after `:` identifies the character. That segment is the upper-case romanized form
/// of `chara_text_roma`; the displayable name is read under the same hash in `chara_text` of the
/// requested language.
///
/// An unresolved tag is left as is rather than removed: a visible `<FLC:XXX>` beats a sentence
/// stripped of its subject.
fn table_des_balises(vfs: &Vfs, langue: &str) -> BTreeMap<String, String> {
    use nie_formats::cfgbin;

    let lire = |nom: &str| -> BTreeMap<u32, String> {
        let mut out = BTreeMap::new();
        if let Ok(brut) = vfs.read(&format!("data/common/text/{langue}/{nom}.cfg.bin"))
            && let Ok(table) = cfgbin::cfgbin_parse(&brut)
        {
            collecter_textes(&table.entries, &mut out);
        }
        out
    };

    let roma = lire("chara_text_roma");
    let affichables = lire("chara_text");

    let mut out = BTreeMap::new();
    for (hash, forme) in &roma {
        // Only all-uppercase forms serve as tags.
        if forme.is_empty()
            || !forme
                .chars()
                .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
        {
            continue;
        }
        if let Some(nom) = affichables.get(hash) {
            out.entry(forme.clone()).or_insert_with(|| capitaliser(nom));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn export(
    vfs: &Vfs,
    out: &Path,
    codes: &[String],
    noms: &[String],
    langue_noms: &str,
    langue: &str,
    voix_max: usize,
    bgm_max: usize,
    textures_max: usize,
    dialogues_max: usize,
    langue_dialogues: &str,
) -> Result<()> {
    let toutes = banques(vfs, langue);
    if toutes.is_empty() {
        anyhow::bail!("no voice bank under data/common/sound_asset/{langue}/");
    }

    let index: BTreeMap<String, CharacterIdentity> = identites(vfs, langue_noms)
        .into_iter()
        .map(|f| (f.code.clone(), f))
        .collect();

    let plan = plan_export(
        &toutes,
        &index.values().cloned().collect::<Vec<_>>(),
        &ExportOptions {
            codes: codes.to_vec(),
            names: noms.to_vec(),
            language: langue.to_owned(),
        },
    )
    .map_err(anyhow::Error::msg)?;
    for warning in plan.warnings {
        eprintln!("  {warning}");
    }
    let retenues = plan.banks;

    std::fs::create_dir_all(out).with_context(|| format!("create {}", out.display()))?;

    // Lines are collected in a single pass over the event scripts: every file is walked once for
    // the whole cast, not once per character.
    let vises: BTreeSet<String> = retenues.iter().map(|b| b.code.clone()).collect();
    let mut dialogues = if dialogues_max > 0 {
        relever_dialogues(vfs, &vises, langue_dialogues, dialogues_max)
    } else {
        BTreeMap::new()
    };

    let mut personnages = Vec::new();
    for banque in &retenues {
        let voix = exporter_voix(vfs, out, banque, voix_max)?;
        let textures = exporter_textures(vfs, out, &banque.code, textures_max);
        let repliques = dialogues.remove(&banque.code).unwrap_or_default();
        let fiche = index.get(&banque.code);
        eprintln!(
            "  {} {} - {} voice(s), {} texture(s), {} line(s) written",
            banque.code,
            fiche.map_or(String::new(), |f| format!("({})", f.name)),
            voix.len(),
            textures.len(),
            repliques.len()
        );
        personnages.push(serde_json::json!({
            "code": banque.code,
            "nom": fiche.map(|f| f.name.clone()),
            "genre": fiche.map(|f| f.gender.clone()),
            "voix": voix,
            "textures": textures,
            "dialogues": repliques,
        }));
    }

    let musique = if bgm_max > 0 {
        exporter_bgm(vfs, out, bgm_max)?
    } else {
        Vec::new()
    };

    let catalogue = construire_catalogue(langue, personnages, musique.clone());
    let chemin = out.join("catalogue.json");
    std::fs::write(&chemin, serde_json::to_vec_pretty(&catalogue)?)
        .with_context(|| format!("write {}", chemin.display()))?;

    println!(
        "catalogue  {} character(s), {} music track(s) -> {}",
        retenues.len(),
        musique.len(),
        chemin.display()
    );
    Ok(())
}

/// Decode up to `max` lines of a bank and return their JSON descriptors.
fn exporter_voix(
    vfs: &Vfs,
    out: &Path,
    banque: &VoiceBank,
    max: usize,
) -> Result<Vec<serde_json::Value>> {
    if banque.acb.is_empty() {
        return Ok(Vec::new());
    }
    let raw = vfs
        .read(&banque.acb)
        .with_context(|| format!("read {}", banque.acb))?;
    let Some((awb, _source)) = audio::resoudre_awb(vfs, &banque.acb, &raw) else {
        eprintln!("  {} - byte bank not found, skipped", banque.code);
        return Ok(Vec::new());
    };

    let dossier = out.join("voix").join(&banque.code);
    std::fs::create_dir_all(&dossier)?;

    let mut sortie = Vec::new();
    for cue in audio::cues(&raw, Some(&awb)) {
        if sortie.len() >= max {
            break;
        }
        let Some(id) = cue.awb_id else { continue };
        let Ok(wav) = audio::decoder_cue(&awb, id) else {
            continue;
        };
        let nom = audio::nom_de_fichier(&banque.acb, &cue);
        std::fs::write(dossier.join(&nom), wav)?;
        sortie.push(serde_json::json!({
            "cue": cue.name,
            "fichier": format!("voix/{}/{}", banque.code, nom),
            "ms": cue.length_ms,
        }));
    }
    Ok(sortie)
}

/// Extract the character textures (expression atlas and model sheets) as PNG.
fn exporter_textures(vfs: &Vfs, out: &Path, code: &str, max: usize) -> Vec<serde_json::Value> {
    let mut chemins: Vec<String> = vfs
        .iter()
        .filter(|(path, _)| path.ends_with(".g4tx") && path.contains(code))
        .map(|(path, _)| path.to_string())
        .collect();
    // `_face` first: it is the expression atlas, the most useful for a VN.
    chemins.sort_by_key(|p| (!p.contains("/_face/"), p.clone()));

    let dossier = out.join("images").join(code);
    if std::fs::create_dir_all(&dossier).is_err() {
        return Vec::new();
    }

    let mut sortie = Vec::new();
    for chemin in chemins {
        if sortie.len() >= max {
            break;
        }
        let Ok(raw) = vfs.read(&chemin) else { continue };
        let base = nie_formats::g4tx_decode::basename_of(&chemin);
        let Some(png) = nie_formats::g4tx_decode::decode_best_to_png(&raw, base) else {
            continue;
        };
        let nom = format!("{}.png", assainir(base));
        if std::fs::write(dossier.join(&nom), png).is_err() {
            continue;
        }
        sortie.push(serde_json::json!({
            "source": chemin,
            "fichier": format!("images/{code}/{nom}"),
            "role": if chemin.contains("/_face/") { "expressions" } else { "planche" },
        }));
    }
    sortie
}

/// Decode up to `max` tracks of `bgm.acb`.
fn exporter_bgm(vfs: &Vfs, out: &Path, max: usize) -> Result<Vec<serde_json::Value>> {
    const BGM: &str = "data/common/sound_asset/bgm.acb";
    let Ok(raw) = vfs.read(BGM) else {
        eprintln!("  bgm.acb missing, music skipped");
        return Ok(Vec::new());
    };
    let Some((awb, _)) = audio::resoudre_awb(vfs, BGM, &raw) else {
        return Ok(Vec::new());
    };

    let dossier = out.join("musique");
    std::fs::create_dir_all(&dossier)?;

    let mut sortie = Vec::new();
    for cue in audio::cues(&raw, Some(&awb)) {
        if sortie.len() >= max {
            break;
        }
        let Some(id) = cue.awb_id else { continue };
        let Ok(wav) = audio::decoder_cue(&awb, id) else {
            continue;
        };
        let nom = audio::nom_de_fichier(BGM, &cue);
        std::fs::write(dossier.join(&nom), wav)?;
        sortie.push(serde_json::json!({
            "cue": cue.name,
            "fichier": format!("musique/{nom}"),
            "ms": cue.length_ms,
        }));
    }
    eprintln!("  music - {} track(s)", sortie.len());
    Ok(sortie)
}

#[cfg(test)]
mod tests {
    use super::assainir;
    use nie_explore::vn::{CastingOptions, CharacterIdentity, VoiceBank, casting_entries};

    #[test]
    fn assainir_remplace_les_separateurs() {
        assert_eq!(assainir("c01/00.0010"), "c01_00_0010");
        assert_eq!(assainir("ev74_00840_me"), "ev74_00840_me");
    }

    #[test]
    fn casting_adapter_preserves_cli_json_field_names() {
        let entries = casting_entries(
            [VoiceBank {
                code: "c1".into(),
                acb: "c1.acb".into(),
                awb_bytes: 7,
            }],
            [CharacterIdentity {
                code: "c1".into(),
                name: "Mark".into(),
                gender: "m".into(),
            }],
            &CastingOptions {
                limit: 1,
                search: None,
                gender: None,
            },
        );
        let value = serde_json::json!({
            "code": entries[0].code,
            "nom": entries[0].name,
            "genre": entries[0].gender,
            "acb": entries[0].acb,
            "awb_octets": entries[0].awb_bytes,
        });
        assert_eq!(value["nom"], "Mark");
        assert_eq!(value["awb_octets"], 7);
    }
}
