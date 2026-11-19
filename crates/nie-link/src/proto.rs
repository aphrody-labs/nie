//! Manifest and page naming shared by server, client and tools.
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A page is named by the BLAKE3 hash of its bytes.
pub type PageId = [u8; 32];

pub const FORMAT: u32 = 1;
pub const DEFAULT_PAGE_SIZE: u32 = 1 << 20;

#[derive(Debug, thiserror::Error)]
pub enum ProtoError {
    #[error("manifest: {0}")]
    Invalid(String),
    #[error("manifest json: {0}")]
    Json(#[from] serde_json::Error),
}

/// One file of the installation: `first_page..first_page + pages` indexes [`Manifest::page_id`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    /// `/`-separated path relative to the installation root.
    pub path: String,
    pub size: u64,
    pub mtime_ns: i64,
    pub first_page: u32,
    pub pages: u32,
}

/// The installation of one Steam build as a table of files over a table of pages.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format: u32,
    pub app_id: u32,
    pub build_id: String,
    /// `<depot>_<manifest id>` of Steam's own depot manifest of this build, when the origin has it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steam_manifest: Option<String>,
    pub install_dir: String,
    pub page_size: u32,
    pub files: Vec<FileEntry>,
    /// Page ids in file order, 32 bytes each, base64.
    pub page_table: String,
    /// BLAKE3 over everything that defines the content: names, sizes and page ids.
    pub manifest_id: String,
    #[serde(skip)]
    ids: Vec<PageId>,
}

impl Manifest {
    /// Assemble a manifest from files and their page ids and compute its id.
    pub fn assemble(
        app_id: u32,
        build_id: String,
        steam_manifest: Option<String>,
        install_dir: String,
        page_size: u32,
        files: Vec<FileEntry>,
        ids: Vec<PageId>,
    ) -> Self {
        let mut m = Self {
            format: FORMAT,
            app_id,
            build_id,
            steam_manifest,
            install_dir,
            page_size,
            files,
            page_table: String::new(),
            manifest_id: String::new(),
            ids,
        };
        m.seal();
        m
    }

    fn seal(&mut self) {
        let mut raw = Vec::with_capacity(self.ids.len() * 32);
        for id in &self.ids {
            raw.extend_from_slice(id);
        }
        self.page_table = base64::engine::general_purpose::STANDARD.encode(&raw);
        let mut h = blake3::Hasher::new();
        h.update(&self.format.to_le_bytes());
        h.update(&self.app_id.to_le_bytes());
        h.update(self.build_id.as_bytes());
        h.update(&self.page_size.to_le_bytes());
        for f in &self.files {
            h.update(&(f.path.len() as u64).to_le_bytes());
            h.update(f.path.as_bytes());
            h.update(&f.size.to_le_bytes());
            h.update(&f.first_page.to_le_bytes());
        }
        h.update(&raw);
        self.manifest_id = crate::hex(h.finalize().as_bytes());
    }

    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Parse and check the internal consistency of a served manifest.
    pub fn from_json(bytes: &[u8]) -> Result<Self, ProtoError> {
        let mut m: Self = serde_json::from_slice(bytes)?;
        if m.format != FORMAT {
            return Err(ProtoError::Invalid(format!("unsupported format {}", m.format)));
        }
        if m.page_size == 0 || !m.page_size.is_power_of_two() {
            return Err(ProtoError::Invalid("page size must be a power of two".into()));
        }
        let raw = base64::engine::general_purpose::STANDARD
            .decode(&m.page_table)
            .map_err(|e| ProtoError::Invalid(format!("page table: {e}")))?;
        if raw.len() % 32 != 0 {
            return Err(ProtoError::Invalid("page table length".into()));
        }
        m.ids = raw.as_chunks::<32>().0.to_vec();
        let mut next = 0u64;
        for f in &m.files {
            let want = f.size.div_ceil(u64::from(m.page_size));
            if u64::from(f.pages) != want || u64::from(f.first_page) != next {
                return Err(ProtoError::Invalid(format!("page range of {}", f.path)));
            }
            next += want;
        }
        if next != m.ids.len() as u64 {
            return Err(ProtoError::Invalid("page count".into()));
        }
        let claimed = m.manifest_id.clone();
        m.seal();
        if m.manifest_id != claimed {
            return Err(ProtoError::Invalid("manifest id does not match content".into()));
        }
        Ok(m)
    }

    #[must_use]
    pub fn page_count(&self) -> usize {
        self.ids.len()
    }

    #[must_use]
    pub fn page_id(&self, ordinal: u32) -> Option<&PageId> {
        self.ids.get(ordinal as usize)
    }

    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    /// Page ids of one file.
    #[must_use]
    pub fn file_ids(&self, f: &FileEntry) -> &[PageId] {
        let a = f.first_page as usize;
        &self.ids[a..a + f.pages as usize]
    }

    /// Length in bytes of page `index` of file `f` (the last page is short).
    #[must_use]
    pub fn page_len(&self, f: &FileEntry, index: u32) -> usize {
        let start = u64::from(index) * u64::from(self.page_size);
        let left = f.size.saturating_sub(start);
        usize::try_from(left.min(u64::from(self.page_size))).unwrap_or(0)
    }

