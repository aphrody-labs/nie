//! Output model: every record is a game value plus its decoded label, per language, with the VFS
//! provenance of each part. A field with no source is `None` or absent, never a derived copy.

use std::collections::BTreeMap;

use serde::{Serialize, Serializer};
use serde_json::Value;

/// A 32-bit game identifier, serialized as `0xXXXXXXXX` the way the game tables print it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(pub u32);

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:08X}", self.0)
    }
}

impl Serialize for Id {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// Where a record came from: the VFS file, the table inside it, and the entry's position among
/// that table's entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Provenance {
    pub vfs_path: String,
    pub table: String,
    pub index: usize,
}

/// A language the installation ships, derived from the folders of `data/common/text`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Language {
    /// Folder name under `data/common/text` (`zh_hans`).
    pub folder: String,
    /// BCP 47 tag (`zh-Hans`).
    pub tag: String,
}

/// One text entry. `form` is the entry's second variable as stored (0 for the base text; the
/// character names also carry forms 11 and 12 for the same key).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Form {
    pub form: i64,
    /// The decoded label: furigana, tags and escapes resolved by the nie-data text decoder.
    pub text: String,
    /// The stored string, present only when it differs from `text`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
    /// Position among the entries of the same kind in the text file.
    pub index: usize,
}

/// The entries one language holds for a key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LangText {
    pub vfs_path: String,
    /// Entry kind in the file: `NOUN_INFO` or `TEXT_INFO`.
    pub section: String,
    pub forms: Vec<Form>,
}

/// A text key resolved in every language that has it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Localized {
    pub id: Id,
    /// Text file stem (`chara_text`).
    pub table: String,
    /// Keyed by language folder.
    pub texts: BTreeMap<String, LangText>,
}

impl Localized {
    /// The base text (form 0) in `language`, falling back to the first stored form.
    #[must_use]
    pub fn text(&self, language: &str) -> Option<&str> {
        let forms = &self.texts.get(language)?.forms;
        forms
            .iter()
            .find(|f| f.form == 0)
            .or(forms.first())
            .map(|f| f.text.as_str())
    }
}

/// A value stored as a code in a table, with its label when a VFS text backs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Coded {
    /// The code as stored.
    pub value: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<Localized>,
}

/// A family the VFS gives no source for. Pages render this as an explicit empty state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct NoSource {
    pub family: &'static str,
    pub reason: &'static str,
}

/// The families without a VFS source today (docs/wiki.md, rule 3).
pub const UNSOURCED: [NoSource; 3] = [
    NoSource {
        family: "team roster",
        reason: "no table lists a team's match lineup; the only character-to-team link is the affiliation in chara_base",
    },
    NoSource {
        family: "capsule lot contents",
        reason: "the lot entries' vars[1] matches no table or text",
    },
    NoSource {
        family: "invocation rates",
        reason: "no table carries a skill invocation rate",
    },
];

/// The game family an id of a character's learned-skill list belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillFamily {
    /// `skill_config`: `m_skillInfoList` skillID.
    Skill,
    /// `aura_skill_config`: `AURA_CMD_INFO` variable 0.
    Aura,
    /// `passive_skill_config`: `PASSIVE_SKILL_INFO` variable 0.
    Passive,
}

/// A skill a character learns, from the character's `chara_param` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LearnedSkill {
    pub skill_id: Id,
    pub learn_level: u8,
    /// The table family whose entry carries this id; absent when none does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<SkillFamily>,
    /// The entry of that family.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<Provenance>,
    /// The text key the entry names; absent when the entry has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_text_id: Option<Id>,
    /// Absent when no id entry exists or no text holds the key in any language.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Localized>,
}

/// A team as a character page references it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TeamRef {
    pub belong_team_id: Id,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Localized>,
}

/// A series as a character references it (`chara_series_config`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SeriesRef {
    pub series_id: Id,
    /// `charaSeriesType` as stored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_type: Option<i64>,
    /// The `charaSeriesNameTextId` text; absent when no series entry or no text exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Localized>,
}

/// How a character record was formed from the two tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterClass {
    /// A `chara_param` entry whose baseId names a `chara_base` entry.
    ParamAndBase,
    /// A `chara_param` entry whose baseId matches no `chara_base` entry.
    ParamWithoutBase,
    /// A `chara_base` entry that no `chara_param` entry names, or a later entry of an id an
    /// earlier entry already holds.
    BaseWithoutParam,
}

/// How many character records each class holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClassCount {
    pub class: CharacterClass,
    pub count: usize,
    pub description: &'static str,
}

/// A slice of another node class referenced by `[offset, count]`, resolved.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RefSlice {
    /// The class the slice indexes.
    pub class: String,
    pub offset: i64,
    pub count: i64,
    /// The referenced entries as stored; empty when the slice leaves the class's range.
    pub items: Vec<RefItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RefItem {
    pub index: usize,
    pub variables: Vec<Value>,
}

