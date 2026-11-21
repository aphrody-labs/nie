//! `nie mode` - catalogue of game modes: screens, layers, objects, assets and scripts.
//!
//! ## What a "mode" is here
//!
//! A main-menu entry (a tile of `mode_base01_atl`, see `mainmenu90_01.g4tx`). The game stores the
//! list nowhere in clear: the `main_menu` script names its tabs by an **integer** `TAB_TYPE`, and
//! the include carries no usable string. The mode list is therefore editorial ([`MODES`]), but
//! each entry is backed by `*_setting.cfg.bin` screens that exist in the VFS; the aggregation is
//! mechanical.
//!
//! `victory_road` shows why a curated list is needed: its assets live under four spellings
//! (`victory_road`, `victory_load`, `victory_lode`, `vroad`) that no prefix rule would link.
//!
//! ## Aggregation
//!
//! Per mode: screens (by prefix) -> layers (`MENU_LAYER_INFO`) -> objbin -> referenced `g4pkm` /
//! `g4tx`, plus Lua scripts with the same prefix. Everything is written to `mode`, `mode_screen`
//! and `mode_asset`.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};
use nie_explore::menu_mode_analysis::{MenuPaths, analyze_mode_script, collect_mode};
use nie_formats::cfgbin::{self, CfgEntry, Value};
use nie_formats::objbin;
use nie_formats::vfs::Vfs;
use serde_json::Value as Json;

pub use nie_explore::menu_modes::MODES;
use nie_explore::menu_modes::{ModeDef, matches_stem};

/// Modes a screen belongs to, by its real prefixes.
///
/// An empty list is a useful result: the catalogue also holds shared or utility screens that are
/// not mode tiles. Several results signal an editorial overlap that must stay visible in the audit.
fn classify_screen(stem: &str) -> Vec<&'static str> {
    MODES
        .iter()
        .filter(|def| matches_stem(def, stem))
        .map(|def| def.slug)
        .collect()
}

/// Audit how each `*_menu_setting.cfg.bin` definition in the VFS maps to modes.
///
/// The renderer and `mode index` consume the same files, but their outputs could not be checked
/// together for coverage. This output is independent of SQLite and can be compared directly with
/// the `--menu-matrix` JSON of `nie-game` without rebuilding an intermediate catalogue.
pub fn menu_coverage_json(vfs: &Vfs) -> Json {
    let mut screens: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (path, _) in vfs.iter() {
        if !path.starts_with("data/common/gamedata/menu/cfg/")
            || !path.ends_with("_setting.cfg.bin")
        {
            continue;
        }
        let Some(stem) = path
            .rsplit('/')
            .next()
            .and_then(|file| file.strip_suffix("_setting.cfg.bin"))
        else {
            continue;
        };
        screens
            .entry(stem.to_string())
            .or_default()
            .push(path.to_string());
    }

    let mut mode_counts: BTreeMap<&str, usize> = MODES.iter().map(|def| (def.slug, 0)).collect();
    let mut classified = 0usize;
    let mut overlapping = Vec::new();
    let mut unclassified = Vec::new();
    let mut duplicates = Vec::new();

    for (screen, paths) in &screens {
        if paths.len() > 1 {
            duplicates.push(serde_json::json!({
                "screen": screen,
                "paths": paths,
            }));
        }
        let modes = classify_screen(screen);
        if modes.is_empty() {
            unclassified.push(serde_json::json!({
                "screen": screen,
                "paths": paths,
            }));
        } else {
            classified += 1;
            for mode in &modes {
                *mode_counts
                    .get_mut(mode)
                    .expect("classify_screen only returns MODES slugs") += 1;
            }
            if modes.len() > 1 {
                overlapping.push(serde_json::json!({
                    "screen": screen,
                    "modes": modes,
                    "paths": paths,
                }));
            }
        }
    }

    let modes = MODES
        .iter()
        .map(|def| {
            serde_json::json!({
                "slug": def.slug,
                "official": def.official,
                "screens": mode_counts[def.slug],
            })
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "schema": "nie.menu.coverage/v1",
        "settings": {
            "unique": screens.len(),
            "classified": classified,
            "unclassified": unclassified.len(),
            "overlapping": overlapping.len(),
            "duplicateStems": duplicates.len(),
        },
        "modes": modes,
        "unclassifiedScreens": unclassified,
        "overlappingScreens": overlapping,
        "duplicateScreens": duplicates,
    })
}

