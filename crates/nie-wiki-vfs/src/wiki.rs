use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};

use nie_data::chara_param::{MAX_SKILL_SLOTS, SKILL_BLOCK_START};
use nie_formats::vfs_read::VfsRead;
use serde::Serialize;
use serde_json::Value;

use crate::error::{Error, Result};
use crate::index::{self, name_hash};
use crate::model::{
    AffiliatedCharacter, CatalogEntry, Character, CharacterClass, ClassCount, ClassInfo, Coded,
    Emblem, Id, Language, LearnedSkill, Localized, Provenance, SeriesRef, Skill, SkillFamily,
    TableInfo, TableRecord, Team, TeamRef, UNSOURCED,
};
use crate::tables::{self, Rec, Table};
use crate::text::{Section, TextSet, TextTable};

const SKILL_LIST: &str = "m_skillInfoList";
const TEAM_LIST: &str = "m_belongTeamInfoList";
const SERIES_LIST: &str = "m_charaSeriesInfoList";
const PARAM_NODE: &str = "CHARA_PARAM_INFO";
const BASE_NODE: &str = "CHARA_BASE_INFO";
const BATTLE_NODE: &str = "CHARA_BASE_BATTLE";
const AURA_NODE: &str = "AURA_CMD_INFO";
const PASSIVE_NODE: &str = "PASSIVE_SKILL_INFO";
const EMBLEM_FIELD_PREFIX: &str = "teamEmblemId";
const GAMEDATA_FILE_MARK: &str = ".cfg";

/// `chara_param` variable positions (nie-data `chara_param`).
const PARAM_ID: usize = 0;
const PARAM_BASE: usize = 1;
const PARAM_ELEMENT: usize = 2;
const PARAM_MAIN_POSITION: usize = 3;
const PARAM_SUB_POSITION: usize = 4;
const PARAM_GROWTH: usize = 8;
/// `chara_base` variable positions (nie-data `chara_base`, verified against the real table).
const BASE_ID: usize = 0;
const BASE_CODE: usize = 1;
const BASE_NAME: usize = 3;
const BASE_FAMILY: usize = 4;
const BASE_FAMILY_VARIANT: usize = 5;
const BASE_GENDER: usize = 11;
const BASE_SERIES: usize = 15;
const BASE_TEAM: usize = 16;
const BASE_DESCRIPTION: usize = 19;

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

/// Where a table's text references point; each is verified against the real installation by the
/// share of distinct keys the text file holds.
enum Sel {
    Var(usize),
    Field(&'static str),
}

struct TextRef {
    table: &'static str,
    class: &'static str,
    sel: Sel,
    text: &'static str,
    section: Section,
    label: &'static str,
}

const fn text_ref(
    table: &'static str,
    class: &'static str,
    sel: Sel,
    text: &'static str,
    section: Section,
    label: &'static str,
) -> TextRef {
    TextRef {
        table,
        class,
        sel,
        text,
        section,
        label,
    }
}

const TEXT_REFS: &[TextRef] = &[
    text_ref(
        "chara_base",
        BASE_NODE,
        Sel::Var(BASE_NAME),
        "chara_text",
        Section::Noun,
        "name",
    ),
    text_ref(
        "chara_base",
        BASE_NODE,
        Sel::Var(BASE_FAMILY),
        "chara_text",
        Section::Noun,
        "family_name",
    ),
    text_ref(
        "chara_base",
        BASE_NODE,
        Sel::Var(BASE_FAMILY_VARIANT),
        "chara_text",
        Section::Noun,
        "family_name_variant",
    ),
    text_ref(
        "chara_base",
        BASE_NODE,
        Sel::Var(BASE_DESCRIPTION),
        "chara_description_text",
        Section::Text,
        "description",
    ),
    text_ref(
        "chara_series_config",
        SERIES_LIST,
        Sel::Field("charaSeriesNameTextId"),
        "chara_add_info_text",
        Section::Noun,
        "name",
    ),
    text_ref(
        "skill_config",
        SKILL_LIST,
        Sel::Field("skillNameId"),
        "skill_text",
        Section::Noun,
        "name",
    ),
    text_ref(
        "skill_config",
        SKILL_LIST,
        Sel::Field("skillDescId"),
        "skill_text",
        Section::Text,
        "description",
    ),
    text_ref(
        "passive_skill_config",
        PASSIVE_NODE,
        Sel::Var(1),
        "skill_text",
        Section::Noun,
        "name",
    ),
    text_ref(
        "aura_skill_config",
        AURA_NODE,
        Sel::Var(2),
        "skill_text",
        Section::Noun,
        "name",
    ),
    text_ref(
        "aura_skill_config",
        AURA_NODE,
        Sel::Var(3),
        "skill_text",
        Section::Text,
        "description",
    ),
    text_ref(
        "belong_team_config",
        TEAM_LIST,
        Sel::Field("teamNameTextId"),
        "team_text",
        Section::Any,
        "name",
    ),
];

/// The tables the wiki serves, with the classes a page family renders richly.
const REGISTRY: &[(&str, &[(&str, &str)])] = &[
    (
        "chara_param",
        &[(PARAM_NODE, "characters"), ("__SORT_INDEX", "characters")],
    ),
    (
        "chara_base",
        &[
            (BASE_NODE, "characters"),
            ("CHARA_BASE_INFO_REF_BATTLE", "characters"),
        ],
    ),
    ("chara_series_config", &[]),
    ("seasonal_chara_param", &[]),
    ("chara_param_table_config", &[]),
    ("skill_config", &[(SKILL_LIST, "skills")]),
    ("passive_skill_config", &[]),
    ("aura_skill_config", &[]),
    ("change_aura_skill_config", &[]),
    ("override_skill_config", &[]),
    ("real_skill_config", &[]),
    ("belong_team_config", &[(TEAM_LIST, "teams")]),
];

fn locked<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A value computed once per `Wiki`.
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

fn int(vars: &[Value], i: usize) -> Option<i64> {
    vars.get(i)?.as_i64()
}

fn hash(vars: &[Value], i: usize) -> Option<u32> {
    int(vars, i).map(|v| v as u32)
}

fn nonzero(vars: &[Value], i: usize) -> Option<u32> {
    hash(vars, i).filter(|h| *h != 0)
}

fn text_var(vars: &[Value], i: usize) -> Option<&str> {
    vars.get(i)?.as_str()
}

/// A hash stored as `0xXXXXXXXX` or as a number in an RDBN field.
fn field_id(fields: &BTreeMap<String, Value>, key: &str) -> Option<u32> {
    match fields.get(key)? {
        Value::String(s) => nie_data::hash::HashId::parse(s).map(|h| h.0),
        Value::Number(n) => n.as_i64().map(|v| v as u32),
        _ => None,
    }
    .filter(|id| *id != 0)
}

fn fields(rec: &Rec) -> Option<&BTreeMap<String, Value>> {
    rec.fields.as_ref()
}

/// A paged answer; `total` counts every match.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Page<T> {
    pub total: usize,
    pub offset: usize,
    pub items: Vec<T>,
}

