//! Positioned, asynchronous reads of the game's bytes, independent of where they live.
//!
//! The installation is read through two small traits and nothing else:
//!
//! - [`ByteSource`]: one file, read by `(offset, length)`. Native hosts back it with positioned
//!   file reads, a browser with the File System Access API or HTTP `Range` requests.
//! - [`SourceTree`]: a folder of such files, opened by a `/`-separated relative path
//!   (`data/cpk_list.cfg.bin`, `data/packs/<pack>.cpk`).
//!
//! [`crate::source_vfs::SourceVfs`] mounts the CPK VFS over a [`SourceTree`], so the code that
//! decrypts `cpk_list.cfg.bin`, parses the pack tables and decompresses entries is the one the
//! native [`crate::vfs::Vfs`] runs, compiled for `wasm32-unknown-unknown` too. No type here owns
//! a runtime or a thread: a browser awaits its `fetch` and `Blob` promises, a test drives the
//! futures with any executor.

use crate::FormatError;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

/// One readable file of the installation.
///
/// The futures are deliberately not `Send`: on `wasm32-unknown-unknown` they hold JavaScript
/// values.
#[allow(async_fn_in_trait, clippy::len_without_is_empty)]
pub trait ByteSource {
    /// Size of the file in bytes.
    ///
    /// # Errors
    /// The size cannot be obtained.
    async fn len(&self) -> Result<u64, FormatError>;

    /// Exactly `len` bytes starting at `offset`.
    ///
    /// # Errors
    /// The range lies outside the file, or the transport failed.
    async fn read_at(&self, offset: u64, len: usize) -> Result<Vec<u8>, FormatError>;

    /// The whole file.
    ///
    /// # Errors
    /// The size or the bytes cannot be read.
    async fn read_all(&self) -> Result<Vec<u8>, FormatError> {
        let len = usize::try_from(self.len().await?)
            .map_err(|_| FormatError::Corrupt("file exceeds the address space"))?;
        self.read_at(0, len).await
    }
}

/// A folder of [`ByteSource`] files addressed by relative path.
#[allow(async_fn_in_trait)]
pub trait SourceTree {
    /// The file type this tree opens.
    type File: ByteSource;

    /// Opens `path` (`/`-separated, relative to the tree root, no `..`).
    ///
    /// # Errors
    /// The file does not exist or cannot be opened.
    async fn open(&self, path: &str) -> Result<Self::File, FormatError>;
}

/// In-memory file: fixtures, tests and bytes already fetched.
#[derive(Clone, Debug, Default)]
pub struct MemorySource(Arc<[u8]>);

impl MemorySource {
    /// Wraps `bytes`.
    #[must_use]
    pub fn new(bytes: impl Into<Arc<[u8]>>) -> Self {
        Self(bytes.into())
    }
}

impl ByteSource for MemorySource {
    async fn len(&self) -> Result<u64, FormatError> {
        Ok(self.0.len() as u64)
    }

    async fn read_at(&self, offset: u64, len: usize) -> Result<Vec<u8>, FormatError> {
        let start = usize::try_from(offset)
            .map_err(|_| FormatError::Corrupt("offset exceeds the address space"))?;
        let end = start
            .checked_add(len)
            .ok_or(FormatError::Corrupt("range overflow"))?;
        self.0
            .get(start..end)
            .map(<[u8]>::to_vec)
            .ok_or(FormatError::TooShort {
                got: self.0.len(),
                need: end,
            })
    }
}

/// In-memory tree: path to bytes.
#[derive(Clone, Debug, Default)]
pub struct MemoryTree(BTreeMap<String, MemorySource>);

impl MemoryTree {
    /// An empty tree.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds or replaces the file at `path`.
    pub fn insert(&mut self, path: impl Into<String>, bytes: impl Into<Arc<[u8]>>) {
        self.0.insert(path.into(), MemorySource::new(bytes));
    }

    /// The paths of the tree, sorted.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }
}

impl SourceTree for MemoryTree {
    type File = MemorySource;

    async fn open(&self, path: &str) -> Result<MemorySource, FormatError> {
        self.0
            .get(path)
            .cloned()
            .ok_or_else(|| FormatError::Malformed(alloc::format!("no such file: {path}")))
    }
}

/// Folder on the local disk, read with positioned reads and never mapped whole.
#[cfg(all(feature = "std", not(target_arch = "wasm32")))]
#[derive(Clone, Debug)]
pub struct FsTree(std::path::PathBuf);

#[cfg(all(feature = "std", not(target_arch = "wasm32")))]
impl FsTree {
    /// A tree rooted at `root`.
    #[must_use]
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        Self(root.into())
    }
}

/// A local file opened by [`FsTree`].
#[cfg(all(feature = "std", not(target_arch = "wasm32")))]
#[derive(Clone, Debug)]
pub struct FsSource(std::path::PathBuf);

#[cfg(all(feature = "std", not(target_arch = "wasm32")))]
impl SourceTree for FsTree {
    type File = FsSource;

    async fn open(&self, path: &str) -> Result<FsSource, FormatError> {
        if path.split('/').any(|part| part == ".." || part.contains('\\')) {
            return Err(FormatError::Corrupt("path escapes the tree"));
        }
        let full = self.0.join(path);
        if full.is_file() {
            Ok(FsSource(full))
        } else {
            Err(FormatError::Malformed(alloc::format!("no such file: {path}")))
        }
    }
}

#[cfg(all(feature = "std", not(target_arch = "wasm32")))]
impl ByteSource for FsSource {
    async fn len(&self) -> Result<u64, FormatError> {
        std::fs::metadata(&self.0)
            .map(|m| m.len())
            .map_err(|e| FormatError::Malformed(e.to_string()))
    }

    async fn read_at(&self, offset: u64, len: usize) -> Result<Vec<u8>, FormatError> {
        crate::vfs::read_file_range(&self.0, offset, len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_lite::future::block_on;

    #[test]
    fn memory_source_reads_ranges_and_rejects_overruns() {
        let source = MemorySource::new(*b"0123456789");
        assert_eq!(block_on(source.len()).unwrap(), 10);
        assert_eq!(block_on(source.read_at(3, 4)).unwrap(), b"3456");
        assert_eq!(block_on(source.read_all()).unwrap(), b"0123456789");
        assert!(block_on(source.read_at(8, 4)).is_err());
        assert!(block_on(source.read_at(u64::MAX, 2)).is_err());
    }

    #[test]
    fn memory_tree_opens_by_path() {
        let mut tree = MemoryTree::new();
        tree.insert("data/a.bin", *b"abc");
        assert!(block_on(tree.open("data/a.bin")).is_ok());
        assert!(block_on(tree.open("data/b.bin")).is_err());
    }
}