/// Locales whose official label is resolved.
const LOCALES: [&str; 3] = ["fr", "en", "ja"];

/// Load `menu_text` of a locale: `hash` -> label.
///
/// The file holds `TEXT_INFO_BEGIN > TEXT_INFO [hash, 0, text]`. A missing file means the locale
/// is not installed, which is not an error: the catalogue falls back to the default label.
fn charger_menu_text(vfs: &Vfs, locale: &str) -> BTreeMap<u32, String> {
    let mut out = BTreeMap::new();
    let path = format!("data/common/text/{locale}/menu_text.cfg.bin");
    let Ok(bytes) = vfs.read(&path) else {
        return out;
    };
    let Ok(file) = cfgbin::parse_t2b(&bytes) else {
        return out;
    };
    walk(&file.entries, &mut |e: &CfgEntry| {
        if !e.name.starts_with("TEXT_INFO") || e.name.contains("BEGIN") || e.name.contains("END") {
            return;
        }
        let hash = e.variables.iter().find_map(|v| match v {
            Value::Int(i) => Some(u32::from_ne_bytes(i.to_ne_bytes())),
            _ => None,
        });
        let texte = e.variables.iter().rev().find_map(|v| match v {
            Value::String(s) if !s.is_empty() => Some(s.clone()),
            _ => None,
        });
        if let (Some(h), Some(t)) = (hash, texte) {
            out.insert(h, t);
        }
    });
    out
}

/// What was found for a mode.
#[derive(Default)]
pub struct ModeFacts {
    /// `*_setting.cfg.bin` screens (stem -> VFS path).
    pub screens: BTreeMap<String, String>,
    /// Layers declared by those screens.
    pub layers: BTreeSet<String>,
    /// Objbin resolved from the layers.
    pub objbins: BTreeSet<String>,
    /// `g4pkm` referenced by those objbin.
    pub g4pkm: BTreeSet<String>,
    /// `g4tx` referenced (SETUP or component parameter).
    pub g4tx: BTreeSet<String>,
    /// Component types met (RTTI class names).
    pub components: BTreeSet<String>,
    /// Lua scripts with the same prefix.
    pub lua: BTreeSet<String>,
    /// Total focusable element count.
    pub focus: usize,
    /// Text slots of `MenuTextSetting` components: `(object, slot, hash)`.
    ///
    /// The hash resolves in `menu_text`; many point to button guides (`<CMD_BACK|10>`), which is data
    /// in itself: the screen UI.
    pub text_slots: BTreeSet<(String, String, u32)>,
}

fn first_string(e: &CfgEntry) -> Option<&str> {
    e.variables.iter().find_map(|v| match v {
        Value::String(s) if !s.is_empty() => Some(s.as_str()),
        _ => None,
    })
}

fn walk<'a>(entries: &'a [CfgEntry], f: &mut impl FnMut(&'a CfgEntry)) {
    for e in entries {
        f(e);
        walk(&e.children, f);
    }
}

/// Gather the facts of a mode from the VFS.
pub fn collect(vfs: &Vfs, def: &ModeDef) -> ModeFacts {
    let paths = MenuPaths::from_paths(vfs.iter().map(|(path, _)| path));
    let shared = collect_mode(vfs, &paths, def, &BTreeMap::new());
    ModeFacts {
        screens: shared
            .screens
            .into_iter()
            .map(|screen| (screen.screen, screen.cfg))
            .collect(),
        layers: shared.layers,
        objbins: shared.objbins,
        g4pkm: shared.g4pkm,
        g4tx: shared.g4tx,
        components: shared.components.into_keys().collect(),
        lua: shared
            .scripts
            .into_iter()
            .map(|script| script.path)
            .collect(),
        focus: shared.focus,
        text_slots: shared.text_slots,
    }
}

