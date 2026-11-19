//! A synthetic installation for tests: every registry table and the texts of two languages
//! (`fr`, `de`), written into a directory and mounted as overlay files. No game data is read.

use std::fs;
use std::path::Path;

use nie_formats::cfgbin::{CfgEntry, RdbnList, RdbnRow, RdbnValue, Value, encode_rdbn, encode_t2b};
use nie_formats::vfs::Vfs;

pub const CHARA_ID: u32 = 0xA000_0001;
pub const PARAM_ID: u32 = 0xB000_0001;
pub const NAME_HASH: u32 = 0x1111_0001;
pub const LAST_HASH: u32 = 0x1111_0002;
pub const DESC_HASH: u32 = 0x2222_0001;
pub const SKILL_ID: u32 = 0xC000_0001;
pub const SKILL_NAME: u32 = 0x3333_0001;
pub const SKILL_DESC: u32 = 0x3333_0002;
pub const AURA_ID: u32 = 0xC000_0002;
pub const AURA_NAME: u32 = 0x7777_0001;
pub const UNKNOWN_SKILL: u32 = 0xDEAD_0001;
pub const TEAM_ID: u32 = 0xD000_0001;
pub const TEAM_NAME: u32 = 0x4444_0001;
pub const SERIES_ID: u32 = 0x5555_0001;
pub const SERIES_NAME: u32 = 0x6666_0001;

fn int(value: u32) -> Value {
    Value::Int(value as i32)
}

fn string(value: &str) -> Value {
    Value::String(value.to_owned())
}

fn zeros(n: usize) -> Vec<Value> {
    (0..n).map(|_| Value::Int(0)).collect()
}

fn entry(name: &str, variables: Vec<Value>, children: Vec<CfgEntry>) -> CfgEntry {
    CfgEntry {
        name: name.to_owned(),
        variables,
        children,
    }
}

/// One container (`<ROOT>`) holding `children`, as a T2B file.
fn t2b(root: &str, children: Vec<CfgEntry>) -> Vec<u8> {
    encode_t2b(&[entry(root, vec![int(children.len() as u32)], children)])
}

fn noun(hash: u32, form: i32, label: &str) -> CfgEntry {
    let mut vars = vec![int(hash), Value::Int(form)];
    vars.extend((0..3).map(|_| string("")));
    vars.push(string(label));
    vars.extend((0..4).map(|_| string("")));
    vars.extend(zeros(5));
    entry("NOUN_INFO", vars, vec![])
}

fn text(hash: u32, label: &str) -> CfgEntry {
    entry(
        "TEXT_INFO",
        vec![int(hash), Value::Int(0), string(label)],
        vec![],
    )
}

