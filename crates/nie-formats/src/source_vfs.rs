//! The CPK VFS over any [`SourceTree`]: the browser's mount of the installation.
//!
//! [`SourceVfs::mount`] reads and decrypts `cpk_list.cfg.bin`, which is the whole logical index
//! (`path -> pack`); a pack's table of contents is read, with ranged reads, the first time one of
//! its files is asked for, and only the bytes of the requested entry are ever fetched. This is
//! what [`crate::vfs::Vfs`] does on a disk, written once over [`ByteSource`] so that a wasm build
//! runs it unchanged.
//!
//! Decoders are synchronous and a browser fetch is not. [`MemoryVfs`] bridges the two: a
//! [`VfsRead`] over bytes already fetched that records every path it was asked for and did not
//! hold, and [`SourceVfs::run`] fetches those paths and runs the decoder again until it asks for
//! nothing new. The decoder is therefore never rewritten for the browser.

use crate::FormatError;
use crate::byte_source::{ByteSource, SourceTree};
use crate::cpk::CpkReader;
use crate::cpk_list;
use crate::vfs_read::VfsRead;
use alloc::boxed::Box;
use alloc::collections::BTreeSet;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::collections::HashMap;
use std::sync::Mutex;

/// Where `cpk_list.cfg.bin` can sit relative to the tree root: the data folder itself, or an
/// installation root that contains `data/`.
const INDEX_LOCATIONS: [&str; 2] = ["", "data/"];

const INDEX_FILE: &str = "cpk_list.cfg.bin";

/// Upper bound of fetch rounds in [`SourceVfs::run`]; every round adds at least one path.
const MAX_ROUNDS: usize = 512;

/// The logical index: path to `(pack, size)`.
#[derive(Debug, Default)]
struct Index {
    packs: Vec<String>,
    files: HashMap<String, (u32, u32)>,
}

/// The mounted installation.
pub struct SourceVfs<T: SourceTree> {
    tree: T,
    /// `""` or `"data/"`: the tree prefix of the folder holding `cpk_list.cfg.bin` and `packs/`.
    prefix: &'static str,
    index: Arc<Index>,
    #[allow(clippy::type_complexity)]
    tocs: Mutex<HashMap<String, Arc<(CpkReader, T::File)>>>,
}

impl<T: SourceTree> SourceVfs<T> {
    /// Mounts the installation held by `tree`.
    ///
    /// # Errors
    /// The tree holds no readable `cpk_list.cfg.bin`, or it is neither known envelope.
    pub async fn mount(tree: T) -> Result<Self, FormatError> {
        let mut found = None;
        for prefix in INDEX_LOCATIONS {
            if let Ok(file) = tree.open(&alloc::format!("{prefix}{INDEX_FILE}")).await {
                found = Some((prefix, file));
                break;
            }
        }
        let (prefix, file) = found.ok_or(FormatError::Corrupt(
            "the source holds no cpk_list.cfg.bin (pick the game folder or its data folder)",
        ))?;
        let entries = cpk_list::parse(&file.read_all().await?)?;
        let mut index = Index::default();
        let mut pack_ids: HashMap<String, u32> = HashMap::new();
        index.files.reserve(entries.len());
        for entry in entries {
            let id = match pack_ids.get(&entry.pack) {
                Some(id) => *id,
                None => {
                    let id = u32::try_from(index.packs.len())
                        .map_err(|_| FormatError::Corrupt("too many packs"))?;
                    index.packs.push(entry.pack.clone());
                    pack_ids.insert(entry.pack, id);
                    id
                }
            };
            index.files.insert(entry.path, (id, entry.size));
        }
        Ok(Self {
            tree,
            prefix,
            index: Arc::new(index),
            tocs: Mutex::new(HashMap::new()),
        })
    }

    /// Number of logical files the index declares.
    #[must_use]
    pub fn len(&self) -> usize {
        self.index.files.len()
    }