/// What a list route asks for: the languages of the labels, a text filter, a window.
#[derive(Debug, Clone, Default)]
pub struct Query {
    /// Language folders or BCP 47 tags; empty means every language.
    pub languages: Vec<String>,
    /// Case-insensitive substring searched in names (every selected language) and codes.
    pub text: Option<String>,
    /// Restricts characters to one [`CharacterClass`] (`param_and_base`, ...).
    pub class: Option<String>,
    pub offset: usize,
    /// `None` returns every match.
    pub limit: Option<usize>,
}

/// A table class that backs a page family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Source {
    pub family: &'static str,
    pub vfs_path: String,
    pub table: String,
    pub entries: usize,
}

enum Entry {
    /// (param index, first base entry of its base id)
    Param(usize, Option<usize>),
    BaseOnly(usize),
}

struct CharaIndex {
    param: Arc<Table>,
    base: Arc<Table>,
    entries: Vec<Entry>,
}

impl CharaIndex {
    fn params(&self) -> &[Rec] {
        self.param.class(PARAM_NODE).map_or(&[], |c| &c.recs)
    }

    fn bases(&self) -> &[Rec] {
        self.base.class(BASE_NODE).map_or(&[], |c| &c.recs)
    }

    fn class_of(entry: &Entry) -> CharacterClass {
        match entry {
            Entry::Param(_, Some(_)) => CharacterClass::ParamAndBase,
            Entry::Param(_, None) => CharacterClass::ParamWithoutBase,
            Entry::BaseOnly(_) => CharacterClass::BaseWithoutParam,
        }
    }

    fn base_of(entry: &Entry) -> Option<usize> {
        match entry {
            Entry::Param(_, base) => *base,
            Entry::BaseOnly(base) => Some(*base),
        }
    }
}

struct SkillRef {
    family: SkillFamily,
    entry: Provenance,
    name_key: Option<u32>,
}

struct SeriesRow {
    series_type: Option<i64>,
    name_key: Option<u32>,
}

/// Emblem files and affiliations of every team, computed once.
struct TeamIndex {
    files: HashMap<u32, Vec<String>>,
    affiliated: HashMap<u32, Vec<AffiliatedCharacter>>,
}

/// The wiki over one mounted installation.
pub struct Wiki {
    vfs: Arc<dyn VfsRead + Send + Sync>,
    languages: Vec<Language>,
    text_tables: Mutex<HashMap<(String, String), Arc<TextTable>>>,
    tables: Mutex<HashMap<String, Arc<Table>>>,
    chara: Lazy<CharaIndex>,
    skill_refs: Lazy<HashMap<u32, SkillRef>>,
    series: Lazy<HashMap<u32, SeriesRow>>,
    team_index: Lazy<TeamIndex>,
}

