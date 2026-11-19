//! SSH client in pure Rust on `russh`.
//!
//! `~/.ssh/config` (`HostName`, `User`, `Port`, `IdentityFile`, `ProxyJump`, `ConnectTimeout`) is
//! honoured, identities come from `ssh-agent` (a Unix socket or the Windows named pipe) and the
//! `~/.ssh` keys, and server keys are checked against `known_hosts`. A [`Connection`] runs commands,
//! opens the SFTP subsystem and forwards TCP ports; the `nie-ssh` binary is a thin shell over it.

mod config;
mod connect;
mod exec;
mod forward;

pub use config::{Hop, Target, default_known_hosts, home_dir, resolve_chain, split_authority};
pub use connect::{Connection, HostKeyPolicy};
pub use exec::{Output, exec_blocking};
pub use forward::Forward;

/// Re-exported so callers speak the same `russh` types as this crate.
pub use russh;
pub use russh_sftp;

/// Every failure of this crate, already phrased for the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(String);

impl Error {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Self(message)
    }
}

impl From<&str> for Error {
    fn from(message: &str) -> Self {
        Self(message.to_owned())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
