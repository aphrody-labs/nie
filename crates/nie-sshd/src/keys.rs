//! Host key and `authorized_keys`.

use anyhow::{Context, Result, bail};
use russh::keys::ssh_key::private::{Ed25519Keypair, KeypairData};
use russh::keys::ssh_key::{LineEnding, PrivateKey, PublicKey};
use std::io::Read;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// Loads the host key at `path`, creating an Ed25519 key (mode 0600) when it does not exist.
pub fn host_key_or_create(path: &Path) -> Result<PrivateKey> {
    if path.exists() {
        return russh::keys::load_secret_key(path, None)
            .with_context(|| format!("host key {}", path.display()));
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| dir.display().to_string())?;
    }
    let mut seed = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut seed))
        .context("reading /dev/urandom")?;
    let key = PrivateKey::new(
        KeypairData::Ed25519(Ed25519Keypair::from_seed(&seed)),
        "nie-sshd host key",
    )
    .context("building the host key")?;
    let pem = key.to_openssh(LineEnding::LF).context("encoding the host key")?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("creating {}", path.display()))?;
    std::io::Write::write_all(&mut file, pem.as_bytes())?;
    std::fs::write(path.with_extension("pub"), format!("{}\n", key.public_key().to_openssh()?))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(key)
}

/// The public keys allowed to log in.
#[derive(Clone, Debug, Default)]
pub struct AuthorizedKeys {
    keys: Vec<PublicKey>,
    /// Lines that carry options (`command=`, `from=`, ...): they are skipped, because the daemon
    /// cannot honour restrictions and must not widen what the line allowed.
    pub skipped: usize,
}

impl AuthorizedKeys {
    /// Parses `authorized_keys` text: `type base64 [comment]` lines, comments and blanks ignored.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut keys = Vec::new();
        let mut skipped = 0;
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            match PublicKey::from_openssh(line) {
                Ok(key) => keys.push(key),
                Err(_) => skipped += 1,
            }
        }
        Self { keys, skipped }
    }

    /// Reads and parses the file at `path`; a missing file is an error (nobody could log in).
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("authorized keys {}", path.display()))?;
        let keys = Self::parse(&text);
        if keys.is_empty() {
            bail!("{} holds no usable public key", path.display());
        }
        Ok(keys)
    }

    /// True when `key` is listed.
    #[must_use]
    pub fn allows(&self, key: &PublicKey) -> bool {
        self.keys.iter().any(|k| k.key_data() == key.key_data())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ED25519: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl nie-test";

    #[test]
    fn parses_plain_keys_and_skips_optioned_lines() {
        let text = format!("# comment\n\n{ED25519}\ncommand=\"/bin/true\" {ED25519}\nnot a key\n");
        let keys = AuthorizedKeys::parse(&text);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys.skipped, 2);
        let key = PublicKey::from_openssh(ED25519).unwrap();
        assert!(keys.allows(&key));
    }

    #[test]
    fn host_key_is_created_once_and_stable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state/host_ed25519");
        let first = host_key_or_create(&path).unwrap();
        let second = host_key_or_create(&path).unwrap();
        assert_eq!(first.public_key().key_data(), second.public_key().key_data());
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        assert!(path.with_extension("pub").is_file());
    }
}