/// `cmdId -> handler VA` table from `data/re/funclua-cmdid-handlers.json`, or empty if absent.
///
/// Walks up from the current directory (same convention as `fichier_re` in the `couverture_funclua`
/// example): the file is gitignored (reverse dump), so absent until regenerated on the machine;
/// its absence is not an error, just less enrichment.
fn charger_handlers_funclua() -> BTreeMap<u32, u64> {
    let mut out = BTreeMap::new();
    let Ok(cwd) = std::env::current_dir() else {
        return out;
    };
    let mut courant: &std::path::Path = &cwd;
    let chemin = loop {
        let candidat = courant.join("data/re/funclua-cmdid-handlers.json");
        if candidat.is_file() {
            break candidat;
        }
        match courant.parent() {
            Some(p) => courant = p,
            None => return out,
        }
    };
    let Ok(texte) = std::fs::read_to_string(&chemin) else {
        return out;
    };
    let Ok(brut) = serde_json::from_str::<BTreeMap<String, String>>(&texte) else {
        return out;
    };
    for (k, v) in brut {
        let (Some(id), Some(va)) = (
            k.strip_prefix("0x")
                .and_then(|h| u32::from_str_radix(h, 16).ok()),
            v.strip_prefix("0x")
                .and_then(|h| u64::from_str_radix(h, 16).ok()),
        ) else {
            continue;
        };
        out.insert(id, va);
    }
    out
}

/// Byte-exact analysis of a `.lua.bin` script: disassembles the real Lua 5.2 bytecode container
/// ([`nie_lua::bytecode::parse`], not an external decompiler) and extracts what a mode record
/// needs: instruction/function counts, `INCLUDE`d modules, and the `funcLuaMenuCommand` commands
/// the script is structurally able to emit (a constant integer in the pool matching a known
/// `cmdId` of the reverse dump; a false positive would need a 32-bit hash collision).
///
/// Returns `{"erreur": ...}` if the file is not a recognised Lua 5.2 bytecode, rather than an empty
/// object that would pass for "nothing to report".
fn analyse_lua(bytes: &[u8], handlers: &BTreeMap<u32, u64>) -> Json {
    let analysis = analyze_mode_script("", bytes, handlers);
    if let Some(error) = analysis.error {
        return serde_json::json!({ "erreur": error });
    }
    let commandes: Vec<Json> = analysis
        .commands
        .iter()
        .map(|command| {
            let id = command.cmd_id;
            serde_json::json!({
                "cmdId": format!("0x{id:08X}"),
                "nom": nie_lua::menu_host::command_name(id),
                "handler": format!("0x{:X}", command.handler),
            })
        })
        .collect();

    serde_json::json!({
        "instructions": analysis.instructions,
        "fonctions": analysis.functions,
        "includes": analysis.includes,
        "commandes": commandes,
    })
}

