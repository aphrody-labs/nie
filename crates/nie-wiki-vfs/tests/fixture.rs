//! Pages computed from a synthetic installation built in a temp directory: no game file is read.

use std::fs;
use std::path::Path;

use nie_formats::cfgbin::{CfgEntry, RdbnList, RdbnRow, RdbnValue, Value, encode_rdbn, encode_t2b};
use nie_formats::vfs::Vfs;
use nie_wiki_vfs::{Id, Query, Wiki};
use tempfile::TempDir;

const CHARA_ID: u32 = 0xA000_0001;
const PARAM_ID: u32 = 0xB000_0001;
const NAME_HASH: u32 = 0x1111_0001;
const LAST_HASH: u32 = 0x1111_0002;
const DESC_HASH: u32 = 0x2222_0001;
const SKILL_ID: u32 = 0xC000_0001;
const SKILL_NAME: u32 = 0x3333_0001;
const SKILL_DESC: u32 = 0x3333_0002;
const TEAM_ID: u32 = 0xD000_0001;
const TEAM_NAME: u32 = 0x4444_0001;

fn int(value: u32) -> Value {
    Value::Int(value as i32)
}

fn string(value: &str) -> Value {
    Value::String(value.to_owned())
}

fn entry(name: &str, variables: Vec<Value>, children: Vec<CfgEntry>) -> CfgEntry {
    CfgEntry {
        name: name.to_owned(),
        variables,
        children,
    }
}

fn t2b(root: &str, children: Vec<CfgEntry>) -> Vec<u8> {
    encode_t2b(&[entry(root, vec![int(children.len() as u32)], children)])
}

fn noun(hash: u32, form: i32, label: &str) -> CfgEntry {
    let mut vars = vec![
        int(hash),
        Value::Int(form),
        string(""),
        string(""),
        string(""),
    ];
    vars.push(string(label));
    vars.extend([string(""), string(""), string(""), string("")]);
    vars.extend((0..5).map(|_| Value::Int(0)));
    entry("NOUN_INFO", vars, vec![])
}

fn text(hash: u32, label: &str) -> CfgEntry {
    entry(
        "TEXT_INFO",
        vec![int(hash), Value::Int(0), string(label)],
        vec![],
    )
}

fn element_key(key: &str) -> u32 {
    crc32fast::hash(format!("element_type_{key}").as_bytes())
}

fn write(dir: &Path, vfs: &mut Vfs, internal: &str, bytes: &[u8]) {
    let disk = dir.join(internal.replace('/', "__"));
    fs::write(&disk, bytes).unwrap();
    vfs.add_overlay_file(internal.to_owned(), disk, bytes.len() as u32);
}

fn rdbn(list: &str, rows: Vec<Vec<(&str, RdbnValue)>>) -> Vec<u8> {
    let rows = rows
        .into_iter()
        .map(|fields| RdbnRow {
            fields: fields.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
        })
        .collect();
    encode_rdbn(&[RdbnList {
        name: list.to_owned(),
        type_name: "INFO".to_owned(),
        rows,
    }])
    .unwrap()
}

