//! The plugin that registers `game://`.

use std::sync::Arc;

use bevy_app::{App, Plugin};

use crate::config::{AssetsConfig, ConfigError};
use crate::files::GameFiles;
use crate::reader::{VfsAssetReader, register_game_source};

/// Registers the `game://` asset source over mounted game files.
///
/// Add it **before** `AssetPlugin` (or `DefaultPlugins`).
#[derive(Clone)]
pub struct GameAssetsPlugin {
    reader: VfsAssetReader,
}

impl GameAssetsPlugin {
    /// Mounts the source described by `config` (native builds).
    ///
    /// # Errors
    /// Fails when the source cannot be mounted or the cache cannot be opened.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(config: &AssetsConfig) -> Result<Self, ConfigError> {
        Self::from_files(Arc::new(config.open_vfs()?), config)
    }

    /// Wraps already mounted files, applying only the cache settings of `config`.
    ///
    /// # Errors
    /// Fails when the cache cannot be opened.
    pub fn from_files<F: GameFiles>(
        files: Arc<F>,
        config: &AssetsConfig,
    ) -> Result<Self, ConfigError> {
        let reader = VfsAssetReader::new(files).with_cache(config.open_cache()?);
        Ok(Self { reader })
    }
}

impl Plugin for GameAssetsPlugin {
    fn build(&self, app: &mut App) {
        register_game_source(app, self.reader.clone());
    }
}