/// The **content** of a mode's files, not just their inventory.
///
/// `collect` counts and names; here the files are opened. Each screen yields its layers and
/// focusables, each `objbin` its parsed object, each `g4tx` its textures and regions, and the
/// mode text keys (those carried by `nie.exe`, not by the objects) are resolved in the localized
/// tables.
///
/// # Errors
///
/// Returns an error if the VFS cannot be opened. A single unreadable file is not one: it is just
/// absent from the result, as in `collect`.
pub fn contenu_json(vfs: &Vfs, def: &ModeDef, exe: Option<&std::path::Path>) -> Result<Json> {
    let facts = collect(vfs, def);

    // Screens: re-read the cfg to render its layers in file order, which the sorted set of `collect`
    // loses.
    let mut screens = Vec::new();
    for (stem, path) in &facts.screens {
        let Ok(bytes) = vfs.read(path) else { continue };
        let Ok(file) = cfgbin::parse_t2b(&bytes) else {
            continue;
        };
        let (mut layers, mut focus) = (Vec::new(), 0usize);
        walk(&file.entries, &mut |e: &CfgEntry| {
            if e.name.contains("LIST_BEG") || e.name.contains("LIST_END") {
                return;
            }
            if e.name.starts_with("MENU_LAYER_INFO")
                && let Some(n) = first_string(e)
            {
                layers.push(n.to_string());
            } else if e.name.starts_with("MENU_FOCUS_BASE_INFO") {
                focus += 1;
            }
        });
        screens.push(serde_json::json!({
            "screen": stem, "cfg": path, "octets": bytes.len(),
            "layers": layers, "focus": focus,
        }));
    }

    // Menu objects: the whole parsed object (components included).
    let mut objbins = Vec::new();
    for path in &facts.objbins {
        let Ok(bytes) = vfs.read(path) else { continue };
        let Ok(obj) = objbin::parse(&bytes) else {
            continue;
        };
        objbins.push(serde_json::json!({
            "path": path, "octets": bytes.len(), "objet": obj,
        }));
    }

    // Textures: dimensions and regions, as the `.g4tx` declares them.
    let mut textures = Vec::new();
    for path in &facts.g4tx {
        // The catalogued path carries `<LG>` for the locale; the VFS wants a real path.
        for candidat in chemins_locale(path) {
            let Ok(bytes) = vfs.read(&candidat) else {
                continue;
            };
            let Ok(atlas) = nie_formats::g4tx::parse(&bytes) else {
                continue;
            };
            let tex: Vec<Json> = atlas
                .textures
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "id": t.id, "nom": t.name, "largeur": t.width, "hauteur": t.height,
                        "dds": t.is_dds,
                        "regions": t.sub_textures.iter().map(|s| serde_json::json!({
                            "nom": s.name, "x": s.x, "y": s.y, "w": s.width, "h": s.height,
                        })).collect::<Vec<_>>(),
                    })
                })
                .collect();
            textures.push(serde_json::json!({
                "path": candidat, "catalogue": path, "octets": bytes.len(), "textures": tex,
            }));
            break;
        }
    }

    let handlers_funclua = charger_handlers_funclua();
    let lua: Vec<Json> = facts
        .lua
        .iter()
        .map(|p| {
            let Ok(bytes) = vfs.read(p) else {
                return serde_json::json!({ "path": p, "octets": 0 });
            };
            let mut entry = serde_json::json!({ "path": p, "octets": bytes.len() });
            let analyse = analyse_lua(&bytes, &handlers_funclua);
            if let (Json::Object(e), Json::Object(a)) = (&mut entry, analyse) {
                e.extend(a);
            }
            entry
        })
        .collect();

    let messages = def.key_pattern.map_or_else(
        || serde_json::json!({}),
        |motif| messages_du_mode(vfs, motif, exe),
    );

    Ok(serde_json::json!({
        "slug": def.slug,
        "label": def.label,
        "screens": screens,
        "objbins": objbins,
        "textures": textures,
        "lua": lua,
        "messages": messages,
    }))
}

/// Possible real paths for a catalogued path carrying `<LG>`.
///
/// Order is a decision: `fr` first (the wiki is French), then the variant without a locale
/// folder, which is the one for non-localized atlases.
fn chemins_locale(path: &str) -> Vec<String> {
    let prefixe = |p: &str| {
        if p.starts_with("data/") {
            p.to_string()
        } else {
            format!("data/{p}")
        }
    };
    if !path.contains("<LG>") {
        return vec![prefixe(path)];
    }
    let mut v: Vec<String> = ["fr", "en", "ja"]
        .iter()
        .map(|l| prefixe(&path.replace("<LG>", l)))
        .collect();
    v.push(prefixe(&path.replace("<LG>/", "")));
    v
}

