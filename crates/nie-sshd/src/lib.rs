//! SSH server in pure Rust on `russh`, for hosts that already run OpenSSH.
//!
//! Public-key authentication only, against an `authorized_keys` file the daemon owns. Commands
//! (`exec`), the SFTP subsystem and direct TCP forwarding are served; there is no pty and no shell,
//! so the daemon cannot replace an interactive OpenSSH login, only the machine-to-machine uses.
//! The daemon serves the one account it runs as. It listens on the addresses of the WireGuard
//! interfaces, on addresses given explicitly, or on a systemd socket.

#[cfg(unix)]
mod keys;
#[cfg(unix)]
mod listen;
#[cfg(unix)]
mod server;
#[cfg(unix)]
mod sftp;

#[cfg(unix)]
pub use keys::{AuthorizedKeys, host_key_or_create};
#[cfg(unix)]
pub use listen::{Listener, wireguard_addresses};
#[cfg(unix)]
pub use server::{Settings, serve};
