//! Raw file bytes as a Bevy asset.

use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetLoader, LoadContext};
use bevy_reflect::TypePath;

/// The unmodified bytes of a game file, loaded for the extensions `GameBytesLoader` claims.
#[derive(Asset, TypePath, Debug, Clone, PartialEq, Eq)]
pub struct GameBytes(pub Vec<u8>);

/// Loads a file as [`GameBytes`]. Claims the `bin` extension, which stays out of the way of the
/// typed loaders of other crates and gives callers an explicit raw-bytes request:
/// `asset_server.load::<GameBytes>("game://data/x.cfg.bin")`.
#[derive(TypePath, Debug, Default, Clone, Copy)]
pub struct GameBytesLoader;

impl AssetLoader for GameBytesLoader {
    type Asset = GameBytes;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<GameBytes, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(GameBytes(bytes))
    }

    fn extensions(&self) -> &[&str] {
        &["bin"]
    }
}