/// Two languages (fr, de), plus a `event` folder that is not a language.
fn installation(dir: &TempDir) -> Vfs {
    let mut vfs = Vfs::new();
    let root = dir.path();

    let mut base_vars = (0..20).map(|_| Value::Int(0)).collect::<Vec<_>>();
    base_vars[0] = int(CHARA_ID);
    base_vars[1] = string("c00000001");
    base_vars[3] = int(NAME_HASH);
    base_vars[4] = int(LAST_HASH);
    base_vars[11] = Value::Int(2);
    base_vars[15] = int(0x5555_0001);
    base_vars[16] = int(TEAM_ID);
    base_vars[19] = int(DESC_HASH);
    let mut other_vars = base_vars.clone();
    other_vars[0] = int(CHARA_ID + 1);
    other_vars[1] = string("c00000002");
    other_vars.truncate(11);
    let bases = vec![
        entry("CHARA_BASE_INFO", base_vars, vec![]),
        entry("CHARA_BASE_INFO", other_vars, vec![]),
    ];
    write(
        root,
        &mut vfs,
        "data/common/gamedata/character/chara_base_1.00.00.00.cfg.bin",
        &t2b("CHARA_BASE_INFO_LIST_BEG", bases),
    );

    let mut param_vars = (0..12).map(|_| Value::Int(0)).collect::<Vec<_>>();
    param_vars[0] = int(PARAM_ID);
    param_vars[1] = int(CHARA_ID);
    param_vars[2] = Value::Int(1);
    param_vars[3] = Value::Int(4);
    param_vars[4] = Value::Int(2);
    param_vars[8] = Value::Int(7);
    param_vars[10] = Value::Int(12);
    param_vars[11] = int(SKILL_ID);
    let mut stale_vars = param_vars.clone();
    stale_vars[0] = int(PARAM_ID + 1);
    stale_vars[1] = int(CHARA_ID + 99);
    write(
        root,
        &mut vfs,
        "data/common/gamedata/character/chara_param_1.00.00.00.cfg.bin",
        &t2b(
            "CHARA_PARAM_INFO_LIST_BEG",
            vec![
                entry("CHARA_PARAM_INFO", param_vars, vec![]),
                entry("CHARA_PARAM_INFO", stale_vars, vec![]),
            ],
        ),
    );
    // A sibling table that shares the stem prefix must not be taken for the table.
    write(
        root,
        &mut vfs,
        "data/common/gamedata/character/chara_param_table_config_9.00.00.00.cfg.bin",
        &t2b("CHARA_PARAM_TABLE_BEGIN", vec![]),
    );

    write(
        root,
        &mut vfs,
        "data/common/gamedata/skill/skill_config_1.00.00.00.cfg.bin",
        &rdbn(
            "m_skillInfoList",
            vec![vec![
                ("skillID", RdbnValue::Hash(SKILL_ID)),
                ("skillIDStr", RdbnValue::Condition("whs00001".to_owned())),
                ("skillNameId", RdbnValue::Hash(SKILL_NAME)),
                ("skillDescId", RdbnValue::Hash(SKILL_DESC)),
                ("element", RdbnValue::Int(1)),
                ("category", RdbnValue::Int(2)),
                ("recastTime", RdbnValue::Float(90.5)),
            ]],
        ),
    );

    write(
        root,
        &mut vfs,
        "data/common/gamedata/character/belong_team_config_0.00.00.cfg.bin",
        &rdbn(
            "m_belongTeamInfoList",
            vec![vec![
                ("belongTeamId", RdbnValue::Hash(TEAM_ID)),
                ("teamNameTextId", RdbnValue::Hash(TEAM_NAME)),
                (
                    "teamEmblemId_IE",
                    RdbnValue::Hash(crc32fast::hash(b"em010001")),
                ),
                (
                    "teamEmblemId_GO",
                    RdbnValue::Hash(crc32fast::hash(b"em_missing")),
                ),
                ("teamEmblemId_V", RdbnValue::Hash(0)),
                ("teamNumber_IE1", RdbnValue::Int(5)),
            ]],
        ),
    );
    write(
        root,
        &mut vfs,
        "data/dx11/menu/200_icon/01_icon_emblem/em010001.g4tx",
        b"x",
    );
    write(
        root,
        &mut vfs,
        "data/dx11/menu/200_icon/01_icon_emblem/em010001_s.g4tx",
        b"x",
    );

    for (language, label) in [("fr", "Vent"), ("de", "Wind")] {
        let dir = format!("data/common/text/{language}/");
        let (given, family, desc, skill, skill_desc, team) = match language {
            "fr" => (
                "Aki",
                "Kino",
                "Description fr\\nligne 2",
                "Tir fr",
                "Tir decrit fr",
                "Raimon",
            ),
            _ => (
                "Aki",
                "Kino",
                "Beschreibung de",
                "Schuss de",
                "Schuss beschrieben",
                "Raimon DE",
            ),
        };
        let files = [
            (
                "chara_text",
                t2b(
                    "NOUN_INFO_BEGIN",
                    vec![
                        noun(NAME_HASH, 0, &format!("{given} {family}")),
                        noun(NAME_HASH, 11, family),
                        noun(NAME_HASH, 12, given),
                        noun(LAST_HASH, 0, family),
                    ],
                ),
            ),
            (
                "chara_description_text",
                t2b("TEXT_INFO_BEGIN", vec![text(DESC_HASH, desc)]),
            ),
            (
                "skill_text",
                encode_t2b(&[
                    entry(
                        "NOUN_INFO_BEGIN",
                        vec![int(1)],
                        vec![noun(SKILL_NAME, 0, skill)],
                    ),
                    entry(
                        "TEXT_INFO_BEGIN",
                        vec![int(1)],
                        vec![text(SKILL_DESC, skill_desc)],
                    ),
                ]),
            ),
            (
                "team_text",
                t2b("NOUN_INFO_BEGIN", vec![noun(TEAM_NAME, 0, team)]),
            ),
            (
                "system_text",
                t2b("NOUN_INFO_BEGIN", vec![noun(element_key("wind"), 0, label)]),
            ),
        ];
        for (stem, bytes) in files {
            write(root, &mut vfs, &format!("{dir}{stem}.cfg.bin"), &bytes);
        }
    }
    write(
        root,
        &mut vfs,
        "data/common/text/event/ev_text.cfg.bin",
        &t2b("TEXT_INFO_BEGIN", vec![]),
    );
    vfs
}

