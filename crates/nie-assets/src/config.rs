//! Which game to mount and where to cache it: a TOML file, overridden by the environment.
//!
//! ```toml
//! # A local install, `ssh://host/path`, `r2://remote/bucket/path`, `gdrive://remote/path`,
//! # `rclone://remote/path` or `https://host/path` (HTTP range reads).
//! source = "ssh://prd/srv/rosegriffon/game/ievr"
//! cache_dir = "/var/cache/nie-assets"
//! cache_max_mib = 4096
//! ```
//!
//! Credentials never appear here: SSH uses the user's SSH configuration, R2 and Google Drive use
//! the rclone remotes already configured on the host.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use nie_formats::FormatError;
use nie_formats::vfs::Vfs;
use serde::{Deserialize, Serialize};

use crate::cache::DiskCache;

/// Environment variable naming a TOML config file.
pub const CONFIG_ENV: &str = "NIE_ASSETS_CONFIG";
/// Environment variable overriding `cache_dir`.
pub const CACHE_DIR_ENV: &str = "NIE_ASSETS_CACHE_DIR";
/// Environment variable overriding `cache_max_mib`.
pub const CACHE_MAX_ENV: &str = "NIE_ASSETS_CACHE_MAX_MIB";
/// An rclone `remote:path` of the game, used when no explicit source is set.
pub const CLOUD_REMOTE_ENV: &str = "NIE_VFS_CLOUD_REMOTE";

const DEFAULT_CACHE_MIB: u64 = 2048;

/// Why a configuration could not be read or mounted.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The config file could not be read.
    #[error("cannot read {path}: {source}")]
    Read {
        /// The file.
        path: PathBuf,
        /// The cause.
        source: std::io::Error,
    },
    /// The config file is not valid TOML for [`AssetsConfig`].
    #[error("invalid {path}: {source}")]
    Parse {
        /// The file.
        path: PathBuf,
        /// The cause.
        source: toml::de::Error,
    },
    /// The source could not be mounted.
    #[error("cannot mount the game source: {0}")]
    Mount(String),
    /// The cache directory could not be opened.
    #[error("cannot open the asset cache: {0}")]
    Cache(std::io::Error),
}

/// Where the game lives and how reads are cached.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AssetsConfig {
    /// Installation path or remote URI. Empty means automatic discovery (`NIE_GAME_SOURCE`,
    /// the saved source, the working directory, Steam libraries).
    pub source: String,
    /// Read-through cache directory; no cache when unset.
    pub cache_dir: Option<PathBuf>,
    /// Cache byte budget in MiB.
    pub cache_max_mib: Option<u64>,
}

impl AssetsConfig {
    /// A configuration for one source without cache.
    #[must_use]
    pub fn for_source(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            ..Self::default()
        }
    }

    /// Parses TOML text.
    ///
    /// # Errors
    /// Fails on invalid TOML or unknown keys.
    pub fn from_toml(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }

    /// Reads a TOML file.
    ///
    /// # Errors
    /// Fails when the file is unreadable or invalid.
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_owned(),
            source,
        })?;
        Self::from_toml(&text).map_err(|source| ConfigError::Parse {
            path: path.to_owned(),
            source,
        })
    }

    /// The file named by `NIE_ASSETS_CONFIG` (when set), then environment overrides.
    ///
    /// # Errors
    /// Fails when the named config file is unreadable or invalid.
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut config = match std::env::var_os(CONFIG_ENV).filter(|v| !v.is_empty()) {
            Some(path) => Self::from_file(Path::new(&path))?,
            None => Self::default(),
        };
        config.apply_env();
        Ok(config)
    }

    /// Applies the environment overrides (`NIE_ASSETS_CACHE_DIR`, `NIE_ASSETS_CACHE_MAX_MIB`,
    /// and `NIE_VFS_CLOUD_REMOTE` when no source is set).
    pub fn apply_env(&mut self) {
        let var = |key: &str| std::env::var(key).ok().filter(|v| !v.trim().is_empty());
        if let Some(dir) = var(CACHE_DIR_ENV) {
            self.cache_dir = Some(PathBuf::from(dir));
        }
        if let Some(max) = var(CACHE_MAX_ENV).and_then(|v| v.trim().parse().ok()) {
            self.cache_max_mib = Some(max);
        }
        if self.source.is_empty()
            && let Some(remote) = var(CLOUD_REMOTE_ENV)
        {
            self.source = rclone_uri(&remote);
        }
    }

    /// Mounts the source. Local paths, `ssh://`, `r2://`, `gdrive://`, `rclone://` and `https://`
    /// sources are read in place; remote mounts fetch the file index and then only the ranges
    /// each asset needs.
    ///
    /// # Errors
    /// Fails when the source cannot be found or mounted.
    pub fn open_vfs(&self) -> Result<Vfs, ConfigError> {
        let hint = (!self.source.is_empty()).then_some(self.source.as_str());
        nie_formats::vfs::open_source(hint)
            .map_err(|e: FormatError| ConfigError::Mount(e.to_string()))
    }

    /// Opens the read-through cache when one is configured.
    ///
    /// # Errors
    /// Fails when the cache directory cannot be created.
    pub fn open_cache(&self) -> Result<Option<Arc<DiskCache>>, ConfigError> {
        let Some(dir) = &self.cache_dir else {
            return Ok(None);
        };
        let max = self.cache_max_mib.unwrap_or(DEFAULT_CACHE_MIB) * 1024 * 1024;
        DiskCache::open(dir, max)
            .map(|cache| Some(Arc::new(cache)))
            .map_err(ConfigError::Cache)
    }
}

/// `remote:path` becomes `rclone://remote/path`; an existing URI is returned unchanged.
fn rclone_uri(spec: &str) -> String {
    if spec.contains("://") {
        return spec.to_owned();
    }
    match spec.split_once(':') {
        Some((remote, path)) => format!("rclone://{remote}/{}", path.trim_start_matches('/')),
        None => format!("rclone://{spec}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_toml_and_rejects_unknown_keys() {
        let config = AssetsConfig::from_toml(
            "source = \"ssh://prd/srv/game\"\ncache_dir = \"/tmp/c\"\ncache_max_mib = 8\n",
        )
        .unwrap();
        assert_eq!(config.source, "ssh://prd/srv/game");
        assert_eq!(config.cache_max_mib, Some(8));
        assert!(AssetsConfig::from_toml("sauce = 1").is_err());
    }

    #[test]
    fn cloud_remote_specs_become_rclone_uris() {
        assert_eq!(rclone_uri("drive:Games/ievr"), "rclone://drive/Games/ievr");
        assert_eq!(rclone_uri("drive:"), "rclone://drive/");
        assert_eq!(rclone_uri("r2://r2/b"), "r2://r2/b");
    }
}