    /// True when the index declares no file.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.index.files.is_empty()
    }

    /// Number of distinct packs the index references (loose files excluded).
    #[must_use]
    pub fn pack_count(&self) -> usize {
        self.index.packs.iter().filter(|p| !p.is_empty()).count()
    }

    /// Every logical path with its declared size.
    pub fn entries(&self) -> impl Iterator<Item = (&str, u64)> + '_ {
        self.index
            .files
            .iter()
            .map(|(path, (_, size))| (path.as_str(), u64::from(*size)))
    }

    /// True when `path` is a logical path of the installation.
    #[must_use]
    pub fn contains(&self, path: &str) -> bool {
        self.index.files.contains_key(path)
    }

    /// An empty [`MemoryVfs`] over this index, to be filled by [`Self::run`].
    #[must_use]
    pub fn memory(&self) -> Arc<MemoryVfs> {
        Arc::new(MemoryVfs {
            index: Arc::clone(&self.index),
            slots: Mutex::new(HashMap::new()),
            misses: Mutex::new(BTreeSet::new()),
        })
    }

    /// The decoded bytes of one entry. Only that entry's bytes and its pack's table are fetched.
    ///
    /// # Errors
    /// The path is not indexed, or the pack cannot be read or decoded.
    pub async fn read(&self, path: &str) -> Result<Vec<u8>, FormatError> {
        let &(pack, _) = self
            .index
            .files
            .get(path)
            .ok_or_else(|| FormatError::Malformed(alloc::format!("not in the VFS: {path}")))?;
        let pack = &self.index.packs[pack as usize];
        if pack.is_empty() {
            let relative = path.strip_prefix("data/").unwrap_or(path);
            let file = self
                .tree
                .open(&alloc::format!("{}{relative}", self.prefix))
                .await?;
            return file.read_all().await;
        }
        let cached = self
            .tocs
            .lock()
            .map_err(|_| FormatError::Corrupt("VFS cache lock poisoned"))?
            .get(pack)
            .cloned();
        let pack_file = if let Some(cached) = cached {
            cached
        } else {
            let file = self
                .tree
                .open(&alloc::format!("{}packs/{pack}", self.prefix))
                .await?;
            let reader = CpkReader::from_source(pack, &file).await?;
            let opened = Arc::new((reader, file));
            self.tocs
                .lock()
                .map_err(|_| FormatError::Corrupt("VFS cache lock poisoned"))?
                .insert(pack.clone(), Arc::clone(&opened));
            opened
        };
        let (reader, file) = &*pack_file;
        // Full path first: same-named files of different directories of one pack must not
        // collide (`find_file` then falls back to a case-insensitive or basename match).
        let entry = reader
            .find_file(path)
            .ok_or(FormatError::Corrupt("fichier non trouve dans le CPK"))?;
        reader.extract_from_source(entry, file).await
    }

    /// Fetches `paths` into `memory`; a path that cannot be read is recorded as absent, so the
    /// decoder sees an ordinary read error rather than looping.
    pub async fn fetch(&self, memory: &MemoryVfs, paths: impl IntoIterator<Item = String>) {
        for path in paths {
            let slot = match self.read(&path).await {
                Ok(bytes) => Slot::Bytes(bytes.into()),
                Err(error) => Slot::Absent(error.to_string()),
            };
            if let Ok(mut slots) = memory.slots.lock() {
                slots.insert(path, slot);
            }
        }
    }

    /// Runs the synchronous decoder `decode` over `memory`, fetching every entry it asks for
    /// and was not holding, until a run asks for nothing new.
    ///
    /// `decode` may therefore run several times: it must be a pure function of the bytes it reads.
    pub async fn run<R>(&self, memory: &MemoryVfs, mut decode: impl FnMut(&MemoryVfs) -> R) -> R {
        let mut out = decode(memory);
        for _ in 0..MAX_ROUNDS {
            let wanted = memory.take_misses();
            if wanted.is_empty() {
                break;
            }
            self.fetch(memory, wanted).await;
            out = decode(memory);
        }
        out
    }
}

enum Slot {
    Bytes(Arc<[u8]>),
    Absent(String),
}

/// Entries already fetched, readable synchronously. See the module documentation.
pub struct MemoryVfs {
    index: Arc<Index>,
    slots: Mutex<HashMap<String, Slot>>,
    misses: Mutex<BTreeSet<String>>,
}

impl MemoryVfs {
    /// Number of entries held.
    #[must_use]
    pub fn loaded(&self) -> usize {
        self.slots.lock().map_or(0, |slots| slots.len())
    }

    fn take_misses(&self) -> Vec<String> {
        self.misses
            .lock()
            .map(|mut misses| core::mem::take(&mut *misses).into_iter().collect())
            .unwrap_or_default()
    }
}

impl VfsRead for MemoryVfs {
    fn read(&self, path: &str) -> Result<Vec<u8>, FormatError> {
        let slots = self
            .slots
            .lock()
            .map_err(|_| FormatError::Corrupt("VFS cache lock poisoned"))?;
        match slots.get(path) {
            Some(Slot::Bytes(bytes)) => Ok(bytes.to_vec()),
            Some(Slot::Absent(reason)) => Err(FormatError::Malformed(reason.clone())),
            None => {
                drop(slots);
                if let Ok(mut misses) = self.misses.lock() {
                    misses.insert(path.to_owned());
                }
                Err(FormatError::Malformed(alloc::format!(
                    "not fetched yet: {path}"
                )))
            }
        }
    }

    fn contains(&self, path: &str) -> bool {
        self.index.files.contains_key(path)
    }

    fn entries(&self) -> Box<dyn Iterator<Item = (&str, u64)> + '_> {
        Box::new(
            self.index
                .files
                .iter()
                .map(|(path, (_, size))| (path.as_str(), u64::from(*size))),
        )
    }
}