#[test]
fn languages_come_from_the_text_folders_that_hold_character_names() {
    let dir = TempDir::new().unwrap();
    let wiki = Wiki::new(installation(&dir)).unwrap();
    let languages: Vec<_> = wiki.languages().iter().map(|l| l.folder.as_str()).collect();
    assert_eq!(languages, ["de", "fr"]);
}

#[test]
fn character_joins_param_base_and_labels_per_language() {
    let dir = TempDir::new().unwrap();
    let wiki = Wiki::new(installation(&dir)).unwrap();
    let characters = wiki.characters(&Query::default()).unwrap().items;
    // The second param names a missing base entry and is skipped.
    assert_eq!(characters.len(), 1);
    let c = &characters[0];

    assert_eq!(c.chara_param_id, Id(PARAM_ID));
    assert_eq!(c.chara_base_id, Id(CHARA_ID));
    assert_eq!(c.internal_code, "c00000001");
    // Gender and positions are the stored codes.
    assert_eq!(c.gender.as_ref().map(|g| g.value), Some(2));
    assert_eq!(c.main_position.value, 4);
    assert_eq!(c.sub_position.value, 2);
    assert!(c.main_position.label.is_none());
    assert_eq!(c.growth_pattern, 7);

    let name = c.name.as_ref().unwrap();
    assert_eq!(name.text("fr"), Some("Aki Kino"));
    let forms: Vec<_> = name.texts["fr"]
        .forms
        .iter()
        .map(|f| (f.form, f.text.as_str()))
        .collect();
    assert_eq!(forms, [(0, "Aki Kino"), (11, "Kino"), (12, "Aki")]);
    assert_eq!(c.family_name.as_ref().unwrap().text("de"), Some("Kino"));

    let description = c.description.as_ref().unwrap();
    assert_eq!(description.text("fr"), Some("Description fr\nligne 2"));
    assert_eq!(
        description.texts["fr"].forms[0].raw.as_deref(),
        Some("Description fr\\nligne 2")
    );

    // The element label is the system_text row keyed by the game's enumeration key.
    let element = c.element.label.as_ref().unwrap();
    assert_eq!(c.element.value, 1);
    assert_eq!(
        (element.text("fr"), element.text("de")),
        (Some("Vent"), Some("Wind"))
    );

    let team = c.team.as_ref().unwrap();
    assert_eq!(team.belong_team_id, Id(TEAM_ID));
    assert_eq!(team.name.as_ref().unwrap().text("de"), Some("Raimon DE"));

    assert_eq!(c.skills.len(), 1);
    assert_eq!(c.skills[0].learn_level, 12);
    assert_eq!(
        c.skills[0].name.as_ref().unwrap().text("fr"),
        Some("Tir fr")
    );

    assert_eq!(c.param_source.table, "CHARA_PARAM_INFO");
    assert_eq!(c.param_source.index, 0);
    assert!(
        c.param_source
            .vfs_path
            .ends_with("chara_param_1.00.00.00.cfg.bin")
    );
    assert_eq!(c.base_source.index, 0);
}

#[test]
fn a_base_without_a_gender_variable_has_no_gender() {
    let dir = TempDir::new().unwrap();
    let mut vfs = installation(&dir);
    // Re-point the param at the second base, which stops before variable 11.
    let mut param_vars = (0..12).map(|_| Value::Int(0)).collect::<Vec<_>>();
    param_vars[0] = int(PARAM_ID + 5);
    param_vars[1] = int(CHARA_ID + 1);
    write(
        dir.path(),
        &mut vfs,
        "data/common/gamedata/character/chara_param_2.00.00.00.cfg.bin",
        &t2b(
            "CHARA_PARAM_INFO_LIST_BEG",
            vec![entry("CHARA_PARAM_INFO", param_vars, vec![])],
        ),
    );
    let wiki = Wiki::new(vfs).unwrap();
    // The highest version wins.
    let c = wiki
        .character("c00000002", &[] as &[&str])
        .unwrap()
        .unwrap();
    assert_eq!(c.chara_param_id, Id(PARAM_ID + 5));
    assert!(c.gender.is_none());
    assert!(c.team.is_none());
}

