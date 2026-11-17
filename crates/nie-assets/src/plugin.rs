//! The plugin that mounts the game and registers `game://`.

use std::sync::Arc;

use bevy_app::{App, Plugin};
use nie_formats::vfs::Vfs;

use crate::config::{AssetsConfig, ConfigError};
use crate::reader::{VfsAssetReader, register_game_source};

/// Mounts the configured game and registers the `game://` asset source.
///
/// Add it **before** `AssetPlugin` (or `DefaultPlugins`). The mounted VFS is available to
/// callers through [`GameAssetsPlugin::vfs`].
#[derive(Clone)]
pub struct GameAssetsPlugin {
    reader: VfsAssetReader,
    vfs: Arc<Vfs>,
}

impl GameAssetsPlugin {
    /// Mounts the source described by `config`.
    ///
    /// # Errors
    /// Fails when the source cannot be mounted or the cache cannot be opened.
    pub fn new(config: &AssetsConfig) -> Result<Self, ConfigError> {
        let vfs = Arc::new(config.open_vfs()?);
        Self::from_vfs(vfs, config)
    }

    /// Wraps an already mounted VFS, applying only the cache settings of `config`.
    ///
    /// # Errors
    /// Fails when the cache cannot be opened.
    pub fn from_vfs(vfs: Arc<Vfs>, config: &AssetsConfig) -> Result<Self, ConfigError> {
        let reader = VfsAssetReader::new(Arc::clone(&vfs)).with_cache(config.open_cache()?);
        Ok(Self { reader, vfs })
    }

    /// The mounted VFS.
    #[must_use]
    pub fn vfs(&self) -> &Arc<Vfs> {
        &self.vfs
    }
}

impl Plugin for GameAssetsPlugin {
    fn build(&self, app: &mut App) {
        register_game_source(app, self.reader.clone());
    }
}
