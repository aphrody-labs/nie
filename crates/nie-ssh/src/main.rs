//! `nie-ssh`: run commands, move files over SFTP and forward ports, with no `ssh` binary.

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use nie_ssh::{Connection, Forward, HostKeyPolicy, Target, home_dir, split_authority};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

#[derive(Parser)]
#[command(
    name = "nie-ssh",
    version,
    about = "SSH client in pure Rust: nie-ssh [OPTIONS] HOST COMMAND... runs a command; subcommands: exec, sftp, forward, config, keyscan",
    args_conflicts_with_subcommands = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,
    #[command(flatten)]
    run: Run,
}

#[derive(Args)]
struct Conn {
    /// Port of the server (default: ssh_config, then 22).
    #[arg(short = 'p', long)]
    port: Option<u16>,
    /// Login name (default: ssh_config, then the current user).
    #[arg(short = 'l', long = "login")]
    login: Option<String>,
    /// Identity file; repeatable.
    #[arg(short = 'i', long = "identity")]
    identity: Vec<PathBuf>,
    /// Jump host `[user@]host[:port]`; repeatable, in order.
    #[arg(short = 'J', long = "jump")]
    jump: Vec<String>,
    /// known_hosts file instead of ~/.ssh/known_hosts.
    #[arg(long)]
    known_hosts: Option<PathBuf>,
    /// Record a host key seen for the first time; a changed key is still refused.
    #[arg(long)]
    accept_new: bool,
    /// Ignore ~/.ssh/config.
    #[arg(long)]
    no_config: bool,
}

#[derive(Args)]
struct Run {
    #[command(flatten)]
    conn: Conn,
    /// `[user@]host[:port]`.
    host: Option<String>,
    /// The command line; its words are joined with spaces, as `ssh` does.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    command: Vec<String>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a command: nie-ssh exec HOST COMMAND...
    Exec(Run),
    /// SFTP: ls, cat, get, put, stat.
    Sftp {
        #[command(flatten)]
        conn: Conn,
        host: String,
        #[command(subcommand)]
        op: SftpOp,
    },
    /// Local port forward until interrupted: nie-ssh forward HOST -L [bind:]port:host:port
    Forward {
        #[command(flatten)]
        conn: Conn,
        host: String,
        #[arg(short = 'L', required = true)]
        forward: Vec<String>,
    },
    /// Print what ssh_config resolves for HOST (jump hosts included).
    Config {
        #[command(flatten)]
        conn: Conn,
        host: String,
    },
    /// Print the host key of HOST as a known_hosts line.
    Keyscan {
        #[arg(short = 'p', long, default_value_t = 22)]
        port: u16,
        host: String,
    },
}

#[derive(Subcommand)]
enum SftpOp {
    /// List a remote directory.
    Ls { path: String },
    /// Print a remote file on standard output.
    Cat { path: String },
    /// Download REMOTE to LOCAL.
    Get { remote: String, local: PathBuf },
    /// Upload LOCAL to REMOTE.
    Put { local: PathBuf, remote: String },
    /// Size and kind of a remote path.
    Stat { path: String },
}

fn target(conn: &Conn, host: &str) -> Result<Target> {
    let mut target = Target::parse(host)?;
    target.port = conn.port.or(target.port);
    target.user = conn.login.clone().or(target.user);
    target.identity_files.clone_from(&conn.identity);
    target.known_hosts.clone_from(&conn.known_hosts);
    target.jumps.clone_from(&conn.jump);
    target.use_config = !conn.no_config;
    Ok(target)
}

async fn connect(conn: &Conn, host: &str) -> Result<Connection> {
    let home = home_dir().context("HOME or USERPROFILE is not set")?;
    let policy = if conn.accept_new {
        HostKeyPolicy::AcceptNew
    } else {
        HostKeyPolicy::Strict
    };
    Ok(Connection::connect(&target(conn, host)?, &home, policy).await?)
}

async fn exec(run: Run) -> Result<i32> {
    let host = run.host.context("a host is required")?;
    if run.command.is_empty() {
        bail!("no command: nie-ssh runs commands, it opens no interactive shell");
    }
    let connection = connect(&run.conn, &host).await?;
    let mut stdout = tokio::io::stdout();
    let mut stderr = tokio::io::stderr();
    let status = connection
        .exec_with(&run.command.join(" "), tokio::io::stdin(), &mut stdout, &mut stderr)
        .await?;
    // 255 is what ssh reports when the remote command died without a status.
    Ok(status.map_or(255, |s| i32::try_from(s).unwrap_or(255)))
}

