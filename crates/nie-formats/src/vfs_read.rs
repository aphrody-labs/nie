//! The synchronous, read-only view of a mounted VFS that decoders consume.
//!
//! Everything above the byte layer (the wiki, the typed game-data tables, the previews) only
//! lists logical paths and reads whole entries. [`VfsRead`] is that surface, so the same code runs
//! over the native [`crate::vfs::Vfs`] and over the browser's [`crate::source_vfs::MemoryVfs`].

use crate::FormatError;
use alloc::boxed::Box;
use alloc::vec::Vec;

/// A mounted VFS, read-only.
pub trait VfsRead {
    /// The decoded bytes of the entry at `path`.
    ///
    /// # Errors
    /// The path is not served or its pack cannot be read.
    fn read(&self, path: &str) -> Result<Vec<u8>, FormatError>;

    /// True when `path` is a servable logical path.
    fn contains(&self, path: &str) -> bool;

    /// Every servable logical path with its extracted size in bytes (0 when unknown).
    fn entries(&self) -> Box<dyn Iterator<Item = (&str, u64)> + '_>;
}
