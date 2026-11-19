use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};

use nie_data::cfgbin::{Node, Var, list_values, walk_named};
use nie_data::chara_base::{CharaBase, parse_base_node};
use nie_data::chara_param::CharaParam;
use nie_formats::vfs::Vfs;
use serde::Serialize;
use serde_json::Value;

use crate::error::{Error, Result};
use crate::index::{self, find_table, name_hash};
use crate::model::{
    AffiliatedCharacter, Character, Coded, Emblem, Id, Language, LearnedSkill, Localized,
    Provenance, Skill, Team, TeamRef, UNSOURCED,
};
use crate::text::{Section, TextSet, TextTable, record_name};

const CHARA_PARAM: &str = "chara_param";
const CHARA_BASE: &str = "chara_base";
const SKILL_CONFIG: &str = "skill_config";
const BELONG_TEAM: &str = "belong_team_config";
const SKILL_LIST: &str = "m_skillInfoList";
const TEAM_LIST: &str = "m_belongTeamInfoList";
const PARAM_NODE: &str = "CHARA_PARAM_INFO";
const BASE_NODE: &str = "CHARA_BASE_INFO";
const EMBLEM_FIELD_PREFIX: &str = "teamEmblemId";
/// `chara_base` variable positions (nie-data `chara_base`).
const BASE_GENDER_VAR: usize = 11;
/// The element enumeration keys the game hashes to find its names in `system_text`
/// (`crc32("element_type_wind")` is the key of the NOUN_INFO row "Vent"/"Wind"/"風").
const ELEMENT_KEYS: [(i64, &str); 6] = [
    (0, "none"),
    (1, "wind"),
    (2, "forest"),
    (3, "fire"),
    (4, "mountain"),
    (5, "void"),
];

fn decode(vfs: &Vfs, path: &str) -> Result<Value> {
    nie_explore::game_data::decode_cfgbin(vfs, path).map_err(|message| Error::Decode {
        path: path.to_owned(),
        message,
    })
}

fn locked<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A table decoded once per `Wiki`.
struct Lazy<T>(Mutex<Option<Arc<T>>>);

impl<T> Lazy<T> {
    fn new() -> Self {
        Self(Mutex::new(None))
    }

    fn get(&self, load: impl FnOnce() -> Result<T>) -> Result<Arc<T>> {
        let mut slot = locked(&self.0);
        if let Some(value) = slot.as_ref() {
            return Ok(Arc::clone(value));
        }
        let value = Arc::new(load()?);
        *slot = Some(Arc::clone(&value));
        Ok(value)
    }
}

struct BaseRow {
    base: CharaBase,
    gender: Option<i64>,
    raw: Vec<Value>,
    index: usize,
}

struct CharaData {
    param_path: String,
    base_path: String,
    params: Vec<(CharaParam, usize)>,
    bases: HashMap<u32, BaseRow>,
}

struct RdbnRows {
    path: String,
    rows: Vec<Value>,
}

fn var_json(var: &Var<'_>) -> Value {
    match var.ty {
        "String" => Value::from(var.value),
        "Float" => Value::from(var.as_f64()),
        _ => Value::from(var.as_i64()),
    }
}

fn load_chara(vfs: &Vfs) -> Result<CharaData> {
    let param_path = find_table(vfs, CHARA_PARAM)?;
    let base_path = find_table(vfs, CHARA_BASE)?;

    let mut params = Vec::new();
    let mut index = 0;
    walk_named(&decode(vfs, &param_path)?, PARAM_NODE, |node: Node<'_>| {
        if node.name().contains("_LIST_") {
            return;
        }
        if let Some(param) = CharaParam::from_node(node) {
            params.push((param, index));
        }
        index += 1;
    });

    let mut bases = HashMap::new();
    let mut index = 0;
    walk_named(&decode(vfs, &base_path)?, BASE_NODE, |node: Node<'_>| {
        if record_name(node.name()) != BASE_NODE {
            return;
        }
        if let Some(base) = parse_base_node(&node) {
            let gender = node
                .var(BASE_GENDER_VAR)
                .filter(|var| var.ty == "Int")
                .map(|var| var.as_i64());
            let raw = (0..node.var_count())
                .filter_map(|i| node.var(i))
                .map(|var| var_json(&var))
                .collect();
            bases.entry(base.chara_id.0).or_insert(BaseRow {
                base,
                gender,
                raw,
                index,
            });
        }
        index += 1;
    });

    Ok(CharaData {
        param_path,
        base_path,
        params,
        bases,
    })
}