async fn sftp(conn: Conn, host: String, op: SftpOp) -> Result<i32> {
    use nie_ssh::russh_sftp::client::SftpSession;
    let connection = connect(&conn, &host).await?;
    let stream = connection.open_subsystem("sftp").await?;
    let sftp = SftpSession::new(stream).await?;
    match op {
        SftpOp::Ls { path } => {
            let mut entries: Vec<_> = sftp
                .read_dir(&path)
                .await?
                .map(|e| (e.file_name(), e.metadata().size.unwrap_or(0), e.file_type().is_dir()))
                .collect();
            entries.sort();
            for (name, size, dir) in entries {
                println!("{}{name}\t{size}", if dir { "d " } else { "- " });
            }
        }
        SftpOp::Cat { path } => {
            let bytes = sftp.read(&path).await?;
            tokio::io::stdout().write_all(&bytes).await?;
        }
        SftpOp::Get { remote, local } => {
            let bytes = sftp.read(&remote).await?;
            std::fs::write(&local, &bytes).with_context(|| local.display().to_string())?;
            println!("{} bytes -> {}", bytes.len(), local.display());
        }
        SftpOp::Put { local, remote } => {
            let bytes = std::fs::read(&local).with_context(|| local.display().to_string())?;
            // `SftpSession::write` opens an existing file; `create` is open(CREATE | TRUNCATE | WRITE).
            let mut file = sftp.create(&remote).await?;
            file.write_all(&bytes).await?;
            file.close().await?;
            println!("{} bytes -> {remote}", bytes.len());
        }
        SftpOp::Stat { path } => {
            let meta = sftp.metadata(&path).await?;
            println!(
                "{}\t{}\tmode {:o}",
                if meta.is_dir() { "dir" } else { "file" },
                meta.size.unwrap_or(0),
                meta.permissions.unwrap_or(0)
            );
        }
    }
    Ok(0)
}

async fn forward(conn: Conn, host: String, specs: Vec<String>) -> Result<i32> {
    let connection = Arc::new(connect(&conn, &host).await?);
    let mut tasks = tokio::task::JoinSet::new();
    for spec in specs {
        let forward = Forward::parse(&spec)?;
        let connection = connection.clone();
        tasks.spawn(async move {
            connection
                .forward_local(&forward, |bound| {
                    eprintln!("forwarding {bound} -> {}:{}", forward.host, forward.port);
                })
                .await
        });
    }
    tokio::select! {
        done = tasks.join_next() => {
            if let Some(Ok(Err(e))) = done {
                return Err(e.into());
            }
        }
        _ = tokio::signal::ctrl_c() => {}
    }
    Ok(0)
}

fn config(conn: &Conn, host: &str) -> Result<i32> {
    let home = home_dir().context("HOME or USERPROFILE is not set")?;
    for (i, hop) in nie_ssh::resolve_chain(&target(conn, host)?, &home)?.iter().enumerate() {
        println!("hop {i}");
        println!("  hostname {}", hop.host);
        println!("  port {}", hop.port);
        println!("  user {}", hop.user);
        println!("  connecttimeout {}", hop.timeout.as_secs());
        for identity in &hop.identities {
            println!("  identityfile {}", identity.display());
        }
    }
    Ok(0)
}

async fn keyscan(port: u16, host: &str) -> Result<i32> {
    let (_, host, parsed) = split_authority(host)?;
    let line = Connection::scan_host_key(&host, parsed.unwrap_or(port), Duration::from_secs(10)).await?;
    println!("{line}");
    Ok(0)
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let outcome = match cli.command {
        Some(Cmd::Exec(run)) => exec(run).await,
        Some(Cmd::Sftp { conn, host, op }) => sftp(conn, host, op).await,
        Some(Cmd::Forward { conn, host, forward: specs }) => forward(conn, host, specs).await,
        Some(Cmd::Config { conn, host }) => config(&conn, &host),
        Some(Cmd::Keyscan { port, host }) => keyscan(port, &host).await,
        None => exec(cli.run).await,
    };
    match outcome {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("nie-ssh: {error:#}");
            std::process::exit(255);
        }
    }
}
