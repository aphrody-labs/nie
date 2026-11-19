//! The russh server: authentication, exec, SFTP and direct TCP forwarding.

use crate::keys::AuthorizedKeys;
use crate::listen::Listener;
use crate::sftp::Sftp;
use anyhow::{Context, Result};
use russh::keys::ssh_key::PrivateKey;
use russh::server::{Auth, Config, Handler, Msg, Server, Session, VerifiedPublicKey};
use russh::{Channel, ChannelId, ChannelMsg, MethodKind, MethodSet};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

/// Everything the daemon needs to run.
pub struct Settings {
    pub host_key: PrivateKey,
    pub authorized: AuthorizedKeys,
    /// The one account served: the one the daemon runs as.
    pub user: String,
    pub home: std::path::PathBuf,
    pub shell: String,
    pub allow_forwarding: bool,
    pub listener: Listener,
}

struct Shared {
    authorized: AuthorizedKeys,
    user: String,
    home: std::path::PathBuf,
    shell: String,
    allow_forwarding: bool,
}

#[derive(Clone)]
struct Daemon(Arc<Shared>);

impl Server for Daemon {
    type Handler = Session1;

    fn new_client(&mut self, peer: Option<SocketAddr>) -> Session1 {
        Session1 {
            shared: self.0.clone(),
            peer,
            channels: HashMap::new(),
        }
    }
}

struct Session1 {
    shared: Arc<Shared>,
    peer: Option<SocketAddr>,
    channels: HashMap<ChannelId, Channel<Msg>>,
}

fn log(peer: Option<SocketAddr>, message: &str) {
    match peer {
        Some(peer) => eprintln!("nie-sshd: {peer}: {message}"),
        None => eprintln!("nie-sshd: {message}"),
    }
}

impl Handler for Session1 {
    type Error = russh::Error;

    async fn auth_publickey(
        &mut self,
        user: &str,
        key: VerifiedPublicKey<'_>,
    ) -> Result<Auth, Self::Error> {
        let allowed = user == self.shared.user && self.shared.authorized.allows(key.public_key);
        log(
            self.peer,
            &format!("publickey for {user}: {}", if allowed { "accepted" } else { "refused" }),
        );
        Ok(if allowed { Auth::Accept } else { Auth::reject() })
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: russh::server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host: &str,
        port: u32,
        _originator: &str,
        _originator_port: u32,
        reply: russh::server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        let target = u16::try_from(port).ok().map(|p| (host.to_owned(), p));
        let (true, Some(target)) = (self.shared.allow_forwarding, target) else {
            reply.reject(russh::ChannelOpenFailure::AdministrativelyProhibited).await;
            return Ok(());
        };
        let peer = self.peer;
        match tokio::time::timeout(Duration::from_secs(10), tokio::net::TcpStream::connect(&target)).await {
            Ok(Ok(mut remote)) => {
                reply.accept().await;
                log(peer, &format!("forward to {}:{}", target.0, target.1));
                tokio::spawn(async move {
                    let mut stream = channel.into_stream();
                    let _ = tokio::io::copy_bidirectional(&mut stream, &mut remote).await;
                });
            }
            _ => {
                reply.reject(russh::ChannelOpenFailure::ConnectFailed).await;
            }
        }
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel_id: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let Some(channel) = self.channels.remove(&channel_id) else {
            session.channel_failure(channel_id)?;
            return Ok(());
        };
        let command = String::from_utf8_lossy(data).into_owned();
        log(self.peer, &format!("exec {command:?}"));
        session.channel_success(channel_id)?;
        let shared = self.shared.clone();
        tokio::spawn(async move { run_command(channel, &shared, &command).await });
        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        channel_id: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if name == "sftp"
            && let Some(channel) = self.channels.remove(&channel_id)
        {
            session.channel_success(channel_id)?;
            log(self.peer, "sftp");
            let handler = Sftp::new(self.shared.home.clone());
            tokio::spawn(async move {
                russh_sftp::server::run(channel.into_stream(), handler).await;
            });
        } else {
            session.channel_failure(channel_id)?;
        }
        Ok(())
    }