#[test]
fn skill_keeps_every_stored_field() {
    let dir = TempDir::new().unwrap();
    let wiki = Wiki::new(installation(&dir)).unwrap();
    let skill = wiki.skill("whs00001", &["fr"]).unwrap().unwrap();
    assert_eq!(skill.skill_id, Id(SKILL_ID));
    assert_eq!(skill.name.as_ref().unwrap().text("fr"), Some("Tir fr"));
    assert!(!skill.name.as_ref().unwrap().texts.contains_key("de"));
    assert_eq!(
        skill.description.as_ref().unwrap().texts["fr"].section,
        "TEXT_INFO"
    );
    assert_eq!(skill.category.as_ref().map(|c| c.value), Some(2));
    assert_eq!(
        skill
            .element
            .as_ref()
            .unwrap()
            .label
            .as_ref()
            .unwrap()
            .text("fr"),
        Some("Vent")
    );
    assert_eq!(skill.fields["recastTime"], 90.5);
    assert_eq!(skill.fields["skillIDStr"], "whs00001");
    assert_eq!(
        (skill.source.table.as_str(), skill.source.index),
        ("m_skillInfoList", 0)
    );
    assert!(wiki.skill("0xC0000001", &["fr"]).unwrap().is_some());
    assert!(wiki.skill("missing", &["fr"]).unwrap().is_none());
}

#[test]
fn team_resolves_emblems_by_hashing_file_names() {
    let dir = TempDir::new().unwrap();
    let wiki = Wiki::new(installation(&dir)).unwrap();
    let team = wiki.team("0xD0000001", &[] as &[&str]).unwrap().unwrap();
    assert_eq!(team.name.as_ref().unwrap().text("fr"), Some("Raimon"));

    let ie = &team.emblems["teamEmblemId_IE"];
    assert_eq!(ie.emblem_id, Id(0xD5AB_E1F0));
    assert_eq!(
        ie.files,
        ["data/dx11/menu/200_icon/01_icon_emblem/em010001.g4tx"]
    );
    // No file hashes to this id: the emblem stays unresolved, not guessed.
    assert!(team.emblems["teamEmblemId_GO"].files.is_empty());
    // A zero id is "no emblem" and is not listed.
    assert!(!team.emblems.contains_key("teamEmblemId_V"));

    assert_eq!(team.fields["teamNumber_IE1"], 5);
    assert_eq!(team.affiliated_characters.len(), 1);
    assert_eq!(team.affiliated_characters[0].internal_code, "c00000001");
    assert_eq!(team.roster.family, "team roster");
    assert_eq!(wiki.teams(&Query::default()).unwrap().total, 1);
    assert!(wiki.team("0xDEADBEEF", &[] as &[&str]).unwrap().is_none());
}

#[test]
fn languages_resolve_from_folders_and_bcp47_tags() {
    let dir = TempDir::new().unwrap();
    let wiki = Wiki::new(installation(&dir)).unwrap();
    assert_eq!(
        wiki.resolve_languages(&["FR", "fr", "de"]).unwrap(),
        ["fr", "de"]
    );
    assert_eq!(
        wiki.resolve_languages(&[] as &[&str]).unwrap(),
        ["de", "fr"]
    );
    assert!(wiki.resolve_languages(&["xx"]).is_err());
    assert!(wiki.character("c00000001", &["zh-Hans"]).is_err());
    assert_eq!(wiki.unsourced().len(), 3);
}

