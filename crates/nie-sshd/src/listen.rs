//! Where the daemon listens: systemd socket, explicit addresses, or the WireGuard interfaces.

use anyhow::{Context, Result, bail};
use nix::ifaddrs::getifaddrs;
use std::net::{IpAddr, SocketAddr};
use std::os::fd::FromRawFd;
use tokio::net::TcpListener;

/// First file descriptor passed by systemd socket activation.
const SD_LISTEN_FDS_START: i32 = 3;

/// The addresses of every interface whose name starts with `wg`, found live (never hardcoded).
pub fn wireguard_addresses() -> Result<Vec<IpAddr>> {
    let mut found = Vec::new();
    for interface in getifaddrs().context("listing network interfaces")? {
        if !interface.interface_name.starts_with("wg") {
            continue;
        }
        let Some(address) = interface.address else { continue };
        if let Some(v4) = address.as_sockaddr_in() {
            found.push(IpAddr::V4(v4.ip()));
        } else if let Some(v6) = address.as_sockaddr_in6() {
            let ip = v6.ip();
            if !ip.is_unicast_link_local() {
                found.push(IpAddr::V6(ip));
            }
        }
    }
    found.sort();
    found.dedup();
    Ok(found)
}

/// How the daemon obtains its listening sockets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Listener {
    /// Sockets handed over by systemd (`LISTEN_FDS`), else the fallback.
    Systemd(Box<Listener>),
    /// These exact addresses.
    Addresses(Vec<SocketAddr>),
    /// The WireGuard interface addresses on this port.
    WireGuard(u16),
}

impl Listener {
    /// Binds the sockets. Systemd sockets are used as given; the others fail loudly when nothing
    /// could be bound, so a daemon never starts listening nowhere (or everywhere).
    pub async fn bind(&self) -> Result<Vec<TcpListener>> {
        match self {
            Self::Systemd(fallback) => {
                if let Some(sockets) = from_systemd()? {
                    return Ok(sockets);
                }
                Box::pin(fallback.bind()).await
            }
            Self::Addresses(addresses) => bind_all(addresses.iter().copied()).await,
            Self::WireGuard(port) => {
                let addresses = wireguard_addresses()?;
                if addresses.is_empty() {
                    bail!("no wg* interface has an address: refusing to listen elsewhere");
                }
                bind_all(addresses.into_iter().map(|ip| SocketAddr::new(ip, *port))).await
            }
        }
    }
}

async fn bind_all(addresses: impl Iterator<Item = SocketAddr>) -> Result<Vec<TcpListener>> {
    let mut listeners = Vec::new();
    for address in addresses {
        listeners.push(
            TcpListener::bind(address)
                .await
                .with_context(|| format!("binding {address}"))?,
        );
    }
    if listeners.is_empty() {
        bail!("no address to listen on");
    }
    Ok(listeners)
}

fn from_systemd() -> Result<Option<Vec<TcpListener>>> {
    let pid_matches = std::env::var("LISTEN_PID")
        .ok()
        .and_then(|p| p.parse::<u32>().ok())
        .is_some_and(|p| p == std::process::id());
    let count: i32 = match std::env::var("LISTEN_FDS").ok().and_then(|n| n.parse().ok()) {
        Some(n) if pid_matches && n > 0 => n,
        _ => return Ok(None),
    };
    let mut sockets = Vec::new();
    for fd in SD_LISTEN_FDS_START..SD_LISTEN_FDS_START + count {
        nix::fcntl::fcntl(
            // SAFETY: systemd passed this descriptor to this process (LISTEN_PID matches) and
            // nothing else in the process owns it.
            unsafe { std::os::fd::BorrowedFd::borrow_raw(fd) },
            nix::fcntl::FcntlArg::F_SETFD(nix::fcntl::FdFlag::FD_CLOEXEC),
        )
        .context("marking the systemd socket close-on-exec")?;
        // SAFETY: as above; ownership moves into the listener exactly once.
        let std_listener = unsafe { std::net::TcpListener::from_raw_fd(fd) };
        std_listener.set_nonblocking(true)?;
        sockets.push(TcpListener::from_std(std_listener)?);
    }
    Ok(Some(sockets))
}