fn load_rdbn(vfs: &Vfs, stem: &str, list: &str) -> Result<RdbnRows> {
    let path = find_table(vfs, stem)?;
    let rows = list_values(&decode(vfs, &path)?, list)
        .cloned()
        .unwrap_or_default();
    Ok(RdbnRows { path, rows })
}

fn field_id(row: &Value, key: &str) -> Option<u32> {
    let id = nie_data::cfgbin::field_hash(row, key);
    (!id.is_zero()).then_some(id.0)
}

fn fields_of(row: &Value) -> BTreeMap<String, Value> {
    row.as_object()
        .map(|map| map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default()
}

/// Emblem files and affiliations of every team, computed once.
struct TeamIndex {
    files: HashMap<u32, Vec<String>>,
    affiliated: HashMap<u32, Vec<AffiliatedCharacter>>,
}

/// What a list route asks for: the languages of the labels, a text filter, a window.
#[derive(Debug, Clone, Default)]
pub struct Query {
    /// Language folders or BCP 47 tags; empty means every language.
    pub languages: Vec<String>,
    /// Case-insensitive substring searched in names (every selected language) and codes.
    pub text: Option<String>,
    pub offset: usize,
    /// `None` returns every match.
    pub limit: Option<usize>,
}

/// A window of matches: `total` counts all of them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Page<T> {
    pub total: usize,
    pub offset: usize,
    pub items: Vec<T>,
}

/// A table that backs a page family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Source {
    pub family: &'static str,
    pub vfs_path: String,
    pub table: &'static str,
    pub entries: usize,
}

/// The wiki over one mounted installation.
pub struct Wiki {
    vfs: Arc<Vfs>,
    languages: Vec<Language>,
    text_tables: Mutex<HashMap<(String, String), Arc<TextTable>>>,
    chara: Lazy<CharaData>,
    skills: Lazy<RdbnRows>,
    teams: Lazy<RdbnRows>,
    team_index: Lazy<TeamIndex>,
}

impl Wiki {
    /// Mounts the installation named by `source`, or discovered from the environment
    /// (`NIE_GAME_SOURCE`) when `None`.
    ///
    /// # Errors
    /// The source cannot be mounted or ships no text language.
    pub fn open(source: Option<&str>) -> Result<Self> {
        let vfs = nie_formats::vfs::open_source(source).map_err(|e| Error::Mount(e.to_string()))?;
        Self::new(vfs)
    }

    /// Wraps an already mounted VFS, owned or shared.
    ///
    /// # Errors
    /// The VFS ships no text language.
    pub fn new(vfs: impl Into<Arc<Vfs>>) -> Result<Self> {
        let vfs = vfs.into();
        let languages = index::languages(&vfs)?;
        Ok(Self {
            vfs,
            languages,
            text_tables: Mutex::new(HashMap::new()),
            chara: Lazy::new(),
            skills: Lazy::new(),
            teams: Lazy::new(),
            team_index: Lazy::new(),
        })
    }

    /// Every language the installation ships.
    #[must_use]
    pub fn languages(&self) -> &[Language] {
        &self.languages
    }

    /// The language folders named by `spec` (folders or BCP 47 tags, any case, `-` or `_`);
    /// every language when `spec` is empty.
    ///
    /// # Errors
    /// An entry names no shipped language.
    pub fn resolve_languages<S: AsRef<str>>(&self, spec: &[S]) -> Result<Vec<String>> {
        if spec.is_empty() {
            return Ok(self.languages.iter().map(|l| l.folder.clone()).collect());
        }
        let mut folders: Vec<String> = Vec::new();
        for entry in spec {
            let wanted = entry.as_ref().trim().replace('-', "_").to_lowercase();
            let found = self
                .languages
                .iter()
                .find(|l| l.folder.to_lowercase() == wanted)
                .ok_or_else(|| Error::UnknownLanguage(entry.as_ref().to_owned()))?;
            if !folders.contains(&found.folder) {
                folders.push(found.folder.clone());
            }
        }
        Ok(folders)
    }

