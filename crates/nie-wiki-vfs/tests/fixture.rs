//! Pages computed from a synthetic installation built in a temp directory: no game file is read.

use nie_wiki_vfs::testing::*;
use nie_wiki_vfs::{CharacterClass, ClassRole, Id, Query, SkillFamily, Wiki};
use tempfile::TempDir;

fn wiki(dir: &TempDir) -> Wiki {
    Wiki::new(installation(dir.path())).unwrap()
}

const ALL: &[&str] = &[];

#[test]
fn languages_come_from_the_text_folders_that_hold_character_names() {
    let dir = TempDir::new().unwrap();
    let wiki = wiki(&dir);
    let languages: Vec<_> = wiki.languages().iter().map(|l| l.folder.as_str()).collect();
    assert_eq!(languages, ["de", "fr"]);
}

#[test]
fn every_entry_of_both_character_tables_is_a_record_with_its_class() {
    let dir = TempDir::new().unwrap();
    let wiki = wiki(&dir);
    let page = wiki.characters(&Query::default()).unwrap();
    assert_eq!(page.total, 5);
    let classes: Vec<_> = page.items.iter().map(|c| c.class).collect();
    assert_eq!(
        classes,
        [
            CharacterClass::ParamAndBase,
            CharacterClass::ParamWithoutBase,
            CharacterClass::ParamAndBase,
            CharacterClass::BaseWithoutParam,
            CharacterClass::BaseWithoutParam,
        ]
    );
    let counts: Vec<_> = wiki
        .character_classes()
        .unwrap()
        .iter()
        .map(|c| (c.class, c.count))
        .collect();
    assert_eq!(
        counts,
        [
            (CharacterClass::ParamAndBase, 2),
            (CharacterClass::ParamWithoutBase, 1),
            (CharacterClass::BaseWithoutParam, 2),
        ]
    );

    // The param whose base is missing keeps its own fields and has no base fields.
    let orphan = &page.items[1];
    assert_eq!(orphan.chara_param_id, Some(Id(PARAM_ID + 1)));
    assert_eq!(orphan.chara_base_id, Id(CHARA_ID + 99));
    assert!(
        orphan.internal_code.is_none() && orphan.name.is_none() && orphan.base_source.is_none()
    );
    assert!(orphan.element.is_some());

    // A base no param names has no param fields; the repeated id is its own record.
    let unnamed = &page.items[3];
    assert_eq!(unnamed.chara_param_id, None);
    assert_eq!(unnamed.internal_code.as_deref(), Some("c00000002"));
    assert!(
        unnamed.gender.is_none(),
        "variable 11 is absent from this entry"
    );
    assert!(unnamed.element.is_none() && unnamed.param_source.is_none());
    assert_eq!(page.items[4].internal_code.as_deref(), Some("dup"));

    // Filtering by class and looking records up by each key form.
    let only = Query {
        class: Some("base_without_param".to_owned()),
        ..Query::default()
    };
    assert_eq!(wiki.characters(&only).unwrap().total, 2);
    let by_entry = wiki.character("base-2", ALL).unwrap().unwrap();
    assert_eq!(by_entry.internal_code.as_deref(), Some("dup"));
    assert!(wiki.character("0xB0000002", ALL).unwrap().is_some());
    assert!(wiki.character("c00000002", ALL).unwrap().is_some());
}

