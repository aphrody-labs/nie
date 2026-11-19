//! Small helpers shared by the tasks.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// Root of the checkout: the git top level, else the nearest ancestor that holds `rust-toolchain.toml`
/// next to a `justfile` (the synced tree on the build host has no `.git`), else the current directory.
pub fn repo_root() -> PathBuf {
    let from_git = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()));
    from_git
        .or_else(|| {
            let cwd = std::env::current_dir().ok()?;
            cwd.ancestors()
                .find(|d| d.join("rust-toolchain.toml").is_file() && d.join("justfile").is_file())
                .map(Path::to_path_buf)
        })
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Runs a command and returns its trimmed standard output; the error carries the standard error.
pub fn capture(cmd: &mut Command) -> Result<String> {
    let out = cmd.output().with_context(|| format!("spawn {cmd:?}"))?;
    if !out.status.success() {
        bail!(
            "{cmd:?} failed ({}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// Runs a command with inherited standard streams; a non-zero exit is an error.
pub fn run(cmd: &mut Command) -> Result<()> {
    let status = cmd.status().with_context(|| format!("spawn {cmd:?}"))?;
    if !status.success() {
        bail!("{cmd:?} exited with {status}");
    }
    Ok(())
}

/// `git -C <repo>`.
pub fn git(repo: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo);
    cmd
}

/// Lowercase hexadecimal.
pub fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// Whether `program` resolves on `PATH`.
pub fn have(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}