/// One `chara_param` and/or `chara_base` entry. Variable lists are every variable as stored.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Character {
    pub class: CharacterClass,
    /// Absent for a `chara_base` entry no `chara_param` entry names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chara_param_id: Option<Id>,
    /// The `chara_base` id (a `chara_param` entry carries it even when no base entry exists).
    pub chara_base_id: Id,
    /// Absent when no `chara_base` entry exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal_code: Option<String>,
    /// `chara_base` variable 3: forms 0, 11 and 12 carry the full name and its two parts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Localized>,
    /// `chara_base` variable 4.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family_name: Option<Localized>,
    /// `chara_base` variable 5 (the stored text differs from variable 4's).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family_name_variant: Option<Localized>,
    /// The `chara_text_roma` rows of the keys of variables 3, 4 and 5.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_roma: Option<Localized>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family_name_roma: Option<Localized>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family_name_variant_roma: Option<Localized>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Localized>,
    /// As stored: 1 = male, 2 = female. Absent when the record has no such variable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<Coded>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<Coded>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_position: Option<Coded>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_position: Option<Coded>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub growth_pattern: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<SeriesRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team: Option<TeamRef>,
    pub skills: Vec<LearnedSkill>,
    /// The `__SORT_INDEX` entry of the same position in the `chara_param` table.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_index: Option<i64>,
    /// The `CHARA_BASE_INFO_REF_BATTLE` slice of `CHARA_BASE_BATTLE`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battle: Option<RefSlice>,
    /// The `chara_param` variables in stored order.
    pub param_raw: Vec<Value>,
    /// The `chara_base` variables in stored order.
    pub base_raw: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param_source: Option<Provenance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_source: Option<Provenance>,
}

/// One `skill_config` entry.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Skill {
    pub skill_id: Id,
    pub skill_id_str: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Localized>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Localized>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<Coded>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<Coded>,
    /// Every field of the entry as the table stores it.
    pub fields: BTreeMap<String, Value>,
    pub source: Provenance,
}

/// An emblem id and the VFS files whose name hashes to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Emblem {
    pub emblem_id: Id,
    /// CRC32 of the file name (without extension) equals `emblem_id`; empty when no file does.
    pub files: Vec<String>,
}

/// A character whose `chara_base` entry names the team as its belongTeamId.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AffiliatedCharacter {
    pub chara_base_id: Id,
    pub internal_code: String,
    pub source: Provenance,
}

/// One `belong_team_config` entry.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Team {
    pub belong_team_id: Id,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Localized>,
    /// One per `teamEmblemId_*` field, keyed by the field name.
    pub emblems: BTreeMap<String, Emblem>,
    pub affiliated_characters: Vec<AffiliatedCharacter>,
    /// Every field of the entry as the table stores it.
    pub fields: BTreeMap<String, Value>,
    pub roster: NoSource,
    pub source: Provenance,
}

/// How a node class of a table is served.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassRole {
    /// Each entry is a record.
    Records,
    /// `__SORT_INDEX`: entry k belongs to record k of the container's main class.
    SortIndex,
    /// `<MAIN>_REF_<TARGET>`: entry k holds the `[offset, count]` slice of target class
    /// entries that belongs to record k of the main class.
    Reference,
}

/// A node class (or RDBN list) of a table.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClassInfo {
    pub name: String,
    pub role: ClassRole,
    pub entries: usize,
    /// For a reference class, the class it slices; for an annotation, the class it annotates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// The page family that renders these entries richly, when one does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rich_family: Option<&'static str>,
    /// The container's own variables (T2B `*_LIST_BEG`), as stored.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub container_variables: Vec<Value>,
}

/// A decoded gamedata table and its classes.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TableInfo {
    /// File stem without version (`passive_skill_config`).
    pub table: String,
    pub vfs_path: String,
    /// `rdbn` (named fields) or `t2b` (positional variables).
    pub format: &'static str,
    pub classes: Vec<ClassInfo>,
}

/// One entry of a table class, as stored, with the labels of its text references.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TableRecord {
    pub table: String,
    pub class: String,
    pub index: usize,
    /// Named fields of an RDBN row.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<BTreeMap<String, Value>>,
    /// Positional variables of a T2B node (the file stores no field names).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<Vec<Value>>,
    /// Nested nodes of a T2B node, as stored.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_index: Option<i64>,
    /// Keyed by the target class.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub refs: BTreeMap<String, RefSlice>,
    /// Text labels keyed by what the reference is (`name`, `description`).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, Localized>,
    pub source: Provenance,
}

/// A gamedata file of the installation and whether the wiki decodes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CatalogEntry {
    pub vfs_path: String,
    /// `decoded` (a page family reads it) or `present` (listed, not decoded).
    pub status: &'static str,
}
