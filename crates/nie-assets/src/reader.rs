//! The Bevy asset reader over a mounted VFS.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::cache::DiskCache;
use crate::files::GameFiles;
use bevy_app::App;
use bevy_asset::AssetApp;
use bevy_asset::io::{
    AssetReader, AssetReaderError, AssetSourceBuilder, AssetSourceId, PathStream, Reader, VecReader,
};

/// The asset source name: `game://data/common/...`.
pub const GAME_SOURCE: &str = "game";

/// Reads assets from game files: a VFS (local install, SSH, rclone, HTTP) through the CPK index,
/// or a preloaded bundle.
///
/// The files are shared by `Arc`: Bevy may build the reader several times and the CPK table cache must
/// stay unique. With a [`DiskCache`], a file is fetched once and then served from disk.
#[derive(Clone)]
pub struct VfsAssetReader {
    files: Arc<dyn GameFiles>,
    cache: Option<Arc<DiskCache>>,
}

impl VfsAssetReader {
    /// A reader over mounted game files (a VFS, or a preloaded bundle on the web).
    #[must_use]
    pub fn new<F: GameFiles>(files: Arc<F>) -> Self {
        Self { files, cache: None }
    }

    /// Adds a read-through disk cache.
    #[must_use]
    pub fn with_cache(mut self, cache: Option<Arc<DiskCache>>) -> Self {
        self.cache = cache;
        self
    }

    /// The logical VFS path of a Bevy path: `/` separators on every host, no root, no `.`.
    fn logical(path: &Path) -> String {
        let mut out = String::new();
        for component in path.components() {
            if let std::path::Component::Normal(part) = component {
                if !out.is_empty() {
                    out.push('/');
                }
                out.push_str(&part.to_string_lossy());
            }
        }
        out
    }

    fn fetch(&self, logical: &str) -> Option<Vec<u8>> {
        if let Some(bytes) = self.cache.as_ref().and_then(|cache| cache.get(logical)) {
            return Some(bytes);
        }
        let bytes = self.files.read(logical)?;
        if let Some(cache) = &self.cache
            && let Err(error) = cache.put(logical, &bytes)
        {
            tracing::warn!(%error, logical, "asset cache write failed");
        }
        Some(bytes)
    }
}

impl AssetReader for VfsAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        self.fetch(&Self::logical(path))
            .map(VecReader::new)
            .ok_or_else(|| AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        // CPK archives carry no `.meta` files; report that instead of inventing an empty reader.
        Err::<VecReader, _>(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        let mut prefix = Self::logical(path);
        if !prefix.is_empty() {
            prefix.push('/');
        }
        // Direct children only, deduplicated: `ls` semantics, not `find`.
        let mut children: Vec<PathBuf> = self
            .files
            .paths()
            .filter_map(|entry| entry.strip_prefix(prefix.as_str()))
            .map(|rest| rest.split('/').next().unwrap_or(rest))
            .map(|child| path.join(child))
            .collect();
        children.sort();
        children.dedup();
        if children.is_empty() {
            return Err(AssetReaderError::NotFound(path.to_path_buf()));
        }
        Ok(Box::new(futures_lite::stream::iter(children)))
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        let mut prefix = Self::logical(path);
        prefix.push('/');
        Ok(self
            .files
            .paths()
            .any(|entry| entry.starts_with(prefix.as_str())))
    }
}

/// Registers `game://` on an app. It must run **before** `AssetPlugin`: sources are built when
/// the asset server is created.
pub fn register_game_source(app: &mut App, reader: VfsAssetReader) -> &mut App {
    app.register_asset_source(
        AssetSourceId::Name(GAME_SOURCE.into()),
        AssetSourceBuilder::new(move || Box::new(reader.clone())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_paths_use_forward_slashes() {
        assert_eq!(
            VfsAssetReader::logical(Path::new("data/common/x.g4md")),
            "data/common/x.g4md"
        );
        assert_eq!(VfsAssetReader::logical(Path::new("./data/./a")), "data/a");
        assert_eq!(VfsAssetReader::logical(Path::new("")), "");
    }
}