/// Resolve the mode text keys held by the binary, in each locale.
///
/// Screen labels go through the `objbin` slots; the mode **messages** (network errors,
/// confirmations) are named only in `nie.exe`, and the tables hold just their CRC-32. So we read
/// the binary strings, keep those containing `motif`, and look them up in every table of the
/// locale.
fn messages_du_mode(vfs: &Vfs, motif: &str, exe: Option<&std::path::Path>) -> Json {
    let Some(exe) = exe else {
        return serde_json::json!({});
    };
    let Ok(bin) = std::fs::read(exe) else {
        return serde_json::json!({});
    };

    // Printable ASCII and UTF-16LE strings of the binary, filtered on the mode pattern. Kizuna
    // class/menu names are stored as UTF-16 in nie.exe; limiting to ASCII produced an empty message
    // catalogue although the CRCs were present.
    let mut cles: BTreeSet<String> = BTreeSet::new();
    let mut collect = |s: &str| {
        if s.len() >= 4
            && s.contains(motif)
            && !s.contains('/')
            && !s.contains('.')
            && s.bytes().all(|b| b.is_ascii_graphic() || b == b'_')
        {
            cles.insert(s.to_string());
        }
    };

    let mut ascii = Vec::new();
    for &b in &bin {
        if b.is_ascii_graphic() || b == b'_' {
            ascii.push(b);
        } else {
            if ascii.len() >= 4
                && let Ok(s) = core::str::from_utf8(&ascii)
            {
                collect(s);
            }
            ascii.clear();
        }
    }
    if ascii.len() >= 4
        && let Ok(s) = core::str::from_utf8(&ascii)
    {
        collect(s);
    }

    let mut wide = Vec::new();
    for pair in bin.as_chunks::<2>().0 {
        let unit = u16::from_le_bytes(*pair);
        if (0x20..=0x7e).contains(&unit) {
            wide.push(unit);
        } else {
            if wide.len() >= 4
                && let Ok(s) = String::from_utf16(&wide)
            {
                collect(&s);
            }
            wide.clear();
        }
    }
    if wide.len() >= 4
        && let Ok(s) = String::from_utf16(&wide)
    {
        collect(&s);
    }

    // The RE dictionary is the source of truth when the executable keeps only the CRC (common for
    // text tables). It links `TEXT_ID_*`, `*_message_*` and Kizuna ids to localized entries.
    if let Ok(cwd) = std::env::current_dir() {
        let mut courant = cwd.as_path();
        'cherche_dico: loop {
            let chemin = courant.join("data/re/menu-crc32-dictionary.json");
            if chemin.is_file() {
                if let Ok(texte) = std::fs::read_to_string(chemin)
                    && let Ok(dico) = serde_json::from_str::<BTreeMap<String, String>>(&texte)
                {
                    for nom in dico.values().filter(|nom| nom.contains(motif)) {
                        collect(nom);
                    }
                }
                break 'cherche_dico;
            }
            match courant.parent() {
                Some(parent) => courant = parent,
                None => break 'cherche_dico,
            }
        }
    }

    let mut out = serde_json::Map::new();
    for locale in LOCALES {
        let prefixe = format!("data/common/text/{locale}/");
        let tables: Vec<String> = vfs
            .iter()
            .map(|(p, _)| p.to_string())
            .filter(|p| {
                p.starts_with(&prefixe)
                    && p.ends_with(".cfg.bin")
                    && p[prefixe.len()..].find('/').is_none()
            })
            .collect();
        let mut par_cle = serde_json::Map::new();
        for table in tables {
            let Ok(bytes) = vfs.read(&table) else {
                continue;
            };
            let Ok(file) = cfgbin::parse_t2b(&bytes) else {
                continue;
            };
            let mut index: BTreeMap<u32, String> = BTreeMap::new();
            walk(&file.entries, &mut |e: &CfgEntry| {
                if !e.name.starts_with("TEXT_INFO")
                    || e.name.contains("BEGIN")
                    || e.name.contains("END")
                {
                    return;
                }
                let hash = e.variables.iter().find_map(|v| match v {
                    Value::Int(i) => Some(u32::from_ne_bytes(i.to_ne_bytes())),
                    _ => None,
                });
                let texte = e.variables.iter().rev().find_map(|v| match v {
                    Value::String(s) if !s.is_empty() => Some(s.clone()),
                    _ => None,
                });
                if let (Some(h), Some(t)) = (hash, texte) {
                    index.insert(h, t);
                }
            });
            if index.is_empty() {
                continue;
            }
            let court = table.rsplit('/').next().unwrap_or(&table).to_string();
            for cle in &cles {
                if let Some(t) = index.get(&cfgbin::crc32(cle.as_bytes())) {
                    par_cle.insert(
                        cle.clone(),
                        serde_json::json!({ "texte": t, "table": court }),
                    );
                }
            }
        }
        out.insert(locale.to_string(), Json::Object(par_cle));
    }
    Json::Object(out)
}

