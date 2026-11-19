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

/// A skill a character learns, from the character's `chara_param` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LearnedSkill {
    pub skill_id: Id,
    pub learn_level: u8,
    /// Absent when `skill_config` has no entry for the id or no text names it.
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

/// One `chara_param` entry joined to its `chara_base` entry.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Character {
    pub chara_param_id: Id,
    pub chara_base_id: Id,
    pub internal_code: String,
    /// `chara_base` nameHash: forms 0, 11 and 12 carry the full name and its two parts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Localized>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family_name: Option<Localized>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Localized>,
    /// As stored: 1 = male, 2 = female. Absent when the record has no such variable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<Coded>,
    pub element: Coded,
    pub main_position: Coded,
    pub sub_position: Coded,
    pub growth_pattern: i64,
    /// `chara_base` seriesId as stored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_id: Option<Id>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team: Option<TeamRef>,
    pub skills: Vec<LearnedSkill>,
    /// The `chara_param` integer variables in stored order.
    pub param_raw: Vec<i64>,
    /// The `chara_base` variables in stored order.
    pub base_raw: Vec<Value>,
    pub param_source: Provenance,
    pub base_source: Provenance,
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