    /// The families the VFS gives no source for.
    #[must_use]
    pub fn unsourced(&self) -> &'static [crate::NoSource] {
        &UNSOURCED
    }

    /// The tables behind the page families, with their entry counts.
    ///
    /// # Errors
    /// A table cannot be found or decoded.
    pub fn sources(&self) -> Result<Vec<Source>> {
        let chara = self.chara_data()?;
        let skills = self.skill_rows()?;
        let teams = self.team_rows()?;
        Ok(vec![
            Source {
                family: "characters",
                vfs_path: chara.param_path.clone(),
                table: PARAM_NODE,
                entries: chara.params.len(),
            },
            Source {
                family: "characters",
                vfs_path: chara.base_path.clone(),
                table: BASE_NODE,
                entries: chara.bases.len(),
            },
            Source {
                family: "skills",
                vfs_path: skills.path.clone(),
                table: SKILL_LIST,
                entries: skills.rows.len(),
            },
            Source {
                family: "teams",
                vfs_path: teams.path.clone(),
                table: TEAM_LIST,
                entries: teams.rows.len(),
            },
        ])
    }

    fn table(&self, language: &str, stem: &str) -> Result<Option<Arc<TextTable>>> {
        let key = (language.to_owned(), stem.to_owned());
        if let Some(table) = locked(&self.text_tables).get(&key) {
            return Ok(Some(Arc::clone(table)));
        }
        if self
            .vfs
            .find_any(&index::text_path(language, stem))
            .is_none()
        {
            return Ok(None);
        }
        let table = Arc::new(TextTable::load(&self.vfs, language, stem)?);
        locked(&self.text_tables).insert(key, Arc::clone(&table));
        Ok(Some(table))
    }

    fn texts<'a>(&'a self, stem: &'a str, langs: &'a [String]) -> Result<TextSet<'a>> {
        let mut tables = Vec::new();
        for language in langs {
            if let Some(table) = self.table(language, stem)? {
                tables.push((language.as_str(), table));
            }
        }
        Ok(TextSet::new(stem, tables))
    }

    fn element(&self, value: i64, names: &TextSet<'_>) -> Coded {
        let label = ELEMENT_KEYS
            .iter()
            .find(|(code, _)| *code == value)
            .and_then(|(_, key)| {
                names.localize(Section::Noun, name_hash(&format!("element_type_{key}")))
            });
        Coded { value, label }
    }

    fn chara_data(&self) -> Result<Arc<CharaData>> {
        self.chara.get(|| load_chara(&self.vfs))
    }

    fn skill_rows(&self) -> Result<Arc<RdbnRows>> {
        self.skills
            .get(|| load_rdbn(&self.vfs, SKILL_CONFIG, SKILL_LIST))
    }

    fn team_rows(&self) -> Result<Arc<RdbnRows>> {
        self.teams
            .get(|| load_rdbn(&self.vfs, BELONG_TEAM, TEAM_LIST))
    }

    fn team_index(&self) -> Result<Arc<TeamIndex>> {
        self.team_index.get(|| {
            let rows = self.team_rows()?;
            let wanted: HashSet<u32> = rows
                .rows
                .iter()
                .flat_map(|row| emblem_ids(row).into_iter().map(|(_, id)| id))
                .collect();
            let files = index::files_by_name_hash(&self.vfs, &wanted);

            let data = self.chara_data()?;
            let mut affiliated: HashMap<u32, Vec<AffiliatedCharacter>> = HashMap::new();
            for row in data.bases.values() {
                if let Some(team) = row.base.belong_team_id {
                    affiliated
                        .entry(team.0)
                        .or_default()
                        .push(AffiliatedCharacter {
                            chara_base_id: Id(row.base.chara_id.0),
                            internal_code: row.base.internal_code.clone(),
                            source: Provenance {
                                vfs_path: data.base_path.clone(),
                                table: BASE_NODE.to_owned(),
                                index: row.index,
                            },
                        });
                }
            }
            for members in affiliated.values_mut() {
                members.sort_by_key(|m| m.source.index);
            }
            Ok(TeamIndex { files, affiliated })
        })
    }

    /// `chara_param` entries joined to their `chara_base` entry, in table order, filtered and
    /// windowed by `query`. Entries whose `chara_base` entry is missing are skipped.
    ///
    /// # Errors
    /// A table cannot be found or decoded, or a language is unknown.
    pub fn characters(&self, query: &Query) -> Result<Page<Character>> {
        let langs = self.resolve_languages(&query.languages)?;
        let data = self.chara_data()?;
        let ctx = CharaContext::new(self, &langs)?;
        let needle = query.text.as_deref().map(str::to_lowercase);
        let hits: Vec<&(CharaParam, usize)> = data
            .params
            .iter()
            .filter(|(param, _)| {
                data.bases.get(&param.chara_base_id.0).is_some_and(|row| {
                    needle
                        .as_deref()
                        .is_none_or(|needle| ctx.matches(param, row, needle))
                })
            })
            .collect();
        Ok(window(&hits, query, |(param, index)| {
            ctx.build(&data, param, *index)
        }))
    }

    /// The character whose `chara_param` id (`0xXXXXXXXX`) or `chara_base` internal code is `key`.
    ///
    /// # Errors
    /// A table cannot be found or decoded, or a language is unknown.
    pub fn character<S: AsRef<str>>(
        &self,
        key: &str,
        languages: &[S],
    ) -> Result<Option<Character>> {
        let langs = self.resolve_languages(languages)?;
        let data = self.chara_data()?;
        let id = nie_data::hash::HashId::parse(key).map(|h| h.0);
        let found = data.params.iter().find(|(param, _)| {
            Some(param.chara_param_id.0) == id
                || data
                    .bases
                    .get(&param.chara_base_id.0)
                    .is_some_and(|row| row.base.internal_code == key)
        });
        let Some((param, index)) = found else {
            return Ok(None);
        };
        Ok(CharaContext::new(self, &langs)?.build(&data, param, *index))
    }

    /// `skill_config` entries in table order, filtered and windowed by `query`.
    ///
    /// # Errors
    /// A table cannot be found or decoded, or a language is unknown.
    pub fn skills(&self, query: &Query) -> Result<Page<Skill>> {
        let langs = self.resolve_languages(&query.languages)?;
        let rows = self.skill_rows()?;
        let ctx = SkillContext::new(self, &langs)?;
        let needle = query.text.as_deref().map(str::to_lowercase);
        let hits: Vec<(usize, &Value)> = rows
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                skill_key(row).is_some()
                    && needle
                        .as_deref()
                        .is_none_or(|needle| ctx.matches(row, needle))
            })
            .collect();
        Ok(window(&hits, query, |(index, row)| {
            ctx.build(&rows.path, row, *index)
        }))
    }

    /// The skill whose `skillID` (`0xXXXXXXXX`) or `skillIDStr` is `key`.
    ///
    /// # Errors
    /// A table cannot be found or decoded, or a language is unknown.
    pub fn skill<S: AsRef<str>>(&self, key: &str, languages: &[S]) -> Result<Option<Skill>> {
        let langs = self.resolve_languages(languages)?;
        let rows = self.skill_rows()?;
        let id = nie_data::hash::HashId::parse(key).map(|h| h.0);
        let found = rows.rows.iter().enumerate().find(|(_, row)| {
            (id.is_some() && field_id(row, "skillID") == id)
                || row.get("skillIDStr").and_then(Value::as_str) == Some(key)
        });
        let Some((index, row)) = found else {
            return Ok(None);
        };
        Ok(SkillContext::new(self, &langs)?.build(&rows.path, row, index))
    }

    /// `belong_team_config` entries in table order, filtered and windowed by `query`, with
    /// emblems resolved by file-name hash.
    ///
    /// # Errors
    /// A table cannot be found or decoded, or a language is unknown.
    pub fn teams(&self, query: &Query) -> Result<Page<Team>> {
        let langs = self.resolve_languages(&query.languages)?;
        let rows = self.team_rows()?;
        let names = self.texts("team_text", &langs)?;
        let needle = query.text.as_deref().map(str::to_lowercase);
        let hits: Vec<(usize, &Value)> = rows
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                field_id(row, "belongTeamId").is_some()
                    && needle.as_deref().is_none_or(|needle| {
                        field_id(row, "teamNameTextId")
                            .is_some_and(|id| names.matches(Section::Any, id, needle))
                    })
            })
            .collect();
        let index = self.team_index()?;
        Ok(window(&hits, query, |(position, row)| {
            build_team(&rows.path, row, *position, &names, &index)
        }))
    }

    /// The team whose `belongTeamId` is `key` (`0xXXXXXXXX`).
    ///
    /// # Errors
    /// A table cannot be found or decoded, or a language is unknown.
    pub fn team<S: AsRef<str>>(&self, key: &str, languages: &[S]) -> Result<Option<Team>> {
        let langs = self.resolve_languages(languages)?;
        let Some(id) = nie_data::hash::HashId::parse(key).map(|h| h.0) else {
            return Ok(None);
        };
        let rows = self.team_rows()?;
        let Some((position, row)) = rows
            .rows
            .iter()
            .enumerate()
            .find(|(_, row)| field_id(row, "belongTeamId") == Some(id))
        else {
            return Ok(None);
        };
        let names = self.texts("team_text", &langs)?;
        let index = self.team_index()?;
        Ok(build_team(&rows.path, row, position, &names, &index))
    }
}