#[test]
fn lists_are_filtered_by_name_in_the_selected_languages_and_windowed() {
    let dir = TempDir::new().unwrap();
    let wiki = Wiki::new(installation(&dir)).unwrap();
    let query = |text: &str, languages: &[&str]| Query {
        languages: languages.iter().map(|l| (*l).to_owned()).collect(),
        text: Some(text.to_owned()),
        ..Query::default()
    };

    // The French team name is "Raimon", the German one "Raimon DE".
    assert_eq!(wiki.teams(&query("raimon", &["fr"])).unwrap().total, 1);
    assert_eq!(wiki.teams(&query(" de", &["fr"])).unwrap().total, 0);
    assert_eq!(wiki.teams(&query(" de", &["de"])).unwrap().total, 1);

    // Skills match on the name and on the code.
    assert_eq!(wiki.skills(&query("tir fr", &["fr"])).unwrap().total, 1);
    assert_eq!(wiki.skills(&query("tir fr", &["de"])).unwrap().total, 0);
    assert_eq!(wiki.skills(&query("WHS0000", &["de"])).unwrap().total, 1);

    // Characters match on either name, the code and the id.
    assert_eq!(wiki.characters(&query("kino", &["de"])).unwrap().total, 1);
    assert_eq!(
        wiki.characters(&query("c0000000", &["de"])).unwrap().total,
        1
    );
    assert_eq!(
        wiki.characters(&query("0xb0000001", &["de"]))
            .unwrap()
            .total,
        1
    );
    assert_eq!(wiki.characters(&query("nobody", &[])).unwrap().total, 0);

    // `total` counts every match, the window only bounds the items.
    let none = Query {
        limit: Some(0),
        ..Query::default()
    };
    let page = wiki.characters(&none).unwrap();
    assert_eq!((page.total, page.items.len()), (1, 0));
    let past = Query {
        offset: 5,
        ..Query::default()
    };
    assert!(wiki.skills(&past).unwrap().items.is_empty());
}

#[test]
fn sources_name_every_backing_table() {
    let dir = TempDir::new().unwrap();
    let wiki = Wiki::new(installation(&dir)).unwrap();
    let sources = wiki.sources().unwrap();
    let tables: Vec<_> = sources
        .iter()
        .map(|s| (s.family, s.table, s.entries))
        .collect();
    assert_eq!(
        tables,
        [
            ("characters", "CHARA_PARAM_INFO", 2),
            ("characters", "CHARA_BASE_INFO", 2),
            ("skills", "m_skillInfoList", 1),
            ("teams", "m_belongTeamInfoList", 1),
        ]
    );
}

#[test]
fn pages_show_provenance_emblems_and_explicit_empty_states() {
    use nie_wiki_vfs::html::{self, Ctx, ListParams};

    let dir = TempDir::new().unwrap();
    let wiki = Wiki::new(installation(&dir)).unwrap();
    let languages = wiki.languages();
    let fr = languages.iter().find(|l| l.folder == "fr").unwrap();
    let ctx = |rest| Ctx {
        languages,
        lang: fr,
        rest,
    };

    let team = wiki.team("0xD0000001", &[] as &[&str]).unwrap().unwrap();
    let page = html::team(&ctx("teams/0xD0000001"), &team);
    assert!(page.contains("<html lang=\"fr\">"));
    assert!(page.contains("<h1>Raimon</h1>"));
    assert!(page.contains("src=\"/tex/dx11/menu/200_icon/01_icon_emblem/em010001.png\""));
    assert!(page.contains("no VFS file name hashes to this id"));
    assert!(page.contains("team roster</strong>: no source in the VFS"));
    assert!(page.contains("belong_team_config_0.00.00.cfg.bin"));
    assert!(page.contains("m_belongTeamInfoList"));
    // The switcher keeps the page across languages.
    assert!(page.contains("href=\"/wiki/de/teams/0xD0000001\""));

    let character = wiki
        .character("c00000001", &[] as &[&str])
        .unwrap()
        .unwrap();
    let page = html::character(&ctx("characters/c00000001"), &character);
    assert!(page.contains("<h1>Aki Kino</h1>"));
    assert!(page.contains("stored code, no label in the VFS"));
    assert!(page.contains("chara_param_1.00.00.00.cfg.bin"));
    assert!(page.contains("Raimon"));

    let list = wiki.characters(&Query::default()).unwrap();
    let params = ListParams {
        text: Some("<b>".to_owned()),
        offset: 0,
        limit: 10,
    };
    let page = html::characters(&ctx("characters"), &list, &params);
    assert!(page.contains("value=\"&lt;b&gt;\""));
    assert!(!page.contains("<b>"));
    assert!(page.contains("1 match(es)"));

    let skill = wiki.skill("whs00001", &["fr"]).unwrap().unwrap();
    let page = html::skill(&ctx("skills/whs00001"), &skill);
    assert!(page.contains("invocation rates</strong>: no source in the VFS"));
    assert!(page.contains("Tir fr"));

    let page = html::index(languages, &wiki.sources().unwrap(), wiki.unsourced());
    assert!(page.contains("capsule lot contents</strong>: no source in the VFS"));
    assert!(
        page.contains("href=\"/wiki/zh-Hans/characters\"")
            || page.contains("href=\"/wiki/de/characters\"")
    );
}