    async fn pty_request(
        &mut self,
        channel_id: ChannelId,
        _term: &str,
        _col_width: u32,
        _row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_failure(channel_id)?;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel_id: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_failure(channel_id)?;
        Ok(())
    }

    async fn env_request(
        &mut self,
        channel_id: ChannelId,
        _name: &str,
        _value: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_failure(channel_id)?;
        Ok(())
    }

    async fn tcpip_forward(
        &mut self,
        _address: &str,
        _port: &mut u32,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        Ok(false)
    }
}

/// Copies everything `reader` yields into `writer` until either side ends.
fn pump<R>(
    reader: Option<R>,
    mut writer: Box<dyn tokio::io::AsyncWrite + Unpin + Send>,
) -> tokio::task::JoinHandle<()>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let Some(mut reader) = reader else { return };
        let mut buf = vec![0u8; 32 * 1024];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if writer.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
            }
        }
    })
}

/// Runs `command` through the account's shell and relays it over `channel`.
async fn run_command(mut channel: Channel<Msg>, shared: &Shared, command: &str) {
    let spawned = Command::new(&shared.shell)
        .arg("-c")
        .arg(command)
        .current_dir(&shared.home)
        .env("HOME", &shared.home)
        .env("USER", &shared.user)
        .env("LOGNAME", &shared.user)
        .env("SHELL", &shared.shell)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(e) => {
            let _ = channel
                .make_writer_ext(Some(1))
                .write_all(format!("nie-sshd: {}: {e}\n", shared.shell).as_bytes())
                .await;
            let _ = channel.exit_status(127).await;
            let _ = channel.eof().await;
            let _ = channel.close().await;
            return;
        }
    };
    let mut stdin = child.stdin.take();
    let (mut out, mut err) = (child.stdout.take(), child.stderr.take());
    let to_client = channel.make_writer();
    let to_client_err = channel.make_writer_ext(Some(1));
    let out_task = pump(out.take(), Box::new(to_client));
    let err_task = pump(err.take(), Box::new(to_client_err));
    let status = loop {
        tokio::select! {
            message = channel.wait() => match message {
                Some(ChannelMsg::Data { data }) => {
                    if let Some(input) = stdin.as_mut()
                        && input.write_all(&data).await.is_err()
                    {
                        stdin = None;
                    }
                }
                Some(ChannelMsg::Eof) => stdin = None,
                Some(ChannelMsg::Close) | None => {
                    let _ = child.kill().await;
                    return;
                }
                Some(_) => {}
            },
            status = child.wait() => break status,
        }
    };
    let _ = out_task.await;
    let _ = err_task.await;
    let code = match status {
        Ok(status) => status.code().map_or(255, |c| u32::try_from(c).unwrap_or(255)),
        Err(_) => 255,
    };
    let _ = channel.exit_status(code).await;
    let _ = channel.eof().await;
    let _ = channel.close().await;
}

/// Serves until the process is told to stop (SIGINT or SIGTERM).
pub async fn serve(settings: Settings) -> Result<()> {
    let listeners = settings.listener.bind().await?;
    let config = Arc::new(Config {
        methods: MethodSet::from(&[MethodKind::PublicKey][..]),
        keys: vec![settings.host_key],
        auth_rejection_time: Duration::from_secs(1),
        max_auth_attempts: 4,
        keepalive_interval: Some(Duration::from_secs(30)),
        keepalive_max: 4,
        inactivity_timeout: Some(Duration::from_secs(3600)),
        nodelay: true,
        ..Config::default()
    });
    let daemon = Daemon(Arc::new(Shared {
        authorized: settings.authorized,
        user: settings.user,
        home: settings.home,
        shell: settings.shell,
        allow_forwarding: settings.allow_forwarding,
    }));
    let mut tasks = tokio::task::JoinSet::new();
    for listener in listeners {
        let address = listener.local_addr().context("listener address")?;
        eprintln!("nie-sshd: listening on {address}");
        let (config, mut daemon) = (config.clone(), daemon.clone());
        tasks.spawn(async move { daemon.run_on_socket(config, &listener).await });
    }
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! {
        finished = tasks.join_next() => {
            if let Some(Ok(Err(e))) = finished {
                return Err(e).context("server stopped");
            }
        }
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }
    Ok(())
}