pub fn element_key(key: &str) -> u32 {
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

fn base_vars(id: u32, code: &str, len: usize) -> Vec<Value> {
    let mut vars = zeros(20);
    vars[0] = int(id);
    vars[1] = string(code);
    vars[3] = int(NAME_HASH);
    vars[4] = int(LAST_HASH);
    vars[11] = Value::Int(2);
    vars[15] = int(SERIES_ID);
    vars[16] = int(TEAM_ID);
    vars[19] = int(DESC_HASH);
    vars.truncate(len);
    vars
}

fn param_vars(id: u32, base: u32) -> Vec<Value> {
    let mut vars = zeros(16);
    vars[0] = int(id);
    vars[1] = int(base);
    vars[2] = Value::Int(1);
    vars[3] = Value::Int(4);
    vars[4] = Value::Int(2);
    vars[8] = Value::Int(7);
    vars[10] = Value::Int(12);
    vars[11] = int(SKILL_ID);
    vars[12] = Value::Int(3);
    vars[13] = int(AURA_ID);
    vars[14] = Value::Int(5);
    vars[15] = int(UNKNOWN_SKILL);
    vars
}

fn sort(position: i32) -> CfgEntry {
    entry("__SORT_INDEX", vec![Value::Int(position)], vec![])
}

fn pair(name: &str, a: i32, b: i32) -> CfgEntry {
    entry(name, vec![Value::Int(a), Value::Int(b)], vec![])
}

/// Two languages (fr, de), a `event` folder that is not a language, and every registry table.
pub fn installation(root: &Path) -> Vfs {
    let mut vfs = Vfs::new();
    let gd = "data/common/gamedata/";

    // chara_base: entry 0 is named by two params, entry 1 by none (and has no gender), entry 2
    // repeats the id of entry 0.
    let infos = vec![
        entry(
            "CHARA_BASE_INFO",
            base_vars(CHARA_ID, "c00000001", 20),
            vec![],
        ),
        entry(
            "CHARA_BASE_INFO",
            base_vars(CHARA_ID + 1, "c00000002", 11),
            vec![],
        ),
        entry("CHARA_BASE_INFO", base_vars(CHARA_ID, "dup", 20), vec![]),
        pair("CHARA_BASE_INFO_REF_BATTLE", 0, 2),
        pair("CHARA_BASE_INFO_REF_BATTLE", 2, 0),
        pair("CHARA_BASE_INFO_REF_BATTLE", 9, 5),
    ];
    write(
        root,
        &mut vfs,
        &format!("{gd}character/chara_base_1.00.00.00.cfg.bin"),
        &encode_t2b(&[
            entry("CHARA_BASE_INFO_LIST_BEG", vec![int(3)], infos),
            entry(
                "CHARA_BASE_BATTLE_LIST_BEG",
                vec![int(2)],
                vec![
                    pair("CHARA_BASE_BATTLE", 1, 2),
                    pair("CHARA_BASE_BATTLE", 3, 4),
                ],
            ),
        ]),
    );

    // chara_param: three entries (the second names no base) with their sort indexes.
    let params = vec![
        entry("CHARA_PARAM_INFO", param_vars(PARAM_ID, CHARA_ID), vec![]),
        entry(
            "CHARA_PARAM_INFO",
            param_vars(PARAM_ID + 1, CHARA_ID + 99),
            vec![],
        ),
        entry(
            "CHARA_PARAM_INFO",
            param_vars(PARAM_ID + 2, CHARA_ID),
            vec![],
        ),
        sort(2),
        sort(0),
        sort(1),
    ];
    write(
        root,
        &mut vfs,
        &format!("{gd}character/chara_param_1.00.00.00.cfg.bin"),
        &encode_t2b(&[
            entry("CHARA_PARAM_INFO_LIST_BEG", vec![int(3)], params),
            entry(
                "STORY_FIX_SKILL_INFO_LIST_BEG",
                vec![int(1)],
                vec![entry(
                    "STORY_FIX_SKILL_INFO",
                    vec![Value::Int(1), Value::Int(2)],
                    vec![],
                )],
            ),
        ]),
    );
    // A sibling table that shares the stem prefix must not be taken for chara_param, and a
    // file no family reads stays listed.
    write(
        root,
        &mut vfs,
        &format!("{gd}misc/unrelated_config_1.00.cfg.bin"),
        &t2b("X_LIST_BEG", vec![]),
    );
    write(
        root,
        &mut vfs,
        &format!("{gd}character/chara_param_table_config_9.00.00.00.cfg.bin"),
        &rdbn(
            "m_charaParamPresetTableParamList",
            vec![vec![("param_min_kick", RdbnValue::Int(5))]],
        ),
    );
    write(
        root,
        &mut vfs,
        &format!("{gd}character/chara_series_config.cfg.bin"),
        &rdbn(
            "m_charaSeriesInfoList",
            vec![vec![
                ("charaSeriesId", RdbnValue::Hash(SERIES_ID)),
                ("charaSeriesType", RdbnValue::Byte(1)),
                ("charaSeriesNameTextId", RdbnValue::Hash(SERIES_NAME)),
            ]],
        ),
    );
    write(
        root,
        &mut vfs,
        &format!("{gd}character/seasonal_chara_param_0.00.00.00.cfg.bin"),
        &encode_t2b(&[entry(
            "SEASONAL_CHARA_PARAM_DATA_LIST_BEG",
            vec![int(0)],
            vec![],
        )]),
    );

    write(
        root,
        &mut vfs,
        &format!("{gd}skill/skill_config_1.00.00.00.cfg.bin"),
        &encode_rdbn(&[
            RdbnList {
                name: "m_skillOptionInfoList".to_owned(),
                type_name: "OPT".to_owned(),
                rows: vec![RdbnRow {
                    fields: vec![("isTechnic".to_owned(), RdbnValue::Bool(true))],
                }],
            },
            RdbnList {
                name: "m_skillInfoList".to_owned(),
                type_name: "SKILL_INFO".to_owned(),
                rows: vec![RdbnRow {
                    fields: vec![
                        ("skillID".to_owned(), RdbnValue::Hash(SKILL_ID)),
                        (
                            "skillIDStr".to_owned(),
                            RdbnValue::Condition("whs00001".to_owned()),
                        ),
                        ("skillNameId".to_owned(), RdbnValue::Hash(SKILL_NAME)),
                        ("skillDescId".to_owned(), RdbnValue::Hash(SKILL_DESC)),
                        ("element".to_owned(), RdbnValue::Int(1)),
                        ("category".to_owned(), RdbnValue::Int(2)),
                        ("recastTime".to_owned(), RdbnValue::Float(90.5)),
                    ],
                }],
            },
        ])
        .unwrap(),
    );
    // aura: one entry (named in fr only) with one effect; passive: one entry with no text.
    write(
        root,
        &mut vfs,
        &format!("{gd}skill/aura_skill_config_1.00.00.00.cfg.bin"),
        &encode_t2b(&[
            entry(
                "AURA_CMD_INFO_LIST_BEG",
                vec![int(1)],
                vec![
                    entry(
                        "AURA_CMD_INFO",
                        vec![int(AURA_ID), string("wks1"), int(AURA_NAME), Value::Int(0)],
                        vec![],
                    ),
                    pair("AURA_CMD_INFO_REF_EFFECT", 1, 1),
                ],
            ),
            entry(
                "AURA_CMD_EFFECT_LIST_BEG",
                vec![int(2)],
                vec![
                    pair("AURA_CMD_EFFECT", 10, 11),
                    pair("AURA_CMD_EFFECT", 20, 21),
                ],
            ),
        ]),
    );
    write(
        root,
        &mut vfs,
        &format!("{gd}skill/passive_skill_config_1.00.00.00.cfg.bin"),
        &encode_t2b(&[entry(
            "PASSIVE_SKILL_INFO_LIST_BEG",
            vec![int(1)],
            vec![entry(
                "PASSIVE_SKILL_INFO",
                vec![int(0xC000_0003), int(0x8888_0001)],
                vec![],
            )],
        )]),
    );
    for (stem, list) in [
        (
            "change_aura_skill_config_1.00.00.00",
            "m_ChangeAuraSkillInfoList",
        ),
        (
            "override_skill_config_1.00.00.00",
            "m_OverrideSkillInfoList",
        ),
        ("real_skill_config_1.00.00.00", "m_RealSkillInfoList"),
    ] {
        write(
            root,
            &mut vfs,
            &format!("{gd}skill/{stem}.cfg.bin"),
            &rdbn(list, vec![vec![("id", RdbnValue::Hash(7))]]),
        );
    }

    write(
        root,
        &mut vfs,
        &format!("{gd}character/belong_team_config_0.00.00.cfg.bin"),
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
        let mut skill_nouns = vec![noun(SKILL_NAME, 0, skill)];
        if language == "fr" {
            skill_nouns.push(noun(AURA_NAME, 0, "Esprit"));
        }
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
                "chara_text_roma",
                t2b(
                    "NOUN_INFO_BEGIN",
                    vec![noun(NAME_HASH, 0, &format!("{given} {family} (roma)"))],
                ),
            ),
            (
                "chara_description_text",
                t2b("TEXT_INFO_BEGIN", vec![text(DESC_HASH, desc)]),
            ),
            (
                "skill_text",
                encode_t2b(&[
                    entry("NOUN_INFO_BEGIN", vec![int(1)], skill_nouns),
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
                "chara_add_info_text",
                t2b("NOUN_INFO_BEGIN", vec![noun(SERIES_NAME, 0, "Saison")]),
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