/// Create the catalogue tables if missing.
///
/// Deliberately outside `schema.sql` (nie-index): this catalogue is a UI tooling product, not part
/// of the RE base, and defining it here avoids touching a file shared with other work.
pub fn ensure_schema(conn: &nie_index::rusqlite::Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS mode (
            id          INTEGER PRIMARY KEY,
            slug        TEXT NOT NULL UNIQUE,
            label       TEXT NOT NULL,
            label_en    TEXT,
            label_ja    TEXT,
            text_hash   INTEGER,
            official    INTEGER NOT NULL DEFAULT 0,
            icon_atlas  TEXT,
            icon_region TEXT,
            screens     INTEGER NOT NULL DEFAULT 0,
            layers      INTEGER NOT NULL DEFAULT 0,
            focus       INTEGER NOT NULL DEFAULT 0,
            note        TEXT
        );
        CREATE TABLE IF NOT EXISTS mode_screen (
            id       INTEGER PRIMARY KEY,
            mode_id  INTEGER NOT NULL REFERENCES mode(id) ON DELETE CASCADE,
            screen   TEXT NOT NULL,
            cfg_path TEXT NOT NULL,
            UNIQUE(mode_id, screen)
        );
        CREATE TABLE IF NOT EXISTS mode_asset (
            id      INTEGER PRIMARY KEY,
            mode_id INTEGER NOT NULL REFERENCES mode(id) ON DELETE CASCADE,
            kind    TEXT NOT NULL,
            path    TEXT NOT NULL,
            UNIQUE(mode_id, kind, path)
        );
        CREATE INDEX IF NOT EXISTS idx_mode_asset ON mode_asset(mode_id, kind);
        CREATE TABLE IF NOT EXISTS mode_text (
            id      INTEGER PRIMARY KEY,
            mode_id INTEGER NOT NULL REFERENCES mode(id) ON DELETE CASCADE,
            obj     TEXT NOT NULL,
            slot    TEXT NOT NULL,
            hash    INTEGER NOT NULL,
            locale  TEXT NOT NULL,
            text    TEXT NOT NULL,
            UNIQUE(mode_id, obj, slot, hash, locale)
        );
        CREATE INDEX IF NOT EXISTS idx_mode_text ON mode_text(mode_id, locale);",
    )
    .context("create the mode catalogue tables")?;
    Ok(())
}

