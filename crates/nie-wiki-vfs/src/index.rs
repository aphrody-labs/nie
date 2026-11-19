//! Lookups over the VFS listing: gamedata tables by stem, text languages, emblem files by hash.

use std::collections::{BTreeSet, HashMap, HashSet};

use nie_formats::vfs_read::VfsRead;

use crate::error::{Error, Result};
use crate::model::Language;

pub(crate) const GAMEDATA_DIR: &str = "data/common/gamedata/";
pub(crate) const TEXT_DIR: &str = "data/common/text/";
const EXTENSION: &str = ".cfg.bin";

/// The numeric version of `<stem>_<a>.<b>...cfg.bin`, or `None` when the name is another table.
fn version_of(file: &str, stem: &str) -> Option<Vec<u64>> {
    if file.strip_suffix(EXTENSION) == Some(stem) {
        return Some(Vec::new());
    }
    let rest = file.strip_prefix(stem)?.strip_prefix('_')?;
    let version = rest.strip_suffix(EXTENSION)?;
    version.split('.').map(|part| part.parse().ok()).collect()
}

/// The highest-versioned gamedata file of table `stem`.
pub(crate) fn find_table(vfs: &dyn VfsRead, stem: &str) -> Result<String> {
    vfs.entries()
        .filter(|(path, _)| path.starts_with(GAMEDATA_DIR))
        .filter_map(|(path, _)| {
            let file = path.rsplit('/').next()?;
            Some((version_of(file, stem)?, path))
        })
        .max()
        .map(|(_, path)| path.to_owned())
        .ok_or_else(|| Error::TableMissing {
            stem: stem.to_owned(),
            dir: GAMEDATA_DIR,
        })
}

/// BCP 47 tag of a text folder: `zh_hans` becomes `zh-Hans`.
fn tag_of(folder: &str) -> String {
    folder
        .split('_')
        .enumerate()
        .map(|(position, part)| match (position, part.len()) {
            (0, _) => part.to_owned(),
            (_, 4) => {
                let mut chars = part.chars();
                chars
                    .next()
                    .map(|first| first.to_uppercase().chain(chars).collect())
                    .unwrap_or_default()
            }
            _ => part.to_uppercase(),
        })
        .collect::<Vec<_>>()
        .join("-")
}

/// The text languages: every folder under `data/common/text` that holds the character names.
pub(crate) fn languages(vfs: &dyn VfsRead) -> Result<Vec<Language>> {
    let folders: BTreeSet<&str> = vfs
        .entries()
        .filter_map(|(path, _)| {
            let rest = path.strip_prefix(TEXT_DIR)?;
            let (folder, file) = rest.split_once('/')?;
            (file == format!("chara_text{EXTENSION}")).then_some(folder)
        })
        .collect();
    if folders.is_empty() {
        return Err(Error::NoLanguages);
    }
    Ok(folders
        .into_iter()
        .map(|folder| Language {
            folder: folder.to_owned(),
            tag: tag_of(folder),
        })
        .collect())
}

pub(crate) fn text_path(language: &str, stem: &str) -> String {
    format!("{TEXT_DIR}{language}/{stem}{EXTENSION}")
}

/// CRC32 of a file name without its extension, the way the game derives emblem ids.
pub(crate) fn name_hash(file_stem: &str) -> u32 {
    crc32fast::hash(file_stem.as_bytes())
}

/// For each wanted id, the VFS files whose name (without extension) hashes to it.
pub(crate) fn files_by_name_hash(vfs: &dyn VfsRead, wanted: &HashSet<u32>) -> HashMap<u32, Vec<String>> {
    let mut found: HashMap<u32, Vec<String>> = HashMap::new();
    for (path, _) in vfs.entries() {
        let file = path.rsplit('/').next().unwrap_or(path);
        let stem = file.split('.').next().unwrap_or(file);
        let hash = name_hash(stem);
        if wanted.contains(&hash) {
            found.entry(hash).or_default().push(path.to_owned());
        }
    }
    for paths in found.values_mut() {
        paths.sort();
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_only_the_exact_table() {
        assert_eq!(
            version_of("chara_param_1.03.66.00.cfg.bin", "chara_param"),
            Some(vec![1, 3, 66, 0])
        );
        assert_eq!(
            version_of("chara_param_table_config_0.00.00.00.cfg.bin", "chara_param"),
            None
        );
        assert_eq!(
            version_of("aura_skill_config_1.04.09.00.cfg.bin", "skill_config"),
            None
        );
    }

    #[test]
    fn tags_follow_bcp47() {
        assert_eq!(tag_of("fr"), "fr");
        assert_eq!(tag_of("zh_hans"), "zh-Hans");
        assert_eq!(tag_of("zh_hant"), "zh-Hant");
    }

    #[test]
    fn emblem_id_is_the_crc32_of_the_file_name() {
        assert_eq!(name_hash("em010001"), 0xD5AB_E1F0);
    }
}
