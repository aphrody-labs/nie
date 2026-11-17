//! Bounded remote VFS transport supporting HTTP Range and the Go `rclone` core
//! (`rclone://`, `r2://`, `s3://`, `gdrive://`, `nievfs://`, `https://`, `ssh://`, `sftp://`).

use crate::FormatError;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc,
};
use std::time::Duration;

const MAX_READ: usize = 256 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(30);
type Result<T> = std::result::Result<T, String>;

enum Request {
    Read {
        path: String,
        range: Option<(u64, usize)>,
        reply: mpsc::Sender<Result<Vec<u8>>>,
    },
    List {
        path: String,
        reply: mpsc::Sender<Result<Vec<String>>>,
    },
}

// ---------------------------------------------------------------------------
// Go rclone-core discovery (native/go/rclone-core/build/nie-rclone)
// ---------------------------------------------------------------------------

fn find_nie_rclone_bin() -> Option<PathBuf> {
    let bin_name = if cfg!(windows) {
        "nie-rclone.exe"
    } else {
        "nie-rclone"
    };
    if let Some(explicit) = std::env::var_os("NIE_RCLONE_BIN")
        && !explicit.is_empty()
    {
        let p = PathBuf::from(explicit);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let sibling = dir.join(bin_name);
        if sibling.is_file() {
            return Some(sibling);
        }
    }
    for base in [
        std::env::current_dir().ok(),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR"))),
    ]
    .into_iter()
    .flatten()
    {
        for ancestor in base.ancestors().take(6) {
            let candidate = ancestor
                .join("native")
                .join("go")
                .join("rclone-core")
                .join("build")
                .join(bin_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    if Command::new(bin_name)
        .arg("version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
    {
        return Some(PathBuf::from(bin_name));
    }
    None
}

fn run_nie_rclone(args: &[&str]) -> std::result::Result<String, FormatError> {
    let bin = find_nie_rclone_bin().ok_or(FormatError::Corrupt(
        "nie-rclone binary not built; run bun native/go/rclone-core/build.ts",
    ))?;
    let output = Command::new(&bin)
        .args(args)
        .output()
        .map_err(|e| FormatError::Malformed(e.to_string()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(FormatError::Malformed(if stderr.is_empty() {
            format!("nie-rclone exited with {}", output.status)
        } else {
            stderr
        }));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Returns true if the native Go `rclone` core (`nie-rclone`) is available.
#[must_use]
pub fn rclone_core_available() -> bool {
    find_nie_rclone_bin().is_some()
}

/// Lists configured rclone remotes via the Go core, returning JSON.
pub fn rclone_list_remotes() -> std::result::Result<String, FormatError> {
    run_nie_rclone(&["nie-remotes"])
}

/// Resolves a cloud URI (`rclone://`, `r2://`, `s3://`, `gdrive://`, `nievfs://`, etc.) via the Go core.
pub fn rclone_resolve_spec(uri: &str) -> std::result::Result<String, FormatError> {
    run_nie_rclone(&["nie-resolve", uri])
}

/// Options for parallel VFS pack export and continuous cloud sync.
#[derive(Clone, Debug)]
pub struct RcloneExportOptions<'a> {
    pub local_data_dir: &'a str,
    pub dst_uri: &'a str,
    pub mode: &'a str,
    pub dry_run: bool,
    pub transfers: usize,
    pub checkers: usize,
    pub zstd_level: i32,
    pub max_packs: usize,
    pub filter: Option<&'a str>,
    pub watch_secs: Option<u64>,
}

/// Converts and/or exports local IEVR `data/packs` to R2 / S3 / Google Drive / rclone remote.
pub fn rclone_export_packs(
    local_data_dir: &str,
    dst_uri: &str,
    mode: &str,
    dry_run: bool,
) -> std::result::Result<String, FormatError> {
    rclone_export_packs_with_opts(&RcloneExportOptions {
        local_data_dir,
        dst_uri,
        mode,
        dry_run,
        transfers: 32,
        checkers: 64,
        zstd_level: 4,
        max_packs: 0,
        filter: None,
        watch_secs: None,
    })
}

/// Exports or continuously watches and syncs local IEVR `data/packs` using parallel Go transfers.
pub fn rclone_export_packs_with_opts(
    opts: &RcloneExportOptions<'_>,
) -> std::result::Result<String, FormatError> {
    let subcmd = if opts.watch_secs.is_some() {
        "nie-watch"
    } else {
        "nie-export"
    };
    let transfers_s = opts.transfers.max(1).to_string();
    let checkers_s = opts.checkers.max(1).to_string();
    let zstd_s = opts.zstd_level.clamp(1, 22).to_string();
    let max_packs_s = opts.max_packs.to_string();
    let interval_s = opts.watch_secs.unwrap_or(15).max(1).to_string();
    let mut args = vec![
        subcmd,
        "--source",
        opts.local_data_dir,
        "--dest",
        opts.dst_uri,
        "--mode",
        opts.mode,
        "--transfers",
        &transfers_s,
        "--checkers",
        &checkers_s,
        "--zstd-level",
        &zstd_s,
    ];
    if opts.max_packs > 0 {
        args.push("--max-packs");
        args.push(&max_packs_s);
    }
    if let Some(f) = opts.filter
        && !f.trim().is_empty()
    {
        args.push("--filter");
        args.push(f.trim());
    }
    if opts.watch_secs.is_some() {
        args.push("--interval");
        args.push(&interval_s);
    }
    if opts.dry_run {
        args.push("--dry-run");
    }
    run_nie_rclone(&args)
}

// ---------------------------------------------------------------------------
// RemoteStorage & Worker
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub(crate) struct RemoteStorage {
    worker: Arc<Worker>,
    bytes: Arc<AtomicU64>,
    calls: Arc<AtomicU64>,
}

struct Worker {
    sender: Option<mpsc::Sender<Request>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn validate_uri(source: &str) -> std::result::Result<(), FormatError> {
    let (_, rest) = source
        .split_once("://")
        .ok_or(FormatError::Corrupt("invalid game source URI"))?;
    let authority = rest.split('/').next().unwrap_or_default();
    let is_ssh = source.starts_with("ssh://") || source.starts_with("sftp://");
    if authority.is_empty()
        || authority.contains(['%', '\\'])
        || source.chars().any(char::is_control)
        || source.contains(['?', '#'])
        || authority
            .split_once('@')
            .is_some_and(|(user, _)| user.contains(':'))
        || (!is_ssh && authority.contains('@'))
    {
        return Err(FormatError::Corrupt(
            "URI credentials/query/fragment are unsupported; use SSH config, rclone.conf, env vars, or NIE_HTTP_AUTH_TOKEN",
        ));
    }
    Ok(())
}

impl RemoteStorage {
    pub fn connect(source: &str) -> std::result::Result<Self, FormatError> {
        validate_uri(source)?;
        let normalized = source.trim_end_matches("/nie.exe").to_owned();
        let (sender, receiver) = mpsc::channel();
        let (ready, start) = mpsc::channel();
        let bytes = Arc::new(AtomicU64::new(0));
        let calls = Arc::new(AtomicU64::new(0));
        let worker_bytes = bytes.clone();
        let worker_calls = calls.clone();
        let thread = std::thread::Builder::new()
            .name("nie-vfs-remote".into())
            .spawn(move || {
                let mut backend = match Backend::connect(&normalized) {
                    Ok(backend) => {
                        let _ = ready.send(Ok(()));
                        backend
                    }
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                };
                while let Ok(request) = receiver.recv() {
                    worker_calls.fetch_add(1, Ordering::Relaxed);
                    match request {
                        Request::Read { path, range, reply } => {
                            let result = backend.read(&path, range);
                            if let Ok(data) = &result {
                                worker_bytes.fetch_add(data.len() as u64, Ordering::Relaxed);
                            }
                            let _ = reply.send(result);
                        }
                        Request::List { path, reply } => {
                            let _ = reply.send(backend.list(&path));
                        }
                    }
                }
            })
            .map_err(|e| FormatError::Malformed(e.to_string()))?;
        let worker = Arc::new(Worker {
            sender: Some(sender),
            thread: Some(thread),
        });
        start
            .recv_timeout(TIMEOUT)
            .map_err(|_| FormatError::Corrupt("remote VFS connection timed out"))?
            .map_err(FormatError::Malformed)?;
        Ok(Self {
            worker,
            bytes,
            calls,
        })
    }

    pub fn read(&self, path: &str) -> std::result::Result<Vec<u8>, FormatError> {
        self.read_inner(path, None)
    }

    pub fn range(
        &self,
        path: &str,
        offset: u64,
        length: usize,
    ) -> std::result::Result<Vec<u8>, FormatError> {
        if length == 0 {
            return Ok(Vec::new());
        }
        if length > MAX_READ || offset.checked_add(length as u64).is_none() {
            return Err(FormatError::Corrupt(
                "remote read exceeds 256 MiB or offset space",
            ));
        }
        self.read_inner(path, Some((offset, length)))
    }

    fn read_inner(
        &self,
        path: &str,
        range: Option<(u64, usize)>,
    ) -> std::result::Result<Vec<u8>, FormatError> {
        validate_path(path)?;
        let (reply, result) = mpsc::channel();
        self.worker
            .sender
            .as_ref()
            .ok_or(FormatError::Corrupt("remote VFS worker stopped"))?
            .send(Request::Read {
                path: path.into(),
                range,
                reply,
            })
            .map_err(|_| FormatError::Corrupt("remote VFS worker stopped"))?;
        result
            .recv_timeout(TIMEOUT + Duration::from_secs(1))
            .map_err(|_| FormatError::Corrupt("remote VFS read timed out"))?
            .map_err(FormatError::Malformed)
    }

    pub fn list(&self, path: &str) -> std::result::Result<Vec<String>, FormatError> {
        validate_path(path)?;
        let (reply, result) = mpsc::channel();
        self.worker
            .sender
            .as_ref()
            .ok_or(FormatError::Corrupt("remote VFS worker stopped"))?
            .send(Request::List {
                path: path.into(),
                reply,
            })
            .map_err(|_| FormatError::Corrupt("remote VFS worker stopped"))?;
        result
            .recv_timeout(TIMEOUT + Duration::from_secs(1))
            .map_err(|_| FormatError::Corrupt("remote directory read timed out"))?
            .map_err(FormatError::Malformed)
    }

    pub fn stats(&self) -> (u64, u64) {
        (
            self.calls.load(Ordering::Relaxed),
            self.bytes.load(Ordering::Relaxed),
        )
    }
}

fn validate_path(path: &str) -> std::result::Result<(), FormatError> {
    if path.starts_with('/')
        || path.contains(['\\', '\0'])
        || path.split('/').any(|s| s == ".." || s == ".")
    {
        return Err(FormatError::Corrupt(
            "remote resource must be a relative path without traversal",
        ));
    }
    Ok(())
}

struct RpcWorker {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    uri: String,
}

impl Drop for RpcWorker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

enum Backend {
    Http {
        host: String,
        port: u16,
        base_path: String,
        token: Option<String>,
    },
    RcloneGo(RpcWorker),
    RcloneCli {
        remote_spec: String,
    },
    /// `ssh://[user@]host[:port]/root`: plain `ssh` commands, host keys and credentials come
    /// from the user's SSH configuration (aliases, agents, jump hosts), never from the URI.
    Ssh {
        target: String,
        port: Option<u16>,
        root: String,
    },
}

/// Single-quotes `value` for a POSIX shell.
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn parse_ssh_uri(source: &str) -> Result<(String, Option<u16>, String)> {
    let rest = source
        .strip_prefix("ssh://")
        .or_else(|| source.strip_prefix("sftp://"))
        .ok_or_else(|| "not an ssh URI".to_owned())?;
    let (authority, root) = rest
        .split_once('/')
        .map_or((rest, "/".to_owned()), |(a, p)| (a, format!("/{p}")));
    let (target, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (
            host.to_owned(),
            Some(port.parse().map_err(|_| "invalid SSH port".to_owned())?),
        ),
        None => (authority.to_owned(), None),
    };
    if target.is_empty() || target.starts_with('-') {
        return Err("invalid SSH host".to_owned());
    }
    Ok((target, port, root.trim_end_matches('/').to_owned()))
}

impl Backend {
    fn ssh_command(target: &str, port: Option<u16>, remote: &str) -> Command {
        let mut cmd = Command::new("ssh");
        cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=15"]);
        // Reuse one authenticated connection for the thousands of range reads of a session.
        #[cfg(unix)]
        {
            let control = std::env::temp_dir().join("nie-ssh-%C");
            cmd.args(["-o", "ControlMaster=auto", "-o", "ControlPersist=120", "-o"])
                .arg(format!("ControlPath={}", control.display()));
        }
        if let Some(port) = port {
            cmd.arg("-p").arg(port.to_string());
        }
        cmd.arg("--").arg(target).arg(remote);
        cmd
    }

    fn ssh_run(target: &str, port: Option<u16>, remote: &str) -> Result<Vec<u8>> {
        let output = Self::ssh_command(target, port, remote)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("ssh: {e}"))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
        Ok(output.stdout)
    }
}

fn resolve_via_cli_fallback(source: &str) -> Result<String> {
    let (_, rest) = source
        .split_once("://")
        .ok_or_else(|| "invalid URI".to_owned())?;
    let trimmed = rest.trim_matches('/');
    if source.starts_with("rclone://") || source.starts_with("r2://") || source.starts_with("s3://")
    {
        if let Some((remote, sub)) = trimmed.split_once('/') {
            return Ok(format!("{remote}:{sub}"));
        }
        return Ok(format!("{trimmed}:"));
    }
    if source.starts_with("gdrive://") {
        if let Some((remote, sub)) = trimmed.split_once('/') {
            let r = if remote.is_empty() {
                "gdrive-ai"
            } else {
                remote
            };
            return Ok(format!("{r}:{sub}"));
        }
        let r = if trimmed.is_empty() {
            "gdrive-ai"
        } else {
            trimmed
        };
        return Ok(format!("{r}:"));
    }
    Err("nie-rclone binary required for this remote scheme".to_owned())
}

impl Backend {
    fn connect(source: &str) -> Result<Self> {
        if let Some(rest) = source.strip_prefix("http://") {
            let (authority, path) = match rest.split_once('/') {
                Some((a, p)) => (a, format!("/{}", p.trim_matches('/'))),
                None => (rest, String::new()),
            };
            let (host, port) = if let Some((h, p)) = authority.rsplit_once(':') {
                let port: u16 = p.parse().map_err(|_| "invalid HTTP port".to_owned())?;
                (h.to_owned(), port)
            } else {
                (authority.to_owned(), 80)
            };
            let base_path = if path == "/" || path.is_empty() {
                "/".to_owned()
            } else {
                format!("{path}/")
            };
            return Ok(Self::Http {
                host,
                port,
                base_path,
                token: std::env::var("NIE_HTTP_AUTH_TOKEN").ok(),
            });
        }

        if source.starts_with("ssh://") || source.starts_with("sftp://") {
            let (target, port, root) = parse_ssh_uri(source)?;
            // Fail at mount time, not at the first read, when the host is unreachable.
            Self::ssh_run(&target, port, &format!("test -d {}", shell_quote(&root)))?;
            return Ok(Self::Ssh { target, port, root });
        }

        if let Some(bin) = find_nie_rclone_bin() {
            rclone_resolve_spec(source).map_err(|e| e.to_string())?;
            let mut child = Command::new(bin)
                .arg("nie-rpc")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .map_err(|e| e.to_string())?;
            let stdin = child
                .stdin
                .take()
                .ok_or_else(|| "failed to open nie-rclone stdin".to_owned())?;
            let stdout = BufReader::new(
                child
                    .stdout
                    .take()
                    .ok_or_else(|| "failed to open nie-rclone stdout".to_owned())?,
            );
            return Ok(Self::RcloneGo(RpcWorker {
                child,
                stdin,
                stdout,
                uri: source.to_owned(),
            }));
        }

        Ok(Self::RcloneCli {
            remote_spec: resolve_via_cli_fallback(source)?,
        })
    }

    fn read(&mut self, path: &str, range: Option<(u64, usize)>) -> Result<Vec<u8>> {
        match self {
            Self::Http {
                host,
                port,
                base_path,
                token,
            } => http_read(host, *port, base_path, token.as_deref(), path, range),

            Self::RcloneGo(rpc) => {
                let (offset, length) = match range {
                    Some((off, len)) => (off as i64, len as i64),
                    None => (0i64, -1i64),
                };
                let req = serde_json::json!({
                    "op": "read",
                    "uri": rpc.uri,
                    "path": path,
                    "offset": offset,
                    "length": length,
                });
                let mut line = serde_json::to_vec(&req).map_err(|e| e.to_string())?;
                line.push(b'\n');
                rpc.stdin.write_all(&line).map_err(|e| e.to_string())?;
                rpc.stdin.flush().map_err(|e| e.to_string())?;
                let mut header = String::new();
                rpc.stdout
                    .read_line(&mut header)
                    .map_err(|e| e.to_string())?;
                let meta: serde_json::Value =
                    serde_json::from_str(header.trim()).map_err(|e| e.to_string())?;
                if !meta["ok"].as_bool().unwrap_or(false) {
                    return Err(meta["error"]
                        .as_str()
                        .unwrap_or("nie-rclone read failed")
                        .to_owned());
                }
                let n = usize::try_from(meta["len"].as_u64().unwrap_or(0))
                    .map_err(|_| "response length overflow".to_owned())?;
                if n > MAX_READ {
                    return Err("remote response exceeds 256 MiB bound".into());
                }
                let mut data = vec![0u8; n];
                rpc.stdout
                    .read_exact(&mut data)
                    .map_err(|e| e.to_string())?;
                if let Some((_, want)) = range
                    && data.len() != want
                {
                    return Err("remote rclone range read truncated".into());
                }
                Ok(data)
            }

            Self::Ssh { target, port, root } => {
                let file = shell_quote(&format!("{root}/{path}"));
                let remote = match range {
                    Some((offset, length)) => format!(
                        "dd if={file} iflag=skip_bytes,count_bytes skip={offset} count={length} bs=1048576 status=none"
                    ),
                    None => format!("cat {file}"),
                };
                let data = Self::ssh_run(target, *port, &remote)?;
                if let Some((_, want)) = range
                    && data.len() != want
                {
                    return Err("ssh range read truncated".into());
                }
                Ok(data)
            }

            Self::RcloneCli { remote_spec } => {
                let target = if remote_spec.ends_with(':') || remote_spec.ends_with('/') {
                    format!("{remote_spec}{path}")
                } else {
                    format!("{remote_spec}/{path}")
                };
                let mut cmd = std::process::Command::new("rclone");
                cmd.arg("cat").arg(&target);
                if let Some((offset, size)) = range {
                    cmd.arg("--offset").arg(offset.to_string());
                    cmd.arg("--count").arg(size.to_string());
                }
                let output = cmd.output().map_err(|e| e.to_string())?;
                if !output.status.success() {
                    return Err(String::from_utf8_lossy(&output.stderr).into_owned());
                }
                if let Some((_, want)) = range
                    && output.stdout.len() != want
                {
                    return Err("rclone cat returned truncated range".into());
                }
                Ok(output.stdout)
            }
        }
    }

    fn list(&mut self, path: &str) -> Result<Vec<String>> {
        match self {
            Self::Http {
                host,
                port,
                base_path,
                token,
            } => {
                let dir_prefix = format!("{base_path}{path}");
                let bytes = http_read(host, *port, base_path, token.as_deref(), path, None)?;
                if bytes.len() > 4 * 1024 * 1024 {
                    return Err("HTTP directory listing exceeds 4 MiB".into());
                }
                let text = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
                Ok(extract_html_links(text, &dir_prefix))
            }

            Self::RcloneGo(rpc) => {
                let req = serde_json::json!({
                    "op": "list",
                    "uri": rpc.uri,
                    "path": path,
                });
                let mut line = serde_json::to_vec(&req).map_err(|e| e.to_string())?;
                line.push(b'\n');
                rpc.stdin.write_all(&line).map_err(|e| e.to_string())?;
                rpc.stdin.flush().map_err(|e| e.to_string())?;
                let mut header = String::new();
                rpc.stdout
                    .read_line(&mut header)
                    .map_err(|e| e.to_string())?;
                let meta: serde_json::Value =
                    serde_json::from_str(header.trim()).map_err(|e| e.to_string())?;
                if !meta["ok"].as_bool().unwrap_or(false) {
                    return Err(meta["error"]
                        .as_str()
                        .unwrap_or("nie-rclone list failed")
                        .to_owned());
                }
                let items: Vec<String> =
                    serde_json::from_value(meta["items"].clone()).map_err(|e| e.to_string())?;
                Ok(items)
            }

            Self::Ssh { target, port, root } => {
                let dir = shell_quote(&format!("{root}/{}", path.trim_matches('/')));
                let out = Self::ssh_run(target, *port, &format!("ls -1Ap -- {dir}"))?;
                let mut names: Vec<String> = String::from_utf8_lossy(&out)
                    .lines()
                    .map(str::to_owned)
                    .collect();
                names.sort();
                Ok(names)
            }

            Self::RcloneCli { remote_spec } => {
                let trimmed = path.trim_matches('/');
                let target = if trimmed.is_empty() {
                    remote_spec.clone()
                } else if remote_spec.ends_with(':') || remote_spec.ends_with('/') {
                    format!("{remote_spec}{trimmed}")
                } else {
                    format!("{remote_spec}/{trimmed}")
                };
                let output = std::process::Command::new("rclone")
                    .args(["lsjson", "--fast-list", &target])
                    .output()
                    .map_err(|e| e.to_string())?;
                if !output.status.success() {
                    return Err(String::from_utf8_lossy(&output.stderr).into_owned());
                }
                let entries: Vec<serde_json::Value> =
                    serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
                let mut names = Vec::with_capacity(entries.len());
                for item in entries {
                    if let Some(name) = item.get("Name").and_then(serde_json::Value::as_str) {
                        let is_dir = item
                            .get("IsDir")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false);
                        names.push(if is_dir {
                            format!("{name}/")
                        } else {
                            name.to_owned()
                        });
                    }
                }
                names.sort();
                names.dedup();
                Ok(names)
            }
        }
    }
}

fn http_read(
    host: &str,
    port: u16,
    base_path: &str,
    token: Option<&str>,
    path: &str,
    range: Option<(u64, usize)>,
) -> Result<Vec<u8>> {
    let mut stream = TcpStream::connect((host, port)).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(TIMEOUT))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(TIMEOUT))
        .map_err(|e| e.to_string())?;

    let req_path = format!("{base_path}{path}");
    let mut req = format!(
        "GET {req_path} HTTP/1.1\r\nHost: {host}:{port}\r\nAccept-Encoding: identity\r\nConnection: close\r\n"
    );
    if let Some(tok) = token {
        req.push_str(&format!("Authorization: Bearer {tok}\r\n"));
    }
    if let Some((offset, size)) = range {
        req.push_str(&format!(
            "Range: bytes={offset}-{}\r\n",
            offset + size as u64 - 1
        ));
    }
    req.push_str("\r\n");
    stream
        .write_all(req.as_bytes())
        .map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())?;

    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader
        .read_line(&mut status_line)
        .map_err(|e| e.to_string())?;
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| "invalid HTTP status line".to_owned())?;

    let mut content_range: Option<String> = None;
    let mut content_encoding: Option<String> = None;
    let mut content_length: Option<u64> = None;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).map_err(|e| e.to_string())?;
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some((k, v)) = trimmed.split_once(':') {
            let key = k.trim().to_ascii_lowercase();
            let val = v.trim().to_owned();
            match key.as_str() {
                "content-range" => content_range = Some(val),
                "content-encoding" => content_encoding = Some(val),
                "content-length" => content_length = val.parse().ok(),
                _ => {}
            }
        }
    }

    if let Some((offset, size)) = range {
        if status != 206 {
            return Err("HTTP server must support byte ranges (206)".into());
        }
        let header = content_range.ok_or("HTTP Content-Range required")?;
        let expected = format!("bytes {offset}-{}/", offset + size as u64 - 1);
        if !header.strip_prefix(&expected).is_some_and(|total| {
            total
                .parse::<u64>()
                .is_ok_and(|n| n >= offset + size as u64)
        }) {
            return Err("HTTP Content-Range does not match requested bytes".into());
        }
    } else if !(200..300).contains(&status) {
        return Err(format!("HTTP status {status}"));
    }

    if content_encoding.is_some_and(|h| h != "identity") {
        return Err("HTTP resource encoding must be identity".into());
    }

    let bound = range.map_or(
        if path.is_empty() || path.ends_with('/') {
            4 * 1024 * 1024
        } else {
            MAX_READ
        },
        |(_, length)| length,
    );
    if content_length.is_some_and(|n| n > bound as u64) {
        return Err("HTTP response exceeds requested budget".into());
    }

    let mut data = Vec::new();
    reader
        .take(bound as u64 + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() > bound || range.is_some_and(|(_, n)| data.len() != n) {
        return Err("HTTP response is oversized or truncated".into());
    }
    Ok(data)
}

