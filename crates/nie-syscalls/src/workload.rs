//! Workloads: the commands traced with `strace`, read from the policy file.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow};
use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct Policy {
    #[serde(rename = "workload")]
    pub workloads: Vec<Workload>,
}

#[derive(Debug, Deserialize)]
pub struct Workload {
    pub name: String,
    /// Command line; `{nie}`, `{game}` and `{port}` are replaced by the [`Context`] and a free
    /// local port.
    pub argv: Vec<String>,
    /// For servers: the workload is stopped once this probe answers 200.
    pub ready: Option<Ready>,
    /// Longest run in seconds before the workload is killed.
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct Ready {
    /// HTTP path probed on `{port}`.
    pub path: String,
}

impl Policy {
    /// Parses the policy file.
    pub fn parse(text: &str) -> Result<Self> {
        toml::from_str(text).context("parse the syscall policy")
    }
}

fn default_timeout() -> u64 {
    120
}

/// What the placeholders of the workload command lines stand for.
pub struct Context {
    pub nie: PathBuf,
    pub game: String,
}

/// How a traced workload ended.
#[derive(Debug, Serialize)]
pub struct Outcome {
    /// Exit status of `strace` (the traced command's own status), `None` when stopped by signal.
    pub exit_code: Option<i32>,
    /// HTTP status of the readiness probe, for servers.
    pub probe_status: Option<u16>,
    pub timed_out: bool,
    pub argv: Vec<String>,
}

fn free_port() -> Result<u16> {
    Ok(TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?.local_addr()?.port())
}

fn probe(port: u16, path: &str) -> Option<u16> {
    let mut stream = TcpStream::connect_timeout(
        &SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        Duration::from_millis(300),
    )
    .ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    write!(stream, "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").ok()?;
    let mut head = [0u8; 12];
    stream.read_exact(&mut head).ok()?;
    std::str::from_utf8(&head).ok()?.get(9..12)?.parse().ok()
}

/// Runs the workload under `strace -yy -f -o <log>` and returns how it ended.
pub fn trace(workload: &Workload, ctx: &Context, log: &Path) -> Result<Outcome> {
    let port = free_port()?;
    let argv: Vec<String> = workload
        .argv
        .iter()
        .map(|arg| {
            arg.replace("{nie}", &ctx.nie.to_string_lossy())
                .replace("{game}", &ctx.game)
                .replace("{port}", &port.to_string())
        })
        .collect();
    let mut child = Command::new("strace")
        .args(["-yy", "-f", "-o"])
        .arg(log)
        .arg("--")
        .args(&argv)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .context("start strace (is it installed?)")?;
    let group = Pid::from_raw(i32::try_from(child.id())?);
    let deadline = Instant::now() + Duration::from_secs(workload.timeout_secs);
    let mut probe_status = None;
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if let Some(ready) = &workload.ready {
            probe_status = probe(port, &ready.path);
            if probe_status.is_some() {
                killpg(group, Signal::SIGTERM).ok();
                break wait_or_kill(&mut child, group)?;
            }
        }
        if Instant::now() >= deadline {
            timed_out = true;
            killpg(group, Signal::SIGTERM).ok();
            break wait_or_kill(&mut child, group)?;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    Ok(Outcome {
        exit_code: if workload.ready.is_some() { None } else { status.code() },
        probe_status,
        timed_out,
        argv,
    })
}

fn wait_or_kill(
    child: &mut std::process::Child,
    group: Pid,
) -> Result<std::process::ExitStatus> {
    let until = Instant::now() + Duration::from_secs(5);
    while Instant::now() < until {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    killpg(group, Signal::SIGKILL).ok();
    child.wait().map_err(|e| anyhow!("wait strace: {e}"))
}