/// Index every mode and write the catalogue. Returns (modes, screens, assets, ...).
pub fn index(db: &nie_index::Db, vfs: &Vfs) -> Result<(usize, usize, usize, usize)> {
    let conn = db.conn();
    ensure_schema(conn)?;
    conn.execute_batch("BEGIN")?;

    // Official labels: the name the GAME displays, in the three locales.
    let textes: Vec<(&str, BTreeMap<u32, String>)> = LOCALES
        .iter()
        .map(|lg| (*lg, charger_menu_text(vfs, lg)))
        .collect();
    let libelle = |lg: &str, h: Option<u32>| -> Option<String> {
        let h = h?;
        textes.iter().find(|(l, _)| *l == lg)?.1.get(&h).cloned()
    };

    let (mut n_modes, mut n_screens, mut n_assets, mut n_texts) = (0usize, 0usize, 0usize, 0usize);
    for def in MODES {
        let f = collect(vfs, def);
        // The game label wins over the fallback name; if missing, keep ours.
        let label_fr = libelle("fr", def.text_hash).unwrap_or_else(|| def.label.to_string());
        conn.execute(
            "INSERT INTO mode(slug, label, label_en, label_ja, text_hash, official,
                              icon_atlas, icon_region, screens, layers, focus, note)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
             ON CONFLICT(slug) DO UPDATE SET
                label=excluded.label, label_en=excluded.label_en, label_ja=excluded.label_ja,
                text_hash=excluded.text_hash, official=excluded.official,
                icon_atlas=excluded.icon_atlas,
                icon_region=excluded.icon_region, screens=excluded.screens,
                layers=excluded.layers, focus=excluded.focus, note=excluded.note",
            nie_index::rusqlite::params![
                def.slug,
                label_fr,
                libelle("en", def.text_hash),
                libelle("ja", def.text_hash),
                def.text_hash.map(i64::from),
                i64::from(def.official),
                def.icon_region.map(
                    |_| "data/dx11/menu/100_mainmenu/mainmenu90/mainmenu90_01/mainmenu90_01.g4tx"
                ),
                def.icon_region,
                f.screens.len() as i64,
                f.layers.len() as i64,
                f.focus as i64,
                def.note,
            ],
        )?;
        let mode_id: i64 =
            conn.query_row("SELECT id FROM mode WHERE slug=?1", [def.slug], |r| {
                r.get(0)
            })?;

        // Exact reindex: a screen's sets can shrink after a VFS update. `INSERT OR IGNORE` alone kept
        // ghost assets in SQLite, making the API differ from the current JSON export.
        for table in ["mode_screen", "mode_asset", "mode_text"] {
            conn.execute(&format!("DELETE FROM {table} WHERE mode_id=?1"), [mode_id])?;
        }

        for (stem, path) in &f.screens {
            conn.execute(
                "INSERT OR IGNORE INTO mode_screen(mode_id, screen, cfg_path) VALUES(?1,?2,?3)",
                nie_index::rusqlite::params![mode_id, stem, path],
            )?;
            n_screens += 1;
        }
        for (kind, set) in [
            ("layer", &f.layers),
            ("objbin", &f.objbins),
            ("g4pkm", &f.g4pkm),
            ("g4tx", &f.g4tx),
            ("component", &f.components),
            ("lua", &f.lua),
        ] {
            for p in set {
                conn.execute(
                    "INSERT OR IGNORE INTO mode_asset(mode_id, kind, path) VALUES(?1,?2,?3)",
                    nie_index::rusqlite::params![mode_id, kind, p],
                )?;
                n_assets += 1;
            }
        }
        // Screen interface texts, resolved in each available locale. A slot whose hash is not in
        // `menu_text` is NOT inserted: a visible gap beats an empty line passing for a label.
        for (obj, slot, hash) in &f.text_slots {
            for (lg, table) in &textes {
                if let Some(t) = table.get(hash) {
                    conn.execute(
                        "INSERT OR IGNORE INTO mode_text(mode_id, obj, slot, hash, locale, text)
                         VALUES(?1,?2,?3,?4,?5,?6)",
                        nie_index::rusqlite::params![mode_id, obj, slot, i64::from(*hash), lg, t],
                    )?;
                    n_texts += 1;
                }
            }
        }

        n_modes += 1;
        println!(
            "  {} {:<15} screens={:<3} layers={:<4} objbin={:<4} g4pkm={:<4} g4tx={:<4} lua={:<3} focus={}",
            if def.official { "*" } else { " " },
            def.slug,
            f.screens.len(),
            f.layers.len(),
            f.objbins.len(),
            f.g4pkm.len(),
            f.g4tx.len(),
            f.lua.len(),
            f.focus
        );
    }

    conn.execute_batch("COMMIT")?;
    Ok((n_modes, n_screens, n_assets, n_texts))
}