fn extract_html_links(html: &str, dir_prefix: &str) -> Vec<String> {
    let mut names = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut idx = 0usize;
    while let Some(pos) = lower[idx..].find("href=") {
        let after = idx + pos + 5;
        if after >= html.len() {
            break;
        }
        let quote = html.as_bytes()[after];
        if quote != b'"' && quote != b'\'' {
            idx = after;
            continue;
        }
        let start = after + 1;
        let Some(end_rel) = html[start..].find(quote as char) else {
            break;
        };
        let href = &html[start..start + end_rel];
        idx = start + end_rel + 1;
        if href.contains(['?', '#']) || href.contains("://") {
            continue;
        }
        let rel = href
            .strip_prefix(dir_prefix)
            .unwrap_or_else(|| href.trim_start_matches('/'));
        if !rel.is_empty() && !rel.trim_end_matches('/').contains('/') && validate_path(rel).is_ok()
        {
            names.push(rel.to_owned());
        }
    }
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{net::TcpListener, sync::atomic::AtomicBool};

    #[test]
    fn ssh_uri_splits_host_port_and_root() {
        assert_eq!(
            parse_ssh_uri("ssh://prd/srv/game/data").unwrap(),
            ("prd".to_owned(), None, "/srv/game/data".to_owned())
        );
        assert_eq!(
            parse_ssh_uri("sftp://ci@host:2222/data/").unwrap(),
            ("ci@host".to_owned(), Some(2222), "/data".to_owned())
        );
        assert!(parse_ssh_uri("ssh://-oProxyCommand=x/").is_err());
        assert!(parse_ssh_uri("ssh://host:notaport/").is_err());
    }

    #[test]
    fn shell_quote_survives_single_quotes() {
        assert_eq!(shell_quote("a'b c"), "'a'\\''b c'");
    }

    #[test]
    fn rclone_cli_fallback_covers_object_stores() {
        assert_eq!(
            resolve_via_cli_fallback("r2://r2/bucket/ievr").unwrap(),
            "r2:bucket/ievr"
        );
        assert_eq!(resolve_via_cli_fallback("s3://remote").unwrap(), "remote:");
        assert_eq!(
            resolve_via_cli_fallback("gdrive://drive/Games").unwrap(),
            "drive:Games"
        );
        assert!(resolve_via_cli_fallback("nievfs://x/y").is_err());
    }

    struct Server {
        source: String,
        stop: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl Server {
        fn start(
            ignore_range: bool,
            wrong_range: bool,
            bad_encoding: bool,
            files: Option<std::collections::HashMap<String, Vec<u8>>>,
        ) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let stop = Arc::new(AtomicBool::new(false));
            let flag = stop.clone();
            let thread = std::thread::spawn(move || {
                for connection in listener.incoming() {
                    if flag.load(Ordering::Relaxed) {
                        break;
                    }
                    let mut stream = connection.unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut bytes = Vec::new();
                    while !bytes.ends_with(b"\r\n\r\n") {
                        let mut buffer = [0; 1024];
                        let size = stream.read(&mut buffer).unwrap();
                        if size == 0 {
                            break;
                        }
                        bytes.extend_from_slice(&buffer[..size]);
                    }
                    let request = String::from_utf8_lossy(&bytes);
                    let range = request.lines().find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("range: bytes=")
                            .map(str::to_owned)
                    });
                    let path = request
                        .lines()
                        .next()
                        .unwrap()
                        .split_whitespace()
                        .nth(1)
                        .unwrap();
                    let payload = if let Some(files) = &files {
                        let Some(payload) = files.get(path) else {
                            stream
                                .write_all(b"HTTP/1.1 404 Missing\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                                .unwrap();
                            continue;
                        };
                        payload.as_slice()
                    } else {
                        b"0123456789".as_slice()
                    };
                    let (status, content_range, body) =
                        if let Some(range) = range.filter(|_| !ignore_range) {
                            let (start, end) = range.split_once('-').unwrap();
                            let start: usize = start.parse().unwrap();
                            let end: usize = end.parse().unwrap();
                            (
                                206,
                                format!(
                                    "Content-Range: bytes {}-{end}/{}\r\n",
                                    if wrong_range { start + 1 } else { start },
                                    payload.len()
                                ),
                                &payload[start..=end],
                            )
                        } else {
                            (200, String::new(), payload)
                        };
                    let encoding = if bad_encoding {
                        "Content-Encoding: gzip\r\n"
                    } else {
                        ""
                    };
                    write!(
                        stream,
                        "HTTP/1.1 {status} OK\r\n{content_range}{encoding}Content-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .unwrap();
                    stream.write_all(body).unwrap();
                }
            });
            Self {
                source: format!("http://{address}/"),
                stop,
                thread: Some(thread),
            }
        }
    }

    impl Drop for Server {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            let _ = std::net::TcpStream::connect(
                self.source
                    .trim_start_matches("http://")
                    .trim_end_matches('/'),
            );
            self.thread.take().unwrap().join().unwrap();
        }
    }

    #[test]
    fn http_requires_exact_ranges_and_returns_requested_bytes() {
        for (ignore, wrong, encoding) in [
            (false, false, false),
            (true, false, false),
            (false, true, false),
            (false, false, true),
        ] {
            let server = Server::start(ignore, wrong, encoding, None);
            let storage = RemoteStorage::connect(&server.source).unwrap();
            let result = storage.range("packs/resource.cpk", 3, 4);
            if ignore || wrong || encoding {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap(), b"3456");
            }
        }
    }

    #[test]
    fn uri_credentials_are_rejected_before_connecting() {
        for uri in [
            "http://user:pass@127.0.0.1/game",
            "http://user@127.0.0.1/game",
            "https://host/game?token=secret",
            "rclone://vps/data#frag",
            "r2://user:secret@bucket/path",
        ] {
            assert!(RemoteStorage::connect(uri).is_err());
            assert!(crate::game_source::resolve(Some(uri)).is_err());
        }
    }

    #[test]
    fn extract_html_links_keeps_immediate_children() {
        let html = r#"<a href="/data/packs/">packs/</a><a href="/data/packs/a.cpk">a.cpk</a><a href="/data/packs/sub/">sub/</a><a href="/data/packs/sub/b.cpk">b</a><a href="https://example.com/x">ext</a>"#;
        assert_eq!(
            extract_html_links(html, "/data/packs/"),
            vec!["a.cpk", "sub/"]
        );
    }

    #[test]
    fn rclone_core_bridges_specs_and_remotes() {
        if !rclone_core_available() {
            return;
        }
        let remotes = rclone_list_remotes().expect("rclone_list_remotes");
        assert!(remotes.starts_with('['));
        let resolved_nv = rclone_resolve_spec("nievfs://C:/Games/IEVR/data").unwrap();
        assert_eq!(resolved_nv, ":nievfs:C:/Games/IEVR/data");
        let resolved_rc = rclone_resolve_spec("rclone://vps/data").unwrap();
        assert_eq!(resolved_rc, "vps:data");
    }
}
