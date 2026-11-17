//! The file source behind the asset reader: anything that resolves a logical path to bytes.

use nie_formats::preloaded_vfs::PreloadedVfsBundle;

/// A read-only set of game files addressed by logical path (`data/common/...`).
pub trait GameFiles: Send + Sync + 'static {
    /// The bytes of one file, `None` when the path is unknown or unreadable.
    fn read(&self, path: &str) -> Option<Vec<u8>>;

    /// Every logical path, used only to enumerate directories.
    fn paths(&self) -> Box<dyn Iterator<Item = &str> + '_>;
}

/// The preloaded bundle served to WebAssembly builds, where no CPK can be streamed.
impl GameFiles for PreloadedVfsBundle {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        PreloadedVfsBundle::read(self, path).map(<[u8]>::to_vec)
    }

    fn paths(&self) -> Box<dyn Iterator<Item = &str> + '_> {
        Box::new(PreloadedVfsBundle::paths(self))
    }
}

/// The CPK-backed VFS of a local, SSH, rclone or HTTP installation.
#[cfg(not(target_arch = "wasm32"))]
impl GameFiles for nie_formats::vfs::Vfs {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        nie_formats::vfs::Vfs::read(self, path).ok()
    }

    fn paths(&self) -> Box<dyn Iterator<Item = &str> + '_> {
        Box::new(self.iter().map(|(path, _)| path))
    }
}