/// The matches in `[offset, offset + limit)`, each built by `build` (builds that yield `None`
/// are dropped from the window but still count in `total`).
fn window<H, T>(hits: &[H], query: &Query, build: impl Fn(&H) -> Option<T>) -> Page<T> {
    let items = hits
        .iter()
        .skip(query.offset)
        .take(query.limit.unwrap_or(usize::MAX))
        .filter_map(build)
        .collect();
    Page {
        total: hits.len(),
        offset: query.offset,
        items,
    }
}

fn skill_key(row: &Value) -> Option<(u32, &str)> {
    Some((field_id(row, "skillID")?, row.get("skillIDStr")?.as_str()?))
}

fn build_team(
    path: &str,
    row: &Value,
    position: usize,
    names: &TextSet<'_>,
    index: &TeamIndex,
) -> Option<Team> {
    let belong_team_id = field_id(row, "belongTeamId")?;
    Some(Team {
        belong_team_id: Id(belong_team_id),
        name: field_id(row, "teamNameTextId").and_then(|id| names.localize(Section::Any, id)),
        emblems: emblem_ids(row)
            .into_iter()
            .map(|(field, id)| {
                let files = index.files.get(&id).cloned().unwrap_or_default();
                (
                    field,
                    Emblem {
                        emblem_id: Id(id),
                        files,
                    },
                )
            })
            .collect(),
        affiliated_characters: index
            .affiliated
            .get(&belong_team_id)
            .cloned()
            .unwrap_or_default(),
        fields: fields_of(row),
        roster: UNSOURCED[0],
        source: Provenance {
            vfs_path: path.to_owned(),
            table: TEAM_LIST.to_owned(),
            index: position,
        },
    })
}