#[test]
fn character_joins_param_base_and_labels_per_language() {
    let dir = TempDir::new().unwrap();
    let wiki = wiki(&dir);
    let c = wiki.character("c00000001", ALL).unwrap().unwrap();

    assert_eq!(c.chara_param_id, Some(Id(PARAM_ID)));
    assert_eq!(c.chara_base_id, Id(CHARA_ID));
    // Gender and positions are the stored codes.
    assert_eq!(c.gender.as_ref().map(|g| g.value), Some(2));
    assert_eq!(c.main_position.as_ref().map(|p| p.value), Some(4));
    assert_eq!(c.sub_position.as_ref().map(|p| p.value), Some(2));
    assert!(c.main_position.as_ref().unwrap().label.is_none());
    assert_eq!(c.growth_pattern, Some(7));
    assert_eq!(c.sort_index, Some(2));

    let name = c.name.as_ref().unwrap();
    assert_eq!(name.text("fr"), Some("Aki Kino"));
    let forms: Vec<_> = name.texts["fr"]
        .forms
        .iter()
        .map(|f| (f.form, f.text.as_str()))
        .collect();
    assert_eq!(forms, [(0, "Aki Kino"), (11, "Kino"), (12, "Aki")]);
    assert_eq!(c.family_name.as_ref().unwrap().text("de"), Some("Kino"));
    assert_eq!(
        c.name_roma.as_ref().unwrap().text("de"),
        Some("Aki Kino (roma)")
    );
    assert!(
        c.family_name_roma.is_none(),
        "chara_text_roma holds no row for this key"
    );

    let description = c.description.as_ref().unwrap();
    assert_eq!(description.text("fr"), Some("Description fr\nligne 2"));
    assert_eq!(
        description.texts["fr"].forms[0].raw.as_deref(),
        Some("Description fr\\nligne 2")
    );

    // The element label is the system_text row keyed by the game's enumeration key.
    let element = c.element.as_ref().unwrap();
    assert_eq!(element.value, 1);
    let label = element.label.as_ref().unwrap();
    assert_eq!(
        (label.text("fr"), label.text("de")),
        (Some("Vent"), Some("Wind"))
    );

    // The series is the chara_series_config row, named through chara_add_info_text.
    let series = c.series.as_ref().unwrap();
    assert_eq!(series.series_id, Id(SERIES_ID));
    assert_eq!(series.series_type, Some(1));
    assert_eq!(series.name.as_ref().unwrap().text("fr"), Some("Saison"));

    let team = c.team.as_ref().unwrap();
    assert_eq!(team.belong_team_id, Id(TEAM_ID));
    assert_eq!(team.name.as_ref().unwrap().text("de"), Some("Raimon DE"));

    // The base's battle slice is read from CHARA_BASE_BATTLE.
    let battle = c.battle.as_ref().unwrap();
    assert_eq!((battle.offset, battle.count, battle.items.len()), (0, 2, 2));
    assert_eq!(battle.items[1].index, 1);

    assert_eq!(c.param_source.as_ref().unwrap().table, "CHARA_PARAM_INFO");
    assert_eq!(c.param_source.as_ref().unwrap().index, 0);
    assert!(
        c.param_source
            .as_ref()
            .unwrap()
            .vfs_path
            .ends_with("chara_param_1.00.00.00.cfg.bin")
    );
    assert_eq!(c.base_source.as_ref().unwrap().index, 0);
    assert_eq!(c.param_raw.len(), 16);
}

#[test]
fn learned_skills_resolve_across_the_three_skill_tables_and_say_what_is_missing() {
    let dir = TempDir::new().unwrap();
    let wiki = wiki(&dir);
    let c = wiki
        .character("c00000001", ["fr", "de"].as_slice())
        .unwrap()
        .unwrap();
    assert_eq!(c.skills.len(), 3);

    let skill = &c.skills[0];
    assert_eq!((skill.skill_id, skill.learn_level), (Id(SKILL_ID), 12));
    assert_eq!(skill.family, Some(SkillFamily::Skill));
    assert_eq!(skill.name.as_ref().unwrap().text("fr"), Some("Tir fr"));

    // The aura is named in fr only: the entry and its text key are known, de has no row.
    let aura = &c.skills[1];
    assert_eq!(aura.family, Some(SkillFamily::Aura));
    assert_eq!(aura.name_text_id, Some(Id(AURA_NAME)));
    assert_eq!(aura.entry.as_ref().unwrap().table, "AURA_CMD_INFO");
    let aura_name = aura.name.as_ref().unwrap();
    assert!(aura_name.texts.contains_key("fr") && !aura_name.texts.contains_key("de"));

    // An id that is in no skill table has no family, entry or name.
    let unknown = &c.skills[2];
    assert_eq!(unknown.skill_id, Id(UNKNOWN_SKILL));
    assert!(unknown.family.is_none() && unknown.entry.is_none() && unknown.name.is_none());
}

