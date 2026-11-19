//! Handshake, host-key checks, authentication and jump chains.

use crate::config::{Hop, Target, default_known_hosts, resolve_chain};
use crate::{Error, Result};
use russh::Preferred;
use russh::client::{self, Handle};
use russh::keys::agent::AgentIdentity;
use russh::keys::agent::client::{AgentClient, AgentStream};
use russh::keys::known_hosts::{
    check_known_hosts_path, known_host_keys_path, learn_known_hosts_path,
};
use russh::keys::{
    Algorithm, Error as KeyError, HashAlg, PrivateKeyWithHashAlg, PublicKeyOrCertificate,
    load_secret_key,
};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::time::timeout;

/// What to do with a server key that `known_hosts` does not know.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HostKeyPolicy {
    /// Refuse, with the `ssh-keyscan` hint (OpenSSH `StrictHostKeyChecking=yes`).
    #[default]
    Strict,
    /// Record a new host and refuse a changed one (`StrictHostKeyChecking=accept-new`).
    AcceptNew,
}

/// Verifies the server key against `known_hosts`; the reason of a refusal is kept for the error.
pub(crate) struct Client {
    host: String,
    port: u16,
    known_hosts: Vec<PathBuf>,
    policy: HostKeyPolicy,
    refusal: Arc<Mutex<Option<String>>>,
}

impl Client {
    fn refuse(&self, reason: String) -> std::result::Result<bool, russh::Error> {
        if let Ok(mut slot) = self.refusal.lock() {
            *slot = Some(reason);
        }
        Ok(false)
    }
}

impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKeyOrCertificate,
    ) -> std::result::Result<bool, Self::Error> {
        let key = server_public_key.public_key();
        for path in self.known_hosts.iter().filter(|p| p.is_file()) {
            match check_known_hosts_path(&self.host, self.port, &key, path) {
                Ok(true) => return Ok(true),
                Ok(false) => {}
                Err(KeyError::KeyChanged { line }) => {
                    return self.refuse(format!(
                        "host key of {} changed ({}:{line}); possible man-in-the-middle, refusing",
                        self.host,
                        path.display()
                    ));
                }
                Err(e) => return self.refuse(format!("{}: {e}", path.display())),
            }
        }
        if self.policy == HostKeyPolicy::AcceptNew
            && let Some(path) = self.known_hosts.first()
        {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            return match learn_known_hosts_path(&self.host, self.port, &key, path) {
                Ok(()) => Ok(true),
                Err(e) => self.refuse(format!("{}: {e}", path.display())),
            };
        }
        let files: Vec<String> = self
            .known_hosts
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        self.refuse(format!(
            "host key of {}:{} is not in known_hosts ({}); add it with `nie-ssh keyscan -p {} {}` or connect with --accept-new",
            self.host,
            self.port,
            files.join(", "),
            self.port,
            self.host
        ))
    }
}

/// Offer the algorithms already recorded in `known_hosts` first, as OpenSSH does.
fn client_config(host: &str, port: u16, known_hosts: &[PathBuf]) -> client::Config {
    let mut preferred = Preferred::default();
    let mut algorithms: Vec<Algorithm> = Vec::new();
    for path in known_hosts {
        for (_, key) in known_host_keys_path(host, port, path).unwrap_or_default() {
            match key.algorithm() {
                Algorithm::Rsa { .. } => {
                    for hash in [HashAlg::Sha512, HashAlg::Sha256] {
                        algorithms.push(Algorithm::Rsa { hash: Some(hash) });
                    }
                }
                other => algorithms.push(other),
            }
        }
    }
    if !algorithms.is_empty() {
        for default in preferred.key.iter() {
            if !algorithms.contains(default) {
                algorithms.push(default.clone());
            }
        }
        preferred.key = Cow::Owned(algorithms);
    }
    client::Config {
        inactivity_timeout: None,
        keepalive_interval: Some(Duration::from_secs(30)),
        keepalive_max: 4,
        preferred,
        ..client::Config::default()
    }
}

#[cfg(unix)]
async fn connect_agent() -> Option<AgentClient<Box<dyn AgentStream + Send + Unpin>>> {
    AgentClient::connect_env()
        .await
        .ok()
        .map(AgentClient::dynamic)
}