/// The non-zero `teamEmblemId_*` fields of a team entry, by field name.
fn emblem_ids(row: &Value) -> Vec<(String, u32)> {
    row.as_object()
        .into_iter()
        .flatten()
        .filter(|(field, _)| field.starts_with(EMBLEM_FIELD_PREFIX))
        .filter_map(|(field, _)| Some((field.clone(), field_id(row, field)?)))
        .collect()
}

struct CharaContext<'a> {
    wiki: &'a Wiki,
    names: TextSet<'a>,
    descriptions: TextSet<'a>,
    system: TextSet<'a>,
    skill_names: SkillContext<'a>,
    teams: HashMap<u32, Option<u32>>,
    team_names: TextSet<'a>,
}

impl<'a> CharaContext<'a> {
    fn new(wiki: &'a Wiki, langs: &'a [String]) -> Result<Self> {
        let teams = wiki
            .team_rows()?
            .rows
            .iter()
            .filter_map(|row| {
                Some((
                    field_id(row, "belongTeamId")?,
                    field_id(row, "teamNameTextId"),
                ))
            })
            .collect();
        Ok(Self {
            wiki,
            names: wiki.texts("chara_text", langs)?,
            descriptions: wiki.texts("chara_description_text", langs)?,
            system: wiki.texts("system_text", langs)?,
            skill_names: SkillContext::new(wiki, langs)?,
            teams,
            team_names: wiki.texts("team_text", langs)?,
        })
    }

    fn matches(&self, param: &CharaParam, row: &BaseRow, needle: &str) -> bool {
        let base = &row.base;
        base.internal_code.to_lowercase().contains(needle)
            || format!("0x{:08x}", param.chara_param_id.0).contains(needle)
            || self.names.matches(Section::Noun, base.name_hash.0, needle)
            || base
                .last_name_hash
                .is_some_and(|h| self.names.matches(Section::Noun, h.0, needle))
    }