#[test]
fn every_class_of_every_table_is_served_as_stored() {
    let dir = TempDir::new().unwrap();
    let wiki = wiki(&dir);
    let tables = wiki.tables().unwrap();
    assert_eq!(tables.len(), 12);

    let param = tables.iter().find(|t| t.table == "chara_param").unwrap();
    assert_eq!(param.format, "t2b");
    let class = |name: &str| param.classes.iter().find(|c| c.name == name).unwrap();
    assert_eq!(class("CHARA_PARAM_INFO").entries, 3);
    assert_eq!(class("CHARA_PARAM_INFO").rich_family, Some("characters"));
    assert_eq!(class("__SORT_INDEX").role, ClassRole::SortIndex);
    assert_eq!(class("STORY_FIX_SKILL_INFO").entries, 1);
    assert_eq!(class("STORY_FIX_SKILL_INFO").rich_family, None);

    let base = tables.iter().find(|t| t.table == "chara_base").unwrap();
    let reference = base
        .classes
        .iter()
        .find(|c| c.name == "CHARA_BASE_INFO_REF_BATTLE")
        .unwrap();
    assert_eq!(reference.role, ClassRole::Reference);
    assert_eq!(reference.target.as_deref(), Some("CHARA_BASE_BATTLE"));

    // RDBN tables list every list, including the one no family renders.
    let skill = tables.iter().find(|t| t.table == "skill_config").unwrap();
    assert_eq!(skill.format, "rdbn");
    assert_eq!(skill.classes.len(), 2);
    let option = wiki
        .table_record("skill_config", "m_skillOptionInfoList", 0, ALL)
        .unwrap()
        .unwrap();
    assert_eq!(option.fields.unwrap()["isTechnic"], true);

    // A T2B entry keeps its positional variables, and a slice that leaves the class stays empty.
    let story = wiki
        .table_record("chara_param", "STORY_FIX_SKILL_INFO", 0, ALL)
        .unwrap()
        .unwrap();
    assert_eq!(story.variables.as_ref().unwrap().len(), 2);
    let ids = wiki
        .table_record("chara_base", "CHARA_BASE_INFO", 2, ALL)
        .unwrap()
        .unwrap();
    assert_eq!(ids.refs["CHARA_BASE_BATTLE"].items.len(), 0);
    assert_eq!(ids.refs["CHARA_BASE_BATTLE"].offset, 9);

    // The aura entry carries its effect slice and the label of its name key.
    let aura = wiki
        .table_record("aura_skill_config", "AURA_CMD_INFO", 0, ALL)
        .unwrap()
        .unwrap();
    let effect = &aura.refs["AURA_CMD_EFFECT"];
    assert_eq!((effect.offset, effect.count, effect.items.len()), (1, 1, 1));
    assert_eq!(effect.items[0].variables[0], 20);
    assert_eq!(aura.labels["name"].text("fr"), Some("Esprit"));

    // Lists are windowed and searchable; unknown classes and tables are absent.
    let page = wiki
        .table_records("chara_param", "CHARA_PARAM_INFO", &Query::default())
        .unwrap()
        .unwrap();
    assert_eq!(page.total, 3);
    let hit = Query {
        text: Some("esprit".to_owned()),
        ..Query::default()
    };
    assert_eq!(
        wiki.table_records("aura_skill_config", "AURA_CMD_INFO", &hit)
            .unwrap()
            .unwrap()
            .total,
        1
    );
    assert!(
        wiki.table_records("chara_param", "NOPE", &Query::default())
            .unwrap()
            .is_none()
    );
    assert!(
        wiki.table_records("item_config", "X", &Query::default())
            .unwrap()
            .is_none()
    );
}

#[test]
fn the_catalog_lists_every_gamedata_file_and_what_is_decoded() {
    let dir = TempDir::new().unwrap();
    let wiki = wiki(&dir);
    let catalog = wiki.catalog().unwrap();
    let status = |needle: &str| {
        catalog
            .iter()
            .find(|e| e.vfs_path.contains(needle))
            .map(|e| e.status)
    };
    assert_eq!(status("chara_param_1.00.00.00"), Some("decoded"));
    assert_eq!(status("chara_param_table_config_9"), Some("decoded"));
    assert_eq!(status("unrelated_config"), Some("present"));
    assert!(
        catalog
            .iter()
            .all(|e| e.vfs_path.starts_with("data/common/gamedata/"))
    );
}

#[test]
fn skill_keeps_every_stored_field() {
    let dir = TempDir::new().unwrap();
    let wiki = wiki(&dir);
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
    let wiki = wiki(&dir);
    let team = wiki.team("0xD0000001", ALL).unwrap().unwrap();
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
    // Every base entry naming the team: the first and the repeated id (the second is too short).
    assert_eq!(team.affiliated_characters.len(), 2);
    assert_eq!(team.affiliated_characters[0].internal_code, "c00000001");
    assert_eq!(team.roster.family, "team roster");
    assert_eq!(wiki.teams(&Query::default()).unwrap().total, 1);
    assert!(wiki.team("0xDEADBEEF", ALL).unwrap().is_none());
}

#[test]
fn languages_resolve_from_folders_and_bcp47_tags() {
    let dir = TempDir::new().unwrap();
    let wiki = wiki(&dir);
    assert_eq!(
        wiki.resolve_languages(&["FR", "fr", "de"]).unwrap(),
        ["fr", "de"]
    );
    assert_eq!(wiki.resolve_languages(ALL).unwrap(), ["de", "fr"]);
    assert!(wiki.resolve_languages(&["xx"]).is_err());
    assert!(wiki.character("c00000001", &["zh-Hans"]).is_err());
    assert_eq!(wiki.unsourced().len(), 3);
}