impl Wiki {
    /// Mounts the installation named by `source`, or discovered from the environment
    /// (`NIE_GAME_SOURCE`) when `None`.
    ///
    /// # Errors
    /// The source cannot be mounted or ships no text language.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(source: Option<&str>) -> Result<Self> {
        let vfs = nie_formats::vfs::open_source(source).map_err(|e| Error::Mount(e.to_string()))?;
        Self::new(vfs)
    }

    /// Wraps an already mounted VFS, owned or shared.
    ///
    /// # Errors
    /// The VFS ships no text language.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(vfs: impl Into<Arc<nie_formats::vfs::Vfs>>) -> Result<Self> {
        Self::over(vfs.into())
    }

    /// Wraps any mounted VFS: the native one, or the bytes a browser fetched
    /// ([`nie_formats::source_vfs::MemoryVfs`]).
    ///
    /// # Errors
    /// The VFS ships no text language.
    pub fn over(vfs: Arc<dyn VfsRead + Send + Sync>) -> Result<Self> {
        let languages = index::languages(&*vfs)?;
        Ok(Self {
            vfs,
            languages,
            text_tables: Mutex::new(HashMap::new()),
            tables: Mutex::new(HashMap::new()),
            chara: Lazy::new(),
            skill_refs: Lazy::new(),
            series: Lazy::new(),
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

    fn table(&self, stem: &str) -> Result<Arc<Table>> {
        if let Some(table) = locked(&self.tables).get(stem) {
            return Ok(Arc::clone(table));
        }
        let table = Arc::new(tables::load(&*self.vfs, stem)?);
        locked(&self.tables).insert(stem.to_owned(), Arc::clone(&table));
        Ok(table)
    }

    fn text_table(&self, language: &str, stem: &str) -> Result<Option<Arc<TextTable>>> {
        let key = (language.to_owned(), stem.to_owned());
        if let Some(table) = locked(&self.text_tables).get(&key) {
            return Ok(Some(Arc::clone(table)));
        }
        if !self.vfs.contains(&index::text_path(language, stem)) {
            return Ok(None);
        }
        let table = Arc::new(TextTable::load(&*self.vfs, language, stem)?);
        locked(&self.text_tables).insert(key, Arc::clone(&table));
        Ok(Some(table))
    }

    fn texts<'a>(&'a self, stem: &'a str, langs: &'a [String]) -> Result<TextSet<'a>> {
        let mut tables = Vec::new();
        for language in langs {
            if let Some(table) = self.text_table(language, stem)? {
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

    /// The tables behind the page families, one row per class, with entry counts.
    ///
    /// # Errors
    /// A table cannot be found or decoded.
    pub fn sources(&self) -> Result<Vec<Source>> {
        let mut out = Vec::new();
        for info in self.tables()? {
            for class in &info.classes {
                out.push(Source {
                    family: class.rich_family.unwrap_or("tables"),
                    vfs_path: info.vfs_path.clone(),
                    table: class.name.clone(),
                    entries: class.entries,
                });
            }
        }
        Ok(out)
    }

    /// Every gamedata file of the installation and whether a page family decodes it.
    ///
    /// # Errors
    /// A registry table cannot be found.
    pub fn catalog(&self) -> Result<Vec<CatalogEntry>> {
        let decoded: HashSet<String> = REGISTRY
            .iter()
            .map(|(stem, _)| index::find_table(&*self.vfs, stem))
            .collect::<Result<_>>()?;
        let mut out: Vec<CatalogEntry> = self
            .vfs
            .entries()
            .filter(|(path, _)| {
                path.starts_with(index::GAMEDATA_DIR) && path.contains(GAMEDATA_FILE_MARK)
            })
            .map(|(path, _)| CatalogEntry {
                vfs_path: path.to_owned(),
                status: if decoded.contains(path) {
                    "decoded"
                } else {
                    "present"
                },
            })
            .collect();
        out.sort_by(|a, b| a.vfs_path.cmp(&b.vfs_path));
        Ok(out)
    }

    // ----- generic tables -----

    /// Every registry table with its classes.
    ///
    /// # Errors
    /// A table cannot be found or decoded.
    pub fn tables(&self) -> Result<Vec<TableInfo>> {
        REGISTRY
            .iter()
            .map(|(stem, rich)| {
                let table = self.table(stem)?;
                Ok(TableInfo {
                    table: table.stem.clone(),
                    vfs_path: table.path.clone(),
                    format: table.format,
                    classes: table
                        .classes
                        .iter()
                        .map(|class| {
                            let mut info: ClassInfo = class.info.clone();
                            info.rich_family = rich
                                .iter()
                                .find(|(name, _)| *name == info.name)
                                .map(|(_, family)| *family);
                            info
                        })
                        .collect(),
                })
            })
            .collect()
    }

    fn registry_table(&self, stem: &str) -> Result<Option<Arc<Table>>> {
        if REGISTRY.iter().any(|(known, _)| *known == stem) {
            self.table(stem).map(Some)
        } else {
            Ok(None)
        }
    }

    fn build_record(
        &self,
        table: &Table,
        class: &str,
        index: usize,
        rec: &Rec,
        sets: &HashMap<&'static str, TextSet<'_>>,
    ) -> TableRecord {
        let mut labels = BTreeMap::new();
        for r in TEXT_REFS
            .iter()
            .filter(|r| r.table == table.stem && r.class == class)
        {
            let key = match r.sel {
                Sel::Var(i) => nonzero(&rec.variables, i),
                Sel::Field(name) => fields(rec).and_then(|f| field_id(f, name)),
            };
            if let (Some(key), Some(set)) = (key, sets.get(r.text))
                && let Some(label) = set.localize(r.section, key)
            {
                labels.insert(r.label.to_owned(), label);
            }
        }
        TableRecord {
            table: table.stem.clone(),
            class: class.to_owned(),
            index,
            fields: rec.fields.clone(),
            variables: rec.fields.is_none().then(|| rec.variables.clone()),
            children: rec.children.clone(),
            sort_index: rec.sort_index,
            refs: rec
                .refs
                .iter()
                .map(|(target, (offset, count))| {
                    (target.clone(), table.slice(target, *offset, *count))
                })
                .collect(),
            labels,
            source: Provenance {
                vfs_path: table.path.clone(),
                table: class.to_owned(),
                index,
            },
        }
    }

    fn text_sets<'a>(
        &'a self,
        stem: &str,
        langs: &'a [String],
    ) -> Result<HashMap<&'static str, TextSet<'a>>> {
        let mut sets = HashMap::new();
        for r in TEXT_REFS.iter().filter(|r| r.table == stem) {
            if !sets.contains_key(r.text) {
                sets.insert(r.text, self.texts(r.text, langs)?);
            }
        }
        Ok(sets)
    }

    /// The entries of one class of a registry table, windowed by `query`.
    ///
    /// # Errors
    /// A table cannot be decoded or a language is unknown. `Ok(None)`: no such table or class.
    pub fn table_records(
        &self,
        stem: &str,
        class: &str,
        query: &Query,
    ) -> Result<Option<Page<TableRecord>>> {
        let langs = self.resolve_languages(&query.languages)?;
        let Some(table) = self.registry_table(stem)? else {
            return Ok(None);
        };
        let Some(found) = table.class(class) else {
            return Ok(None);
        };
        let sets = self.text_sets(stem, &langs)?;
        let needle = query.text.as_deref().map(str::to_lowercase);
        let hits: Vec<usize> = (0..found.recs.len())
            .filter(|i| {
                needle.as_deref().is_none_or(|needle| {
                    let rec = &found.recs[*i];
                    format!("{:?}{:?}", rec.variables, rec.fields)
                        .to_lowercase()
                        .contains(needle)
                        || self
                            .build_record(&table, class, *i, rec, &sets)
                            .labels
                            .values()
                            .any(|l| localized_matches(l, needle))
                })
            })
            .collect();
        Ok(Some(window(&hits, query, |i| {
            Some(self.build_record(&table, class, *i, &found.recs[*i], &sets))
        })))
    }

    /// One entry of a registry table class.
    ///
    /// # Errors
    /// A table cannot be decoded or a language is unknown. `Ok(None)`: no such entry.
    pub fn table_record<S: AsRef<str>>(
        &self,
        stem: &str,
        class: &str,
        index: usize,
        languages: &[S],
    ) -> Result<Option<TableRecord>> {
        let langs = self.resolve_languages(languages)?;
        let Some(table) = self.registry_table(stem)? else {
            return Ok(None);
        };
        let Some(rec) = table.class(class).and_then(|c| c.recs.get(index)) else {
            return Ok(None);
        };
        let sets = self.text_sets(stem, &langs)?;
        Ok(Some(self.build_record(&table, class, index, rec, &sets)))
    }

    // ----- characters -----

    fn chara_index(&self) -> Result<Arc<CharaIndex>> {
        self.chara.get(|| {
            let param = self.table("chara_param")?;
            let base = self.table("chara_base")?;
            let mut index = CharaIndex {
                param,
                base,
                entries: Vec::new(),
            };
            let mut first: HashMap<u32, usize> = HashMap::new();
            for (i, rec) in index.bases().iter().enumerate() {
                if let Some(id) = hash(&rec.variables, BASE_ID) {
                    first.entry(id).or_insert(i);
                }
            }
            let mut used: HashSet<usize> = HashSet::new();
            let mut entries = Vec::new();
            for (i, rec) in index.params().iter().enumerate() {
                let base = hash(&rec.variables, PARAM_BASE).and_then(|id| first.get(&id).copied());
                used.extend(base);
                entries.push(Entry::Param(i, base));
            }
            for i in 0..index.bases().len() {
                if !used.contains(&i) {
                    entries.push(Entry::BaseOnly(i));
                }
            }
            index.entries = entries;
            Ok(index)
        })
    }

    /// How many character records each class holds.
    ///
    /// # Errors
    /// A table cannot be found or decoded.
    pub fn character_classes(&self) -> Result<Vec<ClassCount>> {
        let index = self.chara_index()?;
        let count = |class| {
            index
                .entries
                .iter()
                .filter(|e| CharaIndex::class_of(e) == class)
                .count()
        };
        Ok(vec![
            ClassCount {
                class: CharacterClass::ParamAndBase,
                count: count(CharacterClass::ParamAndBase),
                description: "chara_param entries whose baseId names a chara_base entry",
            },
            ClassCount {
                class: CharacterClass::ParamWithoutBase,
                count: count(CharacterClass::ParamWithoutBase),
                description: "chara_param entries whose baseId matches no chara_base entry",
            },
            ClassCount {
                class: CharacterClass::BaseWithoutParam,
                count: count(CharacterClass::BaseWithoutParam),
                description: "chara_base entries no chara_param entry names",
            },
        ])
    }

    /// `chara_param` entries joined to their `chara_base` entry, then the `chara_base` entries
    /// no `chara_param` entry names; every entry of both tables is one record.
    ///
    /// # Errors
    /// A table cannot be found or decoded, or a language is unknown.
    pub fn characters(&self, query: &Query) -> Result<Page<Character>> {
        let langs = self.resolve_languages(&query.languages)?;
        let index = self.chara_index()?;
        let ctx = CharaContext::new(self, &langs)?;
        let needle = query.text.as_deref().map(str::to_lowercase);
        let class = query.class.as_deref();
        let hits: Vec<&Entry> = index
            .entries
            .iter()
            .filter(|entry| {
                class.is_none_or(|class| class_name(CharaIndex::class_of(entry)) == class)
                    && needle
                        .as_deref()
                        .is_none_or(|needle| ctx.matches(&index, entry, needle))
            })
            .collect();
        Ok(window(&hits, query, |entry| ctx.build(&index, entry)))
    }

    /// The character whose `chara_param` id (`0xXXXXXXXX`), `base-<n>` (entry `n` of the
    /// `chara_base` table), `chara_base` id or internal code is `key`, in that order.
    ///
    /// # Errors
    /// A table cannot be found or decoded, or a language is unknown.
    pub fn character<S: AsRef<str>>(
        &self,
        key: &str,
        languages: &[S],
    ) -> Result<Option<Character>> {
        let langs = self.resolve_languages(languages)?;
        let index = self.chara_index()?;
        let id = nie_data::hash::HashId::parse(key).map(|h| h.0);
        let base_entry = key
            .strip_prefix("base-")
            .and_then(|n| n.parse::<usize>().ok());
        let param_id = |entry: &Entry| match entry {
            Entry::Param(p, _) => hash(&index.params()[*p].variables, PARAM_ID),
            Entry::BaseOnly(_) => None,
        };
        let base_var = |entry: &Entry, var: usize| {
            CharaIndex::base_of(entry)
                .map(|b| &index.bases()[b].variables[..])
                .map(|vars| (vars, var))
        };
        let found = id
            .and_then(|id| index.entries.iter().find(|e| param_id(e) == Some(id)))
            .or_else(|| {
                base_entry.and_then(|n| {
                    index
                        .entries
                        .iter()
                        .find(|e| matches!(e, Entry::BaseOnly(b) if *b == n))
                })
            })
            .or_else(|| {
                id.and_then(|id| {
                    index.entries.iter().find(|e| {
                        base_var(e, BASE_ID).is_some_and(|(vars, i)| hash(vars, i) == Some(id))
                    })
                })
            })
            .or_else(|| {
                index.entries.iter().find(|e| {
                    base_var(e, BASE_CODE).is_some_and(|(vars, i)| text_var(vars, i) == Some(key))
                })
            });
        let Some(entry) = found else {
            return Ok(None);
        };
        Ok(CharaContext::new(self, &langs)?.build(&index, entry))
    }

    fn skill_ref_index(&self) -> Result<Arc<HashMap<u32, SkillRef>>> {
        self.skill_refs.get(|| {
            let mut out: HashMap<u32, SkillRef> = HashMap::new();
            type Pick<'a> = &'a dyn Fn(&Rec) -> Option<u32>;
            let mut add = |table: &Table, class: &str, id: Pick<'_>, name: Pick<'_>, family| {
                if let Some(c) = table.class(class) {
                    for (index, rec) in c.recs.iter().enumerate() {
                        if let Some(id) = id(rec) {
                            out.entry(id).or_insert_with(|| SkillRef {
                                family,
                                entry: Provenance {
                                    vfs_path: table.path.clone(),
                                    table: class.to_owned(),
                                    index,
                                },
                                name_key: name(rec),
                            });
                        }
                    }
                }
            };
            add(
                &*self.table("skill_config")?,
                SKILL_LIST,
                &|r| fields(r).and_then(|f| field_id(f, "skillID")),
                &|r| fields(r).and_then(|f| field_id(f, "skillNameId")),
                SkillFamily::Skill,
            );
            add(
                &*self.table("aura_skill_config")?,
                AURA_NODE,
                &|r| nonzero(&r.variables, 0),
                &|r| nonzero(&r.variables, 2),
                SkillFamily::Aura,
            );
            add(
                &*self.table("passive_skill_config")?,
                PASSIVE_NODE,
                &|r| nonzero(&r.variables, 0),
                &|r| nonzero(&r.variables, 1),
                SkillFamily::Passive,
            );
            Ok(out)
        })
    }

    fn series_index(&self) -> Result<Arc<HashMap<u32, SeriesRow>>> {
        self.series.get(|| {
            let table = self.table("chara_series_config")?;
            Ok(table
                .class(SERIES_LIST)
                .into_iter()
                .flat_map(|c| &c.recs)
                .filter_map(|rec| {
                    let f = fields(rec)?;
                    Some((
                        field_id(f, "charaSeriesId")?,
                        SeriesRow {
                            series_type: f.get("charaSeriesType").and_then(Value::as_i64),
                            name_key: field_id(f, "charaSeriesNameTextId"),
                        },
                    ))
                })
                .collect())
        })
    }

    // ----- skills -----

    fn skill_table(&self) -> Result<Arc<Table>> {
        self.table("skill_config")
    }

    fn skill_recs(table: &Table) -> &[Rec] {
        table.class(SKILL_LIST).map_or(&[], |c| &c.recs)
    }

    /// `skill_config` entries in table order, filtered and windowed by `query`.
    ///
    /// # Errors
    /// A table cannot be found or decoded, or a language is unknown.
    pub fn skills(&self, query: &Query) -> Result<Page<Skill>> {
        let langs = self.resolve_languages(&query.languages)?;
        let table = self.skill_table()?;
        let ctx = SkillContext::new(self, &langs)?;
        let needle = query.text.as_deref().map(str::to_lowercase);
        let hits: Vec<(usize, &Rec)> = Self::skill_recs(&table)
            .iter()
            .enumerate()
            .filter(|(_, rec)| {
                skill_key(rec).is_some()
                    && needle
                        .as_deref()
                        .is_none_or(|needle| ctx.matches(rec, needle))
            })
            .collect();
        Ok(window(&hits, query, |(index, rec)| {
            ctx.build(&table.path, rec, *index)
        }))
    }

    /// The skill whose `skillID` (`0xXXXXXXXX`) or `skillIDStr` is `key`.
    ///
    /// # Errors
    /// A table cannot be found or decoded, or a language is unknown.
    pub fn skill<S: AsRef<str>>(&self, key: &str, languages: &[S]) -> Result<Option<Skill>> {
        let langs = self.resolve_languages(languages)?;
        let table = self.skill_table()?;
        let id = nie_data::hash::HashId::parse(key).map(|h| h.0);
        let found = Self::skill_recs(&table)
            .iter()
            .enumerate()
            .find(|(_, rec)| {
                skill_key(rec).is_some_and(|(sid, code)| Some(sid) == id || code == key)
            });
        let Some((index, rec)) = found else {
            return Ok(None);
        };
        Ok(SkillContext::new(self, &langs)?.build(&table.path, rec, index))
    }

    // ----- teams -----

    fn team_table(&self) -> Result<Arc<Table>> {
        self.table("belong_team_config")
    }

    fn team_recs(table: &Table) -> &[Rec] {
        table.class(TEAM_LIST).map_or(&[], |c| &c.recs)
    }

    fn team_index(&self) -> Result<Arc<TeamIndex>> {
        self.team_index.get(|| {
            let table = self.team_table()?;
            let wanted: HashSet<u32> = Self::team_recs(&table)
                .iter()
                .flat_map(|rec| emblem_ids(rec).into_iter().map(|(_, id)| id))
                .collect();
            let files = index::files_by_name_hash(&*self.vfs, &wanted);

            let data = self.chara_index()?;
            let mut affiliated: HashMap<u32, Vec<AffiliatedCharacter>> = HashMap::new();
            for (i, rec) in data.bases().iter().enumerate() {
                let (Some(team), Some(id), Some(code)) = (
                    nonzero(&rec.variables, BASE_TEAM),
                    hash(&rec.variables, BASE_ID),
                    text_var(&rec.variables, BASE_CODE),
                ) else {
                    continue;
                };
                affiliated
                    .entry(team)
                    .or_default()
                    .push(AffiliatedCharacter {
                        chara_base_id: Id(id),
                        internal_code: code.to_owned(),
                        source: Provenance {
                            vfs_path: data.base.path.clone(),
                            table: BASE_NODE.to_owned(),
                            index: i,
                        },
                    });
            }
            Ok(TeamIndex { files, affiliated })
        })
    }

    /// `belong_team_config` entries in table order, filtered and windowed by `query`, with
    /// emblems resolved by file-name hash.
    ///
    /// # Errors
    /// A table cannot be found or decoded, or a language is unknown.
    pub fn teams(&self, query: &Query) -> Result<Page<Team>> {
        let langs = self.resolve_languages(&query.languages)?;
        let table = self.team_table()?;
        let names = self.texts("team_text", &langs)?;
        let needle = query.text.as_deref().map(str::to_lowercase);
        let hits: Vec<(usize, &Rec)> = Self::team_recs(&table)
            .iter()
            .enumerate()
            .filter(|(_, rec)| {
                fields(rec)
                    .and_then(|f| field_id(f, "belongTeamId"))
                    .is_some()
                    && needle.as_deref().is_none_or(|needle| {
                        fields(rec)
                            .and_then(|f| field_id(f, "teamNameTextId"))
                            .is_some_and(|id| names.matches(Section::Any, id, needle))
                    })
            })
            .collect();
        let index = self.team_index()?;
        Ok(window(&hits, query, |(position, rec)| {
            build_team(&table.path, rec, *position, &names, &index)
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
        let table = self.team_table()?;
        let Some((position, rec)) = Self::team_recs(&table)
            .iter()
            .enumerate()
            .find(|(_, rec)| fields(rec).and_then(|f| field_id(f, "belongTeamId")) == Some(id))
        else {
            return Ok(None);
        };
        let names = self.texts("team_text", &langs)?;
        let index = self.team_index()?;
        Ok(build_team(&table.path, rec, position, &names, &index))
    }
}

fn class_name(class: CharacterClass) -> &'static str {
    match class {
        CharacterClass::ParamAndBase => "param_and_base",
        CharacterClass::ParamWithoutBase => "param_without_base",
        CharacterClass::BaseWithoutParam => "base_without_param",
    }
}

fn localized_matches(l: &Localized, needle: &str) -> bool {
    l.texts.values().flat_map(|t| &t.forms).any(|f| {
        f.text.to_lowercase().contains(needle)
            || f.raw
                .as_deref()
                .is_some_and(|r| r.to_lowercase().contains(needle))
    })
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

fn skill_key(rec: &Rec) -> Option<(u32, &str)> {
    let f = fields(rec)?;
    Some((field_id(f, "skillID")?, f.get("skillIDStr")?.as_str()?))
}

fn build_team(
    path: &str,
    rec: &Rec,
    position: usize,
    names: &TextSet<'_>,
    index: &TeamIndex,
) -> Option<Team> {
    let f = fields(rec)?;
    let belong_team_id = field_id(f, "belongTeamId")?;
    Some(Team {
        belong_team_id: Id(belong_team_id),
        name: field_id(f, "teamNameTextId").and_then(|id| names.localize(Section::Any, id)),
        emblems: emblem_ids(rec)
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
        fields: f.clone(),
        roster: UNSOURCED[0],
        source: Provenance {
            vfs_path: path.to_owned(),
            table: TEAM_LIST.to_owned(),
            index: position,
        },
    })
}

/// The non-zero `teamEmblemId_*` fields of a team entry, by field name.
fn emblem_ids(rec: &Rec) -> Vec<(String, u32)> {
    let Some(f) = fields(rec) else {
        return Vec::new();
    };
    f.keys()
        .filter(|field| field.starts_with(EMBLEM_FIELD_PREFIX))
        .filter_map(|field| Some((field.clone(), field_id(f, field)?)))
        .collect()
}

struct CharaContext<'a> {
    wiki: &'a Wiki,
    names: TextSet<'a>,
    roma: TextSet<'a>,
    descriptions: TextSet<'a>,
    system: TextSet<'a>,
    skill_text: TextSet<'a>,
    team_names: TextSet<'a>,
    series_names: TextSet<'a>,
    teams: HashMap<u32, Option<u32>>,
    series: Arc<HashMap<u32, SeriesRow>>,
    skill_refs: Arc<HashMap<u32, SkillRef>>,
}

impl<'a> CharaContext<'a> {
    fn new(wiki: &'a Wiki, langs: &'a [String]) -> Result<Self> {
        let teams = Wiki::team_recs(&*wiki.team_table()?)
            .iter()
            .filter_map(|rec| {
                let f = fields(rec)?;
                Some((field_id(f, "belongTeamId")?, field_id(f, "teamNameTextId")))
            })
            .collect();
        Ok(Self {
            wiki,
            names: wiki.texts("chara_text", langs)?,
            roma: wiki.texts("chara_text_roma", langs)?,
            descriptions: wiki.texts("chara_description_text", langs)?,
            system: wiki.texts("system_text", langs)?,
            skill_text: wiki.texts("skill_text", langs)?,
            team_names: wiki.texts("team_text", langs)?,
            series_names: wiki.texts("chara_add_info_text", langs)?,
            teams,
            series: wiki.series_index()?,
            skill_refs: wiki.skill_ref_index()?,
        })
    }

    fn matches(&self, index: &CharaIndex, entry: &Entry, needle: &str) -> bool {
        let mut hit = false;
        if let Entry::Param(p, _) = entry {
            let vars = &index.params()[*p].variables;
            hit |= hash(vars, PARAM_ID).is_some_and(|id| format!("0x{id:08x}").contains(needle));
        }
        if let Some(b) = CharaIndex::base_of(entry) {
            let vars = &index.bases()[b].variables;
            hit |= text_var(vars, BASE_CODE).is_some_and(|c| c.to_lowercase().contains(needle));
            for var in [BASE_NAME, BASE_FAMILY, BASE_FAMILY_VARIANT] {
                if let Some(key) = nonzero(vars, var) {
                    hit |= self.names.matches(Section::Noun, key, needle)
                        || self.roma.matches(Section::Noun, key, needle);
                }
            }
        }
        hit
    }

    fn learned(&self, vars: &[Value]) -> Vec<LearnedSkill> {
        let mut out = Vec::new();
        for slot in 0..MAX_SKILL_SLOTS {
            let level_index = SKILL_BLOCK_START + slot * 2;
            let hash_index = level_index + 1;
            if hash_index >= vars.len() {
                break;
            }
            let (Some(id), Some(level)) = (nonzero(vars, hash_index), int(vars, level_index))
            else {
                continue;
            };
            if !(0..=99).contains(&level) {
                continue;
            }
            let found = self.skill_refs.get(&id);
            out.push(LearnedSkill {
                skill_id: Id(id),
                learn_level: u8::try_from(level).unwrap_or(0),
                family: found.map(|f| f.family),
                entry: found.map(|f| f.entry.clone()),
                name_text_id: found.and_then(|f| f.name_key).map(Id),
                name: found
                    .and_then(|f| f.name_key)
                    .and_then(|key| self.skill_text.localize(Section::Noun, key)),
            });
        }
        out
    }

    fn build(&self, index: &CharaIndex, entry: &Entry) -> Option<Character> {
        let param = match entry {
            Entry::Param(p, _) => Some((*p, &index.params()[*p])),
            Entry::BaseOnly(_) => None,
        };
        let base = CharaIndex::base_of(entry).map(|b| (b, &index.bases()[b]));
        let bv: &[Value] = base.map_or(&[], |(_, rec)| &rec.variables);
        let pv: &[Value] = param.map_or(&[], |(_, rec)| &rec.variables);
        let chara_base_id = match param {
            Some(_) => hash(pv, PARAM_BASE)?,
            None => hash(bv, BASE_ID)?,
        };
        let coded = |value: Option<i64>| value.map(|value| Coded { value, label: None });
        let localize = |set: &TextSet<'_>, section, key: Option<u32>| {
            key.and_then(|key| set.localize(section, key))
        };
        let name_key = nonzero(bv, BASE_NAME);
        let family_key = nonzero(bv, BASE_FAMILY);
        let variant_key = nonzero(bv, BASE_FAMILY_VARIANT);
        let team_id = nonzero(bv, BASE_TEAM);
        let series_id = nonzero(bv, BASE_SERIES);
        Some(Character {
            class: CharaIndex::class_of(entry),
            chara_param_id: param
                .and_then(|(_, rec)| hash(&rec.variables, PARAM_ID))
                .map(Id),
            chara_base_id: Id(chara_base_id),
            internal_code: text_var(bv, BASE_CODE).map(str::to_owned),
            name: localize(&self.names, Section::Noun, name_key),
            family_name: localize(&self.names, Section::Noun, family_key),
            family_name_variant: localize(&self.names, Section::Noun, variant_key),
            name_roma: localize(&self.roma, Section::Noun, name_key),
            family_name_roma: localize(&self.roma, Section::Noun, family_key),
            family_name_variant_roma: localize(&self.roma, Section::Noun, variant_key),
            description: localize(
                &self.descriptions,
                Section::Text,
                nonzero(bv, BASE_DESCRIPTION),
            ),
            gender: coded(int(bv, BASE_GENDER)),
            element: int(pv, PARAM_ELEMENT).map(|value| self.wiki.element(value, &self.system)),
            main_position: coded(int(pv, PARAM_MAIN_POSITION)),
            sub_position: coded(int(pv, PARAM_SUB_POSITION)),
            growth_pattern: int(pv, PARAM_GROWTH),
            series: series_id.map(|id| {
                let row = self.series.get(&id);
                SeriesRef {
                    series_id: Id(id),
                    series_type: row.and_then(|r| r.series_type),
                    name: localize(
                        &self.series_names,
                        Section::Noun,
                        row.and_then(|r| r.name_key),
                    ),
                }
            }),
            team: team_id.map(|id| TeamRef {
                belong_team_id: Id(id),
                name: localize(
                    &self.team_names,
                    Section::Any,
                    self.teams.get(&id).copied().flatten(),
                ),
            }),
            skills: self.learned(pv),
            sort_index: param.and_then(|(_, rec)| rec.sort_index),
            battle: base.and_then(|(_, rec)| {
                rec.refs
                    .get(BATTLE_NODE)
                    .map(|(offset, count)| index.base.slice(BATTLE_NODE, *offset, *count))
            }),
            param_raw: pv.to_vec(),
            base_raw: bv.to_vec(),
            param_source: param.map(|(i, _)| Provenance {
                vfs_path: index.param.path.clone(),
                table: PARAM_NODE.to_owned(),
                index: i,
            }),
            base_source: base.map(|(i, _)| Provenance {
                vfs_path: index.base.path.clone(),
                table: BASE_NODE.to_owned(),
                index: i,
            }),
        })
    }
}

struct SkillContext<'a> {
    wiki: &'a Wiki,
    names: TextSet<'a>,
    system: TextSet<'a>,
}

impl<'a> SkillContext<'a> {
    fn new(wiki: &'a Wiki, langs: &'a [String]) -> Result<Self> {
        Ok(Self {
            wiki,
            names: wiki.texts("skill_text", langs)?,
            system: wiki.texts("system_text", langs)?,
        })
    }

    fn matches(&self, rec: &Rec, needle: &str) -> bool {
        skill_key(rec).is_some_and(|(_, code)| code.to_lowercase().contains(needle))
            || fields(rec)
                .and_then(|f| field_id(f, "skillNameId"))
                .is_some_and(|id| self.names.matches(Section::Noun, id, needle))
    }

    fn build(&self, path: &str, rec: &Rec, index: usize) -> Option<Skill> {
        let (skill_id, skill_id_str) = skill_key(rec)?;
        let f = fields(rec)?;
        let code = |key: &str| f.get(key).and_then(Value::as_i64);
        Some(Skill {
            skill_id: Id(skill_id),
            skill_id_str: skill_id_str.to_owned(),
            name: field_id(f, "skillNameId").and_then(|id| self.names.localize(Section::Noun, id)),
            description: field_id(f, "skillDescId")
                .and_then(|id| self.names.localize(Section::Text, id)),
            element: code("element").map(|value| self.wiki.element(value, &self.system)),
            category: code("category").map(|value| Coded { value, label: None }),
            fields: f.clone(),
            source: Provenance {
                vfs_path: path.to_owned(),
                table: SKILL_LIST.to_owned(),
                index,
            },
        })
    }
}