    fn build(&self, data: &CharaData, param: &CharaParam, index: usize) -> Option<Character> {
        let row = data.bases.get(&param.chara_base_id.0)?;
        let base = &row.base;
        Some(Character {
            chara_param_id: Id(param.chara_param_id.0),
            chara_base_id: Id(base.chara_id.0),
            internal_code: base.internal_code.clone(),
            name: self.names.localize(Section::Noun, base.name_hash.0),
            family_name: base
                .last_name_hash
                .and_then(|h| self.names.localize(Section::Noun, h.0)),
            description: base
                .description_hash
                .and_then(|h| self.descriptions.localize(Section::Text, h.0)),
            gender: row.gender.map(|value| Coded { value, label: None }),
            element: self.wiki.element(param.element, &self.system),
            main_position: Coded {
                value: param.main_position,
                label: None,
            },
            sub_position: Coded {
                value: param.sub_position,
                label: None,
            },
            growth_pattern: param.growth_pattern,
            series_id: base.series_id.map(|h| Id(h.0)),
            team: base.belong_team_id.and_then(|team| {
                let name = self.teams.get(&team.0)?;
                Some(TeamRef {
                    belong_team_id: Id(team.0),
                    name: name.and_then(|id| self.team_names.localize(Section::Any, id)),
                })
            }),
            skills: param
                .skills
                .iter()
                .map(|skill| LearnedSkill {
                    skill_id: Id(skill.skill_id.0),
                    learn_level: skill.learn_level,
                    name: self.skill_names.name(skill.skill_id.0),
                })
                .collect(),
            param_raw: param.raw_variables.clone(),
            base_raw: row.raw.clone(),
            param_source: Provenance {
                vfs_path: data.param_path.clone(),
                table: PARAM_NODE.to_owned(),
                index,
            },
            base_source: Provenance {
                vfs_path: data.base_path.clone(),
                table: BASE_NODE.to_owned(),
                index: row.index,
            },
        })
    }
}

struct SkillContext<'a> {
    wiki: &'a Wiki,
    names: TextSet<'a>,
    descriptions: TextSet<'a>,
    system: TextSet<'a>,
    name_ids: HashMap<u32, u32>,
}

impl<'a> SkillContext<'a> {
    fn new(wiki: &'a Wiki, langs: &'a [String]) -> Result<Self> {
        let name_ids = wiki
            .skill_rows()?
            .rows
            .iter()
            .filter_map(|row| Some((field_id(row, "skillID")?, field_id(row, "skillNameId")?)))
            .collect();
        Ok(Self {
            wiki,
            names: wiki.texts("skill_text", langs)?,
            descriptions: wiki.texts("skill_text", langs)?,
            system: wiki.texts("system_text", langs)?,
            name_ids,
        })
    }

    fn name(&self, skill_id: u32) -> Option<Localized> {
        self.names
            .localize(Section::Noun, *self.name_ids.get(&skill_id)?)
    }

    fn matches(&self, row: &Value, needle: &str) -> bool {
        skill_key(row).is_some_and(|(_, code)| code.to_lowercase().contains(needle))
            || field_id(row, "skillNameId")
                .is_some_and(|id| self.names.matches(Section::Noun, id, needle))
    }

    fn build(&self, path: &str, row: &Value, index: usize) -> Option<Skill> {
        let (skill_id, skill_id_str) = skill_key(row)?;
        let code = |key: &str| nie_data::cfgbin::field_i64(row, key);
        Some(Skill {
            skill_id: Id(skill_id),
            skill_id_str: skill_id_str.to_owned(),
            name: field_id(row, "skillNameId")
                .and_then(|id| self.names.localize(Section::Noun, id)),
            description: field_id(row, "skillDescId")
                .and_then(|id| self.descriptions.localize(Section::Text, id)),
            element: code("element").map(|value| self.wiki.element(value, &self.system)),
            category: code("category").map(|value| Coded { value, label: None }),
            fields: fields_of(row),
            source: Provenance {
                vfs_path: path.to_owned(),
                table: SKILL_LIST.to_owned(),
                index,
            },
        })
    }
}
