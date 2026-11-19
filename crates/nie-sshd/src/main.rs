//! `nie-sshd`: the SSH server of the nie stack.

#[cfg(unix)]
mod app {
    use anyhow::{Context, Result, bail};
    use clap::{Parser, Subcommand};
    use nie_sshd::{AuthorizedKeys, Listener, Settings, host_key_or_create, serve, wireguard_addresses};
    use std::net::SocketAddr;
    use std::path::PathBuf;

    #[derive(Parser)]
    #[command(name = "nie-sshd", version, about = "SSH server in pure Rust: public keys, exec, SFTP, forwarding")]
    struct Cli {
        #[command(subcommand)]
        command: Cmd,
    }

    #[derive(Subcommand)]
    enum Cmd {
        /// Serve until SIGINT or SIGTERM.
        Run {
            /// State directory holding host_ed25519 and authorized_keys.
            #[arg(long, env = "NIE_SSHD_STATE", default_value = "/var/lib/nie/sshd")]
            state: PathBuf,
            /// Address to listen on; repeatable. Without it the wg* interface addresses are used.
            #[arg(long)]
            listen: Vec<SocketAddr>,
            /// Port used with the wg* interface addresses.
            #[arg(long, default_value_t = 2222)]
            port: u16,
            /// Use sockets passed by systemd (LISTEN_FDS) when there are any.
            #[arg(long)]
            systemd: bool,
            /// Refuse direct-tcpip channels.
            #[arg(long)]
            no_forwarding: bool,
        },
        /// Print the WireGuard addresses the daemon would listen on.
        Addresses,
        /// Print the host public key (creating the key when missing).
        HostKey {
            #[arg(long, env = "NIE_SSHD_STATE", default_value = "/var/lib/nie/sshd")]
            state: PathBuf,
        },
        /// Print the systemd units (socket on the WireGuard addresses found now, and service).
        Units {
            #[arg(long, env = "NIE_SSHD_STATE", default_value = "/var/lib/nie/sshd")]
            state: PathBuf,
            /// Port of the socket.
            #[arg(long, default_value_t = 2222)]
            port: u16,
            /// Path of the nie-sshd binary the service starts (default: this executable).
            #[arg(long)]
            exe: Option<PathBuf>,
        },
        /// Merge the plain keys of an OpenSSH authorized_keys file into the daemon's file.
        ImportKeys {
            from: PathBuf,
            #[arg(long, env = "NIE_SSHD_STATE", default_value = "/var/lib/nie/sshd")]
            state: PathBuf,
        },
    }

    fn account() -> Result<(String, PathBuf, String)> {
        let user = nix::unistd::User::from_uid(nix::unistd::getuid())
            .context("reading the passwd entry")?
            .context("the current uid has no passwd entry")?;
        Ok((
            user.name,
            user.dir,
            user.shell.to_string_lossy().into_owned(),
        ))
    }

    #[tokio::main]
    pub async fn main() -> Result<()> {
        match Cli::parse().command {
            Cmd::Addresses => {
                for ip in wireguard_addresses()? {
                    println!("{ip}");
                }
            }
            Cmd::Units { state, port, exe } => {
                let addresses = wireguard_addresses()?;
                if addresses.is_empty() {
                    bail!("no wg* interface has an address: nothing to put in the socket unit");
                }
                let (user, _, _) = account()?;
                let exe = match exe {
                    Some(exe) => exe,
                    None => std::env::current_exe()?,
                };
                println!("# nie-sshd.socket");
                println!("[Unit]\nDescription=nie-sshd socket (WireGuard addresses only)\n");
                println!("[Socket]");
                for ip in &addresses {
                    println!("ListenStream={}", SocketAddr::new(*ip, port));
                }
                println!("FreeBind=yes\n\n[Install]\nWantedBy=sockets.target\n");
                println!("# nie-sshd.service");
                println!("[Unit]\nDescription=nie-sshd SSH server (public keys, exec, SFTP, forwarding)");
                println!("Requires=nie-sshd.socket\nAfter=network.target nie-sshd.socket\n");
                println!("[Service]\nUser={user}\nStateDirectory=nie/sshd\nStateDirectoryMode=0700");
                println!("ExecStart={} run --systemd --state {}", exe.display(), state.display());
                println!("Restart=on-failure\nRestartSec=2\n");
                println!("[Install]\nWantedBy=multi-user.target");
            }
            Cmd::HostKey { state } => {
                let key = host_key_or_create(&state.join("host_ed25519"))?;
                println!("{}", key.public_key().to_openssh()?);
            }
            Cmd::ImportKeys { from, state } => {
                let text = std::fs::read_to_string(&from).with_context(|| from.display().to_string())?;
                let incoming = AuthorizedKeys::parse(&text);
                if incoming.is_empty() {
                    bail!("{} holds no plain public key", from.display());
                }
                std::fs::create_dir_all(&state)?;
                let target = state.join("authorized_keys");
                let existing = std::fs::read_to_string(&target).unwrap_or_default();
                let mut lines: Vec<String> = existing.lines().map(str::to_owned).collect();
                let mut added = 0;
                for line in text.lines().map(str::trim) {
                    let plain = AuthorizedKeys::parse(line);
                    if plain.len() == 1 && !lines.iter().any(|l| l.trim() == line) {
                        lines.push(line.to_owned());
                        added += 1;
                    }
                }
                std::fs::write(&target, lines.join("\n") + "\n")?;
                println!(
                    "{} keys added, {} lines with options skipped, {} total in {}",
                    added,
                    incoming.skipped,
                    lines.iter().filter(|l| AuthorizedKeys::parse(l).len() == 1).count(),
                    target.display()
                );
            }
            Cmd::Run { state, listen, port, systemd, no_forwarding } => {
                let (user, home, shell) = account()?;
                let authorized = AuthorizedKeys::load(&state.join("authorized_keys"))?;
                let fresh = if listen.is_empty() {
                    Listener::WireGuard(port)
                } else {
                    Listener::Addresses(listen)
                };
                let listener = if systemd { Listener::Systemd(Box::new(fresh)) } else { fresh };
                serve(Settings {
                    host_key: host_key_or_create(&state.join("host_ed25519"))?,
                    authorized,
                    user,
                    home,
                    shell,
                    allow_forwarding: !no_forwarding,
                    listener,
                })
                .await?;
            }
        }
        Ok(())
    }
}

#[cfg(unix)]
fn main() -> anyhow::Result<()> {
    app::main()
}

#[cfg(not(unix))]
fn main() {
    eprintln!("nie-sshd runs on Unix hosts only");
    std::process::exit(1);
}
