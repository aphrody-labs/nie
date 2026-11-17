//! The installed game as a Bevy asset source.
//!
//! nie never ships game assets. This crate mounts the user's own installation (a local folder, an
//! SSH host, an rclone remote such as R2 or Google Drive) through the nie VFS, which resolves a
//! logical path to its CRI CPK archive, decodes CRILAYLA and reads only the byte ranges it needs.
//! Bevy then loads `game://data/common/...` like any other asset; nothing is extracted to disk
//! except an optional, size-bounded read-through cache outside the repository.
//!
//! - [`AssetsConfig`]: which source, which cache, from TOML or the environment.
//! - [`VfsAssetReader`]: the Bevy [`AssetReader`](bevy_asset::io::AssetReader) over a mounted VFS.
//! - [`GameAssetsPlugin`]: mounts the source and registers `game://`; add it before `AssetPlugin`.
//! - [`GameBytes`] and [`GameBytesLoader`]: raw bytes of any file, for formats decoded elsewhere.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod bytes;
mod cache;
mod config;
mod plugin;
mod reader;

pub use bytes::{GameBytes, GameBytesLoader};
pub use cache::DiskCache;
pub use config::{AssetsConfig, ConfigError};
pub use plugin::GameAssetsPlugin;
pub use reader::{GAME_SOURCE, VfsAssetReader, register_game_source};
