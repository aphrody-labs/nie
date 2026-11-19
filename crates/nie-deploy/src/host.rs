//! Host primitives: write a file only when its content changed, modes and ownership.

use std::path::Path;

use anyhow::{Context, Result};

/// uid and gid of the service user `nie` (a fixed id on every host that runs the stack).
pub const SERVICE_ID: u32 = 10001;

/// Write `content` to `path` (parents created) when it differs; returns whether it wrote.
pub fn write_if_changed(path: &Path, content: &[u8], mode: u32) -> Result<bool> {
    if std::fs::read(path).is_ok_and(|old| old == content) {
        set_mode(path, mode)?;
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    std::fs::write(path, content).with_context(|| format!("write {}", path.display()))?;
    set_mode(path, mode)?;
    Ok(true)
}

/// Set unix permission bits.
pub fn set_mode(path: &Path, mode: u32) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
            .with_context(|| format!("chmod {}", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
    Ok(())
}

/// Give a path to a uid and gid.
pub fn chown(path: &Path, uid: u32, gid: u32) -> Result<()> {
    #[cfg(unix)]
    std::os::unix::fs::chown(path, Some(uid), Some(gid))
        .with_context(|| format!("chown {}", path.display()))?;
    #[cfg(not(unix))]
    let _ = (path, uid, gid);
    Ok(())
}

/// Whether the process runs as root.
#[must_use]
pub fn is_root() -> bool {
    #[cfg(unix)]
    {
        nix::unistd::Uid::effective().is_root()
    }
    #[cfg(not(unix))]
    {
        false
    }
}