#[test]
fn lists_are_filtered_by_name_in_the_selected_languages_and_windowed() {
    let dir = TempDir::new().unwrap();
    let wiki = wiki(&dir);
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

    // Characters match on names, romanized names, the code and the id. Every record whose base
    // entry carries the name key matches: three bases and the two params that name the first.
    assert_eq!(wiki.characters(&query("kino", &["de"])).unwrap().total, 4);
    assert_eq!(wiki.characters(&query("(roma)", &["de"])).unwrap().total, 4);
    assert_eq!(
        wiki.characters(&query("c0000000", &["de"])).unwrap().total,
        3
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
    assert_eq!((page.total, page.items.len()), (5, 0));
    let past = Query {
        offset: 5,
        ..Query::default()
    };
    assert!(wiki.skills(&past).unwrap().items.is_empty());
}

#[test]
fn sources_name_every_class_of_every_backing_table() {
    let dir = TempDir::new().unwrap();
    let wiki = wiki(&dir);
    let sources = wiki.sources().unwrap();
    let find = |table: &str| sources.iter().find(|s| s.table == table).unwrap();
    assert_eq!(
        (
            find("CHARA_PARAM_INFO").family,
            find("CHARA_PARAM_INFO").entries
        ),
        ("characters", 3)
    );
    assert_eq!(
        (
            find("CHARA_BASE_INFO").family,
            find("CHARA_BASE_INFO").entries
        ),
        ("characters", 3)
    );
    assert_eq!(find("m_skillInfoList").family, "skills");
    assert_eq!(find("m_skillOptionInfoList").family, "tables");
    assert_eq!(find("m_belongTeamInfoList").family, "teams");
    assert_eq!(find("PASSIVE_SKILL_INFO").family, "tables");
}

#[test]
fn pages_show_provenance_emblems_and_explicit_empty_states() {
    use nie_wiki_vfs::html::{self, Ctx, ListParams};

    let dir = TempDir::new().unwrap();
    let wiki = wiki(&dir);
    let languages = wiki.languages();
    let fr = languages.iter().find(|l| l.folder == "fr").unwrap();
    let ctx = |rest| Ctx {
        languages,
        lang: fr,
        rest,
    };

    let team = wiki.team("0xD0000001", ALL).unwrap().unwrap();
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

    let character = wiki.character("c00000001", ALL).unwrap().unwrap();
    let page = html::character(&ctx("characters/0xB0000001"), &character);
    assert!(page.contains("<h1>Aki Kino</h1>"));
    assert!(page.contains("stored code, no label in the VFS"));
    assert!(page.contains("chara_param_1.00.00.00.cfg.bin"));
    assert!(page.contains("Raimon"));
    // Learned skills: a skill page, an aura table entry, and an id no table holds.
    assert!(page.contains("href=\"/wiki/fr/skills/0xC0000001\""));
    assert!(page.contains("href=\"/wiki/fr/tables/aura_skill_config/AURA_CMD_INFO/0\""));
    assert!(page.contains(
        "no entry with this id in skill_config, aura_skill_config or passive_skill_config"
    ));

    // The character with no param names the missing param and shows its base entry only.
    let base_only = wiki.character("base-1", ALL).unwrap().unwrap();
    let page = html::character(&ctx("characters/base-1"), &base_only);
    assert!(page.contains("no chara_param entry names this chara_base entry"));
    assert!(page.contains("absent from the entry"));

    let list = wiki.characters(&Query::default()).unwrap();
    let params = ListParams {
        text: Some("<b>".to_owned()),
        offset: 0,
        limit: 10,
    };
    let classes = wiki.character_classes().unwrap();
    let page = html::characters(&ctx("characters"), &list, &params, &classes);
    assert!(page.contains("value=\"&lt;b&gt;\""));
    assert!(!page.contains("<b>"));
    assert!(page.contains("5 match(es)"));
    assert!(page.contains("href=\"/wiki/fr/characters/base-1\""));

    let skill = wiki.skill("whs00001", &["fr"]).unwrap().unwrap();
    let page = html::skill(&ctx("skills/whs00001"), &skill);
    assert!(page.contains("invocation rates</strong>: no source in the VFS"));
    assert!(page.contains("Tir fr"));

    let record = wiki
        .table_record("aura_skill_config", "AURA_CMD_INFO", 0, ALL)
        .unwrap()
        .unwrap();
    let page = html::table_record(&ctx("tables/aura_skill_config/AURA_CMD_INFO/0"), &record);
    assert!(page.contains("The file stores no field names"));
    assert!(page.contains("Esprit"));
    let tables = html::tables(&ctx("tables"), &wiki.tables().unwrap());
    assert!(tables.contains("href=\"/wiki/fr/tables/chara_param/STORY_FIX_SKILL_INFO\""));

    let page = html::index(
        languages,
        &wiki.sources().unwrap(),
        &classes,
        wiki.unsourced(),
        &wiki.catalog().unwrap(),
    );
    assert!(page.contains("capsule lot contents</strong>: no source in the VFS"));
    assert!(page.contains("base without param"));
    assert!(page.contains("unrelated_config_1.00.cfg.bin"));
    assert!(page.contains("href=\"/wiki/de/characters\""));
}
