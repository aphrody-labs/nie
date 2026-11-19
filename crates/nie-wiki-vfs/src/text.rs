//! Text tables (`data/common/text/<lang>/*.cfg.bin`): `NOUN_INFO` rows carry their string in
//! variable 5, `TEXT_INFO` rows in variable 2; both are keyed by (hash, form).

use std::collections::{BTreeMap, HashMap};

use nie_data::cfgbin::{Node, walk_named};
use nie_data::text::sanitize_text;
use nie_formats::vfs_read::VfsRead;

use crate::error::{Error, Result};
use crate::index::text_path;
use crate::model::{Form, Id, LangText, Localized};

pub(crate) const NOUN: &str = "NOUN_INFO";
pub(crate) const TEXT: &str = "TEXT_INFO";
const NOUN_STRING_VAR: usize = 5;
const TEXT_STRING_VAR: usize = 2;

/// A row name without the `_<index>` suffix some dumps append.
pub(crate) fn record_name(name: &str) -> &str {
    match name.rsplit_once('_') {
        Some((base, tail)) if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) => base,
        _ => name,
    }
}

#[derive(Debug, Default)]
struct Entry {
    form: i64,
    raw: String,
    index: usize,
}

/// Which row kinds a lookup may read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Section {
    Noun,
    Text,
    /// Nouns first, then texts.
    Any,
}

#[derive(Debug, Default)]
pub(crate) struct TextTable {
    path: String,
    nouns: HashMap<u32, Vec<Entry>>,
    texts: HashMap<u32, Vec<Entry>>,
}

impl TextTable {
    pub(crate) fn load(vfs: &dyn VfsRead, language: &str, stem: &str) -> Result<Self> {
        let path = text_path(language, stem);
        let json =
            nie_explore::game_data::decode_cfgbin(vfs, &path).map_err(|message| Error::Decode {
                path: path.clone(),
                message,
            })?;
        let mut table = Self {
            path,
            ..Self::default()
        };
        let (mut noun_index, mut text_index) = (0, 0);
        walk_named(&json, "", |node: Node<'_>| {
            let (rows, counter, string_var) = match record_name(node.name()) {
                NOUN => (&mut table.nouns, &mut noun_index, NOUN_STRING_VAR),
                TEXT => (&mut table.texts, &mut text_index, TEXT_STRING_VAR),
                _ => return,
            };
            let index = *counter;
            *counter += 1;
            let (Some(key), Some(text)) = (node.var(0), node.var(string_var)) else {
                return;
            };
            if key.ty != "Int" || text.ty != "String" || text.value.is_empty() {
                return;
            }
            rows.entry(key.as_hash().0).or_default().push(Entry {
                form: node.int(1),
                raw: text.value.to_owned(),
                index,
            });
        });
        Ok(table)
    }

    fn rows(&self, section: Section, id: u32) -> Option<(&'static str, &Vec<Entry>)> {
        let noun = || self.nouns.get(&id).map(|rows| (NOUN, rows));
        let text = || self.texts.get(&id).map(|rows| (TEXT, rows));
        match section {
            Section::Noun => noun(),
            Section::Text => text(),
            Section::Any => noun().or_else(text),
        }
    }

    /// Whether a row of `id` holds `needle` (lowercase) in its stored or decoded text.
    fn matches(&self, section: Section, id: u32, needle: &str) -> bool {
        self.rows(section, id).is_some_and(|(_, rows)| {
            rows.iter().any(|entry| {
                entry.raw.to_lowercase().contains(needle)
                    || sanitize_text(&entry.raw).to_lowercase().contains(needle)
            })
        })
    }

    fn lang_text(&self, section: Section, id: u32) -> Option<LangText> {
        let (kind, rows) = self.rows(section, id)?;
        let forms = rows
            .iter()
            .map(|entry| {
                let text = sanitize_text(&entry.raw);
                Form {
                    raw: (text != entry.raw).then(|| entry.raw.clone()),
                    form: entry.form,
                    text,
                    index: entry.index,
                }
            })
            .collect();
        Some(LangText {
            vfs_path: self.path.clone(),
            section: kind.to_owned(),
            forms,
        })
    }
}

/// The text tables of every language for one file stem.
pub(crate) struct TextSet<'a> {
    stem: &'a str,
    tables: Vec<(&'a str, std::sync::Arc<TextTable>)>,
}

impl<'a> TextSet<'a> {
    pub(crate) fn new(stem: &'a str, tables: Vec<(&'a str, std::sync::Arc<TextTable>)>) -> Self {
        Self { stem, tables }
    }

    /// Whether any language holds `needle` (lowercase) in a row of `id`.
    pub(crate) fn matches(&self, section: Section, id: u32, needle: &str) -> bool {
        self.tables
            .iter()
            .any(|(_, table)| table.matches(section, id, needle))
    }

    /// `None` when no language has the key.
    pub(crate) fn localize(&self, section: Section, id: u32) -> Option<Localized> {
        let texts: BTreeMap<String, LangText> = self
            .tables
            .iter()
            .filter_map(|(language, table)| {
                Some(((*language).to_owned(), table.lang_text(section, id)?))
            })
            .collect();
        (!texts.is_empty()).then(|| Localized {
            id: Id(id),
            table: self.stem.to_owned(),
            texts,
        })
    }
}