    /// First occurrence of every page id: `(file index, byte offset, length)` for the origin.
    #[must_use]
    pub fn locate_index(&self) -> HashMap<PageId, (u32, u64, u32)> {
        let mut map = HashMap::with_capacity(self.ids.len());
        for (fi, f) in self.files.iter().enumerate() {
            for (i, id) in self.file_ids(f).iter().enumerate() {
                let len = self.page_len(f, i as u32);
                map.entry(*id).or_insert((
                    fi as u32,
                    i as u64 * u64::from(self.page_size),
                    len as u32,
                ));
            }
        }
        map
    }

    #[must_use]
    pub fn find(&self, path: &str) -> Option<&FileEntry> {
        self.files.iter().find(|f| f.path == path)
    }
}

/// What a name in a directory is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Node {
    Dir,
    File(usize),
}

/// Directory structure of a manifest, looked up case-insensitively like NTFS.
#[derive(Debug, Default)]
pub struct Tree {
    dirs: HashMap<String, Vec<(String, Node)>>,
    nodes: HashMap<String, (String, Node)>,
}

fn key(path: &str) -> String {
    path.to_uppercase()
}

impl Tree {
    /// Build the tree; `order` sorts siblings (ProjFS requires its own file-name collation).
    pub fn build(m: &Manifest, order: impl Fn(&str, &str) -> std::cmp::Ordering) -> Self {
        let mut t = Self::default();
        t.dirs.insert(String::new(), Vec::new());
        for (i, f) in m.files.iter().enumerate() {
            let mut dir = String::new();
            let mut parts = f.path.split('/').peekable();
            while let Some(part) = parts.next() {
                let full = if dir.is_empty() {
                    part.to_owned()
                } else {
                    format!("{dir}/{part}")
                };
                let node = if parts.peek().is_some() { Node::Dir } else { Node::File(i) };
                let k = key(&full);
                if !t.nodes.contains_key(&k) {
                    t.nodes.insert(k.clone(), (full.clone(), node));
                    t.dirs.entry(key(&dir)).or_default().push((part.to_owned(), node));
                    if node == Node::Dir {
                        t.dirs.entry(k).or_default();
                    }
                }
                dir = full;
            }
        }
        for children in t.dirs.values_mut() {
            children.sort_by(|a, b| order(&a.0, &b.0));
        }
        t
    }

    /// Children of a directory (`""` is the root).
    #[must_use]
    pub fn children(&self, dir: &str) -> Option<&[(String, Node)]> {
        self.dirs.get(&key(dir)).map(Vec::as_slice)
    }

    /// Exact stored path and kind of `path`, matched case-insensitively.
    #[must_use]
    pub fn lookup(&self, path: &str) -> Option<(&str, Node)> {
        if path.is_empty() {
            return Some(("", Node::Dir));
        }
        self.nodes.get(&key(path)).map(|(p, n)| (p.as_str(), *n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Manifest {
        let ps = 4u32;
        let files = vec![
            FileEntry { path: "a.bin".into(), size: 9, mtime_ns: 1, first_page: 0, pages: 3 },
            FileEntry { path: "data/b.cpk".into(), size: 4, mtime_ns: 2, first_page: 3, pages: 1 },
            FileEntry { path: "data/Sub/c.txt".into(), size: 0, mtime_ns: 3, first_page: 4, pages: 0 },
        ];
        let ids = (0..4u8).map(|i| [i; 32]).collect();
        Manifest::assemble(1, "7".into(), None, "g".into(), ps, files, ids)
    }

    #[test]
    fn json_round_trip_and_tamper() {
        let m = sample();
        let back = Manifest::from_json(&m.to_json()).unwrap();
        assert_eq!(back.manifest_id, m.manifest_id);
        assert_eq!(back.page_count(), 4);
        assert_eq!(back.page_len(&back.files[0], 2), 1);
        let mut v: serde_json::Value = serde_json::from_slice(&m.to_json()).unwrap();
        v["files"][0]["size"] = 10.into();
        assert!(Manifest::from_json(&serde_json::to_vec(&v).unwrap()).is_err());
    }

    #[test]
    fn tree_is_case_insensitive() {
        let m = sample();
        let t = Tree::build(&m, |a, b| a.to_uppercase().cmp(&b.to_uppercase()));
        let root = t.children("").unwrap();
        assert_eq!(root.len(), 2);
        assert_eq!(t.lookup("DATA/sub/C.TXT").unwrap().1, Node::File(2));
        assert_eq!(t.lookup("data/SUB").unwrap().1, Node::Dir);
        assert!(t.lookup("data/none").is_none());
        assert_eq!(t.children("Data").unwrap().len(), 2);
    }

    #[test]
    fn locate_dedups_to_first_occurrence() {
        let mut m = sample();
        m.ids[3] = m.ids[0];
        let idx = m.locate_index();
        assert_eq!(idx.len(), 3);
        assert_eq!(idx[&m.ids[0]].0, 0);
    }
}