#[cfg(windows)]
async fn connect_agent() -> Option<AgentClient<Box<dyn AgentStream + Send + Unpin>>> {
    let pipe = std::env::var("SSH_AUTH_SOCK")
        .ok()
        .filter(|p| p.starts_with(r"\\.\pipe\"))
        .unwrap_or_else(|| r"\\.\pipe\openssh-ssh-agent".to_owned());
    AgentClient::connect_named_pipe(pipe)
        .await
        .ok()
        .map(AgentClient::dynamic)
}

#[cfg(not(any(unix, windows)))]
async fn connect_agent() -> Option<AgentClient<Box<dyn AgentStream + Send + Unpin>>> {
    None
}

async fn rsa_hash(handle: &Handle<Client>, algorithm: &Algorithm) -> Option<HashAlg> {
    if matches!(algorithm, Algorithm::Rsa { .. }) {
        handle
            .best_supported_rsa_hash()
            .await
            .ok()
            .flatten()
            .flatten()
    } else {
        None
    }
}

/// ssh-agent identities first, then the identity files, like OpenSSH.
async fn authenticate(handle: &mut Handle<Client>, hop: &Hop) -> Result<()> {
    let mut notes: Vec<String> = Vec::new();
    if let Some(mut agent) = connect_agent().await {
        match agent.request_identities().await {
            Ok(identities) => {
                for identity in identities {
                    let AgentIdentity::PublicKey { key, .. } = identity else {
                        continue;
                    };
                    let hash = rsa_hash(handle, &key.algorithm()).await;
                    match handle
                        .authenticate_publickey_with(hop.user.as_str(), key, hash, &mut agent)
                        .await
                    {
                        Ok(result) if result.success() => return Ok(()),
                        Ok(_) => {}
                        Err(e) => notes.push(format!("ssh-agent: {e}")),
                    }
                }
            }
            Err(e) => notes.push(format!("ssh-agent: {e}")),
        }
    }
    for path in &hop.identities {
        let key = match load_secret_key(path, None) {
            Ok(key) => key,
            Err(KeyError::KeyIsEncrypted) => {
                notes.push(format!(
                    "{} is passphrase protected: load it into ssh-agent",
                    path.display()
                ));
                continue;
            }
            Err(e) => {
                notes.push(format!("{}: {e}", path.display()));
                continue;
            }
        };
        let hash = rsa_hash(handle, &key.algorithm()).await;
        match handle
            .authenticate_publickey(
                hop.user.as_str(),
                PrivateKeyWithHashAlg::new(Arc::new(key), hash),
            )
            .await
        {
            Ok(result) if result.success() => return Ok(()),
            Ok(_) => notes.push(format!("{} was refused", path.display())),
            Err(e) => notes.push(format!("{}: {e}", path.display())),
        }
    }
    let detail = if notes.is_empty() {
        "no ssh-agent identity or ~/.ssh key available".to_owned()
    } else {
        notes.join("; ")
    };
    Err(Error::new(format!(
        "ssh authentication to {}@{} failed: {detail}",
        hop.user, hop.host
    )))
}

async fn open_session<S>(
    hop: &Hop,
    known_hosts: &[PathBuf],
    policy: HostKeyPolicy,
    stream: S,
) -> Result<Handle<Client>>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let refusal = Arc::new(Mutex::new(None));
    let handler = Client {
        host: hop.host.clone(),
        port: hop.port,
        known_hosts: known_hosts.to_vec(),
        policy,
        refusal: refusal.clone(),
    };
    let config = Arc::new(client_config(&hop.host, hop.port, known_hosts));
    let mut handle =
        match timeout(hop.timeout, client::connect_stream(config, stream, handler)).await {
            Ok(Ok(handle)) => handle,
            Ok(Err(e)) => {
                let reason = refusal.lock().ok().and_then(|mut slot| slot.take());
                return Err(Error::new(reason.unwrap_or_else(|| {
                    format!("ssh handshake with {}:{} failed: {e}", hop.host, hop.port)
                })));
            }
            Err(_) => {
                return Err(Error::new(format!(
                    "ssh handshake with {}:{} timed out",
                    hop.host, hop.port
                )));
            }
        };
    authenticate(&mut handle, hop).await?;
    Ok(handle)
}

/// Connects the chain; every jump session must stay alive as long as the target.
async fn dial(
    chain: &[Hop],
    known_hosts: &[PathBuf],
    policy: HostKeyPolicy,
) -> Result<Vec<Handle<Client>>> {
    let mut sessions: Vec<Handle<Client>> = Vec::with_capacity(chain.len());
    for hop in chain {
        let handle = if let Some(previous) = sessions.last() {
            let channel = timeout(
                hop.timeout,
                previous.channel_open_direct_tcpip(
                    hop.host.clone(),
                    u32::from(hop.port),
                    "127.0.0.1",
                    0,
                ),
            )
            .await
            .map_err(|_| Error::new(format!("ssh jump to {}:{} timed out", hop.host, hop.port)))?
            .map_err(|e| Error::new(format!("ssh jump to {}:{}: {e}", hop.host, hop.port)))?;
            open_session(hop, known_hosts, policy, channel.into_stream()).await?
        } else {
            let tcp = timeout(
                hop.timeout,
                TcpStream::connect((hop.host.as_str(), hop.port)),
            )
            .await
            .map_err(|_| Error::new(format!("connecting to {}:{} timed out", hop.host, hop.port)))?
            .map_err(|e| Error::new(format!("connecting to {}:{}: {e}", hop.host, hop.port)))?;
            let _ = tcp.set_nodelay(true);
            open_session(hop, known_hosts, policy, tcp).await?
        };
        sessions.push(handle);
    }
    Ok(sessions)
}

/// A live SSH connection to one target, through its jump hosts.
pub struct Connection {
    /// Jump hosts first, the target session last; kept so the tunnels stay open.
    sessions: Vec<Handle<Client>>,
    chain: Vec<Hop>,
}

impl Connection {
    /// Resolves `target` through `~/.ssh/config` under `home`, connects and authenticates.
    pub async fn connect(target: &Target, home: &Path, policy: HostKeyPolicy) -> Result<Self> {
        let chain = resolve_chain(target, home)?;
        let known_hosts = default_known_hosts(target, home);
        let sessions = dial(&chain, &known_hosts, policy).await?;
        Ok(Self { sessions, chain })
    }

    /// The target hop after `ssh_config` was applied.
    #[must_use]
    pub fn hop(&self) -> &Hop {
        self.chain.last().expect("a chain always holds its target")
    }

    /// The russh handle of the target session.
    pub(crate) fn handle(&self) -> &Handle<Client> {
        self.sessions
            .last()
            .expect("a connection always holds its target session")
    }

    /// True once the transport of the target session closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.handle().is_closed()
    }

    /// Opens a session channel and starts the named subsystem (`sftp`) on it.
    pub async fn open_subsystem(
        &self,
        name: &str,
    ) -> Result<russh::ChannelStream<client::Msg>> {
        let channel = self
            .handle()
            .channel_open_session()
            .await
            .map_err(|e| Error::new(format!("ssh session: {e}")))?;
        channel
            .request_subsystem(true, name)
            .await
            .map_err(|e| Error::new(format!("{name} subsystem refused: {e}")))?;
        Ok(channel.into_stream())
    }

    /// Fetches the server host key without verifying it, for `known_hosts` seeding.
    pub async fn scan_host_key(host: &str, port: u16, wait: Duration) -> Result<String> {
        struct Scan(Arc<Mutex<Option<String>>>);
        impl client::Handler for Scan {
            type Error = russh::Error;
            async fn check_server_key(
                &mut self,
                key: &PublicKeyOrCertificate,
            ) -> std::result::Result<bool, Self::Error> {
                if let Ok(mut slot) = self.0.lock() {
                    *slot = key.public_key().to_openssh().ok();
                }
                Ok(false)
            }
        }
        let seen = Arc::new(Mutex::new(None));
        let tcp = timeout(wait, TcpStream::connect((host, port)))
            .await
            .map_err(|_| Error::new(format!("connecting to {host}:{port} timed out")))?
            .map_err(|e| Error::new(format!("connecting to {host}:{port}: {e}")))?;
        let config = Arc::new(client::Config::default());
        // The handshake is refused on purpose after the key was read.
        let _ = timeout(wait, client::connect_stream(config, tcp, Scan(seen.clone()))).await;
        let key = seen.lock().ok().and_then(|mut slot| slot.take());
        let key = key.ok_or_else(|| Error::new(format!("no host key from {host}:{port}")))?;
        let name = if port == 22 {
            host.to_owned()
        } else {
            format!("[{host}]:{port}")
        };
        Ok(format!("{name} {key}"))
    }
}