/// Export the catalogue as JSON (for the web UI).
pub fn export_json(db: &nie_index::Db) -> Result<serde_json::Value> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, slug, label, icon_atlas, icon_region, screens, layers, focus, note,
                label_en, label_ja, official
         FROM mode ORDER BY official DESC, slug",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, i64>(5)?,
            r.get::<_, i64>(6)?,
            r.get::<_, i64>(7)?,
            r.get::<_, Option<String>>(8)?,
            r.get::<_, Option<String>>(9)?,
            r.get::<_, Option<String>>(10)?,
            r.get::<_, i64>(11)?,
        ))
    })?;

    let mut modes = Vec::new();
    for row in rows {
        let (id, slug, label, atlas, region, screens, layers, focus, note, en, ja, official) = row?;
        let mut screens_v = Vec::new();
        let mut s = conn
            .prepare("SELECT screen, cfg_path FROM mode_screen WHERE mode_id=?1 ORDER BY screen")?;
        for r in s.query_map([id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })? {
            let (screen, cfg) = r?;
            screens_v.push(serde_json::json!({ "screen": screen, "cfg": cfg }));
        }
        let mut assets = serde_json::Map::new();
        let mut a =
            conn.prepare("SELECT kind, path FROM mode_asset WHERE mode_id=?1 ORDER BY kind, path")?;
        for r in a.query_map([id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })? {
            let (kind, path) = r?;
            assets
                .entry(kind)
                .or_insert_with(|| serde_json::Value::Array(Vec::new()))
                .as_array_mut()
                .expect("tableau")
                .push(serde_json::Value::String(path));
        }
        // Interface texts, grouped by locale.
        let mut textes = serde_json::Map::new();
        let mut t = conn.prepare(
            "SELECT locale, obj, slot, text FROM mode_text WHERE mode_id=?1
             ORDER BY locale, obj, slot",
        )?;
        for r in t.query_map([id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })? {
            let (locale, obj, slot, texte) = r?;
            textes
                .entry(locale)
                .or_insert_with(|| serde_json::Value::Array(Vec::new()))
                .as_array_mut()
                .expect("tableau")
                .push(serde_json::json!({ "obj": obj, "slot": slot, "text": texte }));
        }

        modes.push(serde_json::json!({
            "texts": textes,
            "slug": slug,
            "label": label,
            "labelEn": en,
            "labelJa": ja,
            "official": official != 0,
            "icon": { "atlas": atlas, "region": region },
            "counts": { "screens": screens, "layers": layers, "focus": focus },
            "note": note,
            "screens": screens_v,
            "assets": assets,
        }));
    }
    Ok(serde_json::json!({ "modes": modes }))
}

#[cfg(test)]
mod tests {
    use super::{MODES, classify_screen};

    #[test]
    fn coverage_keeps_empty_prefix_modes_unassigned() {
        assert!(
            classify_screen("competition").is_empty(),
            "a mode without a prefix must not capture a stem"
        );
    }

    #[test]
    fn coverage_uses_the_same_prefixes_as_mode_index() {
        assert_eq!(classify_screen("chara_edit_top"), vec!["chara_edit"]);
        assert_eq!(
            classify_screen("victory_road_top_menu"),
            vec!["victory_road"]
        );
        assert_eq!(MODES.iter().filter(|def| def.official).count(), 5);
    }
}
