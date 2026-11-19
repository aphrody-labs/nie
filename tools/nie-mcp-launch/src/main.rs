// SPDX-License-Identifier: Apache-2.0
//! Stdio launcher for the native nie MCP server (`crates/nie-cli/src/bin/nie-mcp.rs`).
//!
//! Resolution order, first hit wins:
//!   1. `NIE_MCP_BIN`                       explicit binary
//!   2. `<cargo target dir>/{release,debug}/nie-mcp[.exe]`, newest first; the target dir is
//!      `CARGO_TARGET_DIR`, else the `target_directory` of `cargo metadata`
//!   3. the build host over SSH (nie-ssh, else ssh)             `NIE_BUILD_HOST`, `NIE_BUILD_DIR`, else the
//!      `[build]` table of `deploy/build.toml` in the project directory
//!
//! Heavy builds stay off the PC: `just compile` builds on the build host of the same table.
//! Paths passed to tools resolve on the machine that runs the binary.
//!
//! stdio is inherited. On Unix the launcher replaces itself with the server (`exec`), so signals
//! reach it directly and its exit status is the launcher's. On Windows it waits for the child and
//! returns its exit code; the child sees EOF on stdin when the launcher goes away.

use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

const MANIFEST: &str = "deploy/build.toml";
const EXE: &str = if cfg!(windows) {
    "nie-mcp.exe"
} else {
    "nie-mcp"
};

/// Environment inputs of the resolution, gathered once so that it can be tested.
#[derive(Debug, Default, Clone)]
struct Config {
    bin: Option<PathBuf>,
    target_dir: Option<PathBuf>,
    host: Option<String>,
    dir: Option<String>,
}

impl Config {
    /// `root` is the project directory holding `deploy/build.toml`.
    fn from_env(root: &Path) -> Self {
        let var = |name: &str| env::var_os(name).filter(|v| !v.is_empty());
        let text = |name: &str| var(name).map(|v| v.to_string_lossy().into_owned());
        let manifest = fs::read_to_string(root.join(MANIFEST)).unwrap_or_default();
        Self {
            bin: var("NIE_MCP_BIN").map(PathBuf::from),
            target_dir: var("CARGO_TARGET_DIR").map(PathBuf::from),
            host: text("NIE_BUILD_HOST").or_else(|| build_key(&manifest, "host")),
            dir: text("NIE_BUILD_DIR").or_else(|| build_key(&manifest, "dir")),
        }
    }
}

/// `key = "value"` of the `[build]` table (all the manifest holds; no general TOML needed).
fn build_key(manifest: &str, key: &str) -> Option<String> {
    let mut in_build = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_build = line == "[build]";
        } else if in_build
            && let Some((name, value)) = line.split_once('=')
            && name.trim() == key
        {
            let value = value.split('#').next().unwrap_or_default().trim();
            let value = value.strip_prefix('"')?.strip_suffix('"')?;
            return (!value.is_empty()).then(|| value.trim_end_matches('/').to_owned());
        }
    }
    None
}

/// What to run.
#[derive(Debug, PartialEq, Eq)]
enum Launch {
    Local(PathBuf),
    Remote { host: String, dir: String },
    /// No local build and no build host configured.
    Missing,
}

/// The newest of `<target>/release/nie-mcp` and `<target>/debug/nie-mcp`; release wins ties.
fn newest_binary(target_dir: &Path) -> Option<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for profile in ["release", "debug"] {
        let path = target_dir.join(profile).join(EXE);
        let Ok(modified) = fs::metadata(&path).and_then(|m| m.modified()) else {
            continue;
        };
        if best.as_ref().is_none_or(|(t, _)| modified > *t) {
            best = Some((modified, path));
        }
    }
    best.map(|(_, path)| path)
}

/// First hit of the resolution order. `cargo_target_dir` is only called when neither
/// `NIE_MCP_BIN` nor `CARGO_TARGET_DIR` decides.
fn resolve(cfg: &Config, cargo_target_dir: impl FnOnce() -> Option<PathBuf>) -> Launch {
    if let Some(bin) = &cfg.bin {
        return Launch::Local(bin.clone());
    }
    let target = cfg.target_dir.clone().or_else(cargo_target_dir);
    if let Some(bin) = target.as_deref().and_then(newest_binary) {
        return Launch::Local(bin);
    }
    match (&cfg.host, &cfg.dir) {
        (Some(host), Some(dir)) => Launch::Remote {
            host: host.clone(),
            dir: dir.clone(),
        },
        _ => Launch::Missing,
    }
}

/// The remote shell snippet: exec the newest built binary of the build directory.
fn remote_script(host: &str, dir: &str) -> String {
    format!(
        "cd {dir} && bin=$(ls -t target/release/nie-mcp target/debug/nie-mcp 2>/dev/null | head -n1) && [ -n \"$bin\" ] && exec \"$bin\" || {{ echo \"nie-mcp not built in {host}:{dir}\" >&2; exit 127; }}"
    )
}

/// The SSH transport: `NIE_SSH_BIN`, else `nie-ssh` when it is on `PATH`, else OpenSSH's `ssh`.
fn ssh_program() -> OsString {
    if let Some(bin) = env::var_os("NIE_SSH_BIN").filter(|b| !b.is_empty()) {
        return bin;
    }
    let name = if cfg!(windows) { "nie-ssh.exe" } else { "nie-ssh" };
    env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
        .map_or_else(|| "ssh".into(), PathBuf::into_os_string)
}

/// True when `program` is OpenSSH's client (`ssh`, `ssh.exe`), which takes `-T -o BatchMode=yes`.
fn is_openssh(program: &OsStr) -> bool {
    Path::new(program)
        .file_stem()
        .is_some_and(|stem| stem.eq_ignore_ascii_case("ssh"))
}

/// Program and arguments of a [`Launch`], the remote one over `ssh_program`.
fn argv_with(launch: &Launch, ssh_program: &OsStr) -> (OsString, Vec<OsString>) {
    match launch {
        Launch::Local(bin) => (bin.clone().into_os_string(), Vec::new()),
        Launch::Missing => (OsString::new(), Vec::new()),
        Launch::Remote { host, dir } => {
            let flags: &[&str] = if is_openssh(ssh_program) {
                &["-T", "-o", "BatchMode=yes"]
            } else {
                &[]
            };
            (
                ssh_program.to_owned(),
                flags
                    .iter()
                    .map(OsString::from)
                    .chain([OsString::from(host), remote_script(host, dir).into()])
                    .collect(),
            )
        }
    }
}

/// Program and arguments of a [`Launch`].
fn argv(launch: &Launch) -> (OsString, Vec<OsString>) {
    argv_with(launch, &ssh_program())
}

/// Value of the JSON string field `key`, first occurrence (enough for `cargo metadata`).
fn json_string_field(json: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\"");
    let rest = json[json.find(&marker)? + marker.len()..].trim_start();
    let mut chars = rest
        .strip_prefix(':')?
        .trim_start()
        .strip_prefix('"')?
        .chars();
    let mut out = String::new();
    loop {
        match chars.next()? {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                'b' => out.push('\u{8}'),
                'f' => out.push('\u{c}'),
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
}

/// `target_directory` of the workspace around `root`, from `cargo metadata`.
fn cargo_target_dir(root: &Path) -> Option<PathBuf> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let meta = Command::new(cargo)
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--offline",
        ])
        .current_dir(root)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    json_string_field(&String::from_utf8_lossy(&meta.stdout), "target_directory").map(PathBuf::from)
}

fn project_root() -> PathBuf {
    env::var_os("CLAUDE_PROJECT_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn main() -> ExitCode {
    let root = project_root();
    let launch = resolve(&Config::from_env(&root), || cargo_target_dir(&root));
    if launch == Launch::Missing {
        eprintln!(
            "nie-mcp-launch: no local nie-mcp build and no build host: set NIE_MCP_BIN or NIE_BUILD_HOST and NIE_BUILD_DIR, or fill {MANIFEST}"
        );
        return ExitCode::from(127);
    }
    let (program, args) = argv(&launch);
    let mut command = Command::new(&program);
    command.args(&args).current_dir(&root);
    run(command, &program)
}

/// Replaces the launcher with the server so signals and the exit status are the server's own.
#[cfg(unix)]
fn run(mut command: Command, program: &OsString) -> ExitCode {
    use std::os::unix::process::CommandExt;
    let err = command.exec();
    eprintln!(
        "nie-mcp-launch: cannot run {}: {err}",
        program.to_string_lossy()
    );
    ExitCode::from(127)
}

/// Runs the server with inherited stdio and returns its exit code.
#[cfg(not(unix))]
fn run(mut command: Command, program: &OsString) -> ExitCode {
    match command.status() {
        Ok(status) => ExitCode::from(u8::try_from(status.code().unwrap_or(1)).unwrap_or(1)),
        Err(err) => {
            eprintln!(
                "nie-mcp-launch: cannot run {}: {err}",
                program.to_string_lossy()
            );
            ExitCode::from(127)
        }
    }
}

#[cfg(test)]
#[path = "../tests/support/tmp.rs"]
mod tmp;

#[cfg(test)]
mod tests {
    use super::tmp::TempDir;
    use super::*;
    use std::fs::File;
    use std::time::{Duration, SystemTime};

    fn touch(path: &Path, age_secs: u64) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let file = File::create(path).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(age_secs))
            .unwrap();
    }

    fn cfg() -> Config {
        Config {
            host: Some("box".into()),
            dir: Some("/work/nie".into()),
            ..Config::default()
        }
    }

    #[test]
    fn explicit_binary_wins_even_when_a_build_exists() {
        let dir = TempDir::new();
        touch(&dir.path().join("release").join(EXE), 0);
        let cfg = Config {
            bin: Some("/opt/custom/nie-mcp".into()),
            target_dir: Some(dir.path().into()),
            ..cfg()
        };
        let launch = resolve(&cfg, || panic!("cargo metadata must not run"));
        assert_eq!(launch, Launch::Local("/opt/custom/nie-mcp".into()));
    }

    #[test]
    fn newest_of_release_and_debug_is_picked() {
        let dir = TempDir::new();
        touch(&dir.path().join("release").join(EXE), 100);
        touch(&dir.path().join("debug").join(EXE), 10);
        let cfg = Config {
            target_dir: Some(dir.path().into()),
            ..cfg()
        };
        assert_eq!(
            resolve(&cfg, || panic!("CARGO_TARGET_DIR decides")),
            Launch::Local(dir.path().join("debug").join(EXE))
        );
        touch(&dir.path().join("release").join(EXE), 1);
        assert_eq!(
            resolve(&cfg, || None),
            Launch::Local(dir.path().join("release").join(EXE))
        );
    }

    #[test]
    fn release_wins_a_tie() {
        let dir = TempDir::new();
        let now = SystemTime::now();
        for profile in ["release", "debug"] {
            let path = dir.path().join(profile).join(EXE);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            File::create(&path).unwrap().set_modified(now).unwrap();
        }
        assert_eq!(
            newest_binary(dir.path()),
            Some(dir.path().join("release").join(EXE))
        );
    }

    #[test]
    fn a_single_profile_is_enough() {
        let dir = TempDir::new();
        touch(&dir.path().join("debug").join(EXE), 5);
        assert_eq!(
            newest_binary(dir.path()),
            Some(dir.path().join("debug").join(EXE))
        );
    }

    #[test]
    fn cargo_metadata_target_dir_is_the_fallback() {
        let dir = TempDir::new();
        touch(&dir.path().join("release").join(EXE), 0);
        let target = dir.path().to_path_buf();
        assert_eq!(
            resolve(&cfg(), || Some(target)),
            Launch::Local(dir.path().join("release").join(EXE))
        );
    }

    #[test]
    fn nothing_local_falls_back_to_the_build_host() {
        let dir = TempDir::new();
        let empty = dir.path().to_path_buf();
        let remote = Launch::Remote {
            host: "box".into(),
            dir: "/work/nie".into(),
        };
        assert_eq!(resolve(&cfg(), || Some(empty)), remote);
        assert_eq!(resolve(&cfg(), || None), remote);
        assert_eq!(resolve(&Config::default(), || None), Launch::Missing);
    }

    #[test]
    fn manifest_build_table_is_read() {
        let text = "# c\n[other]\nhost = \"no\"\n[build]\nhost = \"box\" # why\ndir = \"/w/nie/\"\n";
        assert_eq!(build_key(text, "host").as_deref(), Some("box"));
        assert_eq!(build_key(text, "dir").as_deref(), Some("/w/nie"));
        assert_eq!(build_key("", "host"), None);
        assert!(build_key(include_str!("../../../deploy/build.toml"), "host").is_some());
    }

    #[test]
    fn host_and_dir_overrides_reach_the_ssh_command() {
        let launch = Launch::Remote {
            host: "fr2".into(),
            dir: "/work/nie".into(),
        };
        let (program, args) = argv_with(&launch, OsStr::new("ssh"));
        assert_eq!(program, "ssh");
        let args: Vec<_> = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args[..4], ["-T", "-o", "BatchMode=yes", "fr2"]);
        assert!(args[4].starts_with("cd /work/nie && bin=$(ls -t target/release/nie-mcp"));
        assert!(args[4].contains("nie-mcp not built in fr2:/work/nie"));
        assert!(args[4].contains("exit 127"));
        assert_eq!(args.len(), 5);
    }

    #[test]
    fn nie_ssh_takes_the_host_and_the_script_only() {
        let launch = Launch::Remote {
            host: "box".into(),
            dir: "/work/nie".into(),
        };
        let (program, args) = argv_with(&launch, OsStr::new("/usr/bin/nie-ssh"));
        assert_eq!(program, "/usr/bin/nie-ssh");
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "box");
        assert!(args[1].to_string_lossy().starts_with("cd /work/nie && bin="));
    }

    #[test]
    fn local_launch_has_no_arguments() {
        let (program, args) = argv(&Launch::Local("/x/nie-mcp".into()));
        assert_eq!(program, "/x/nie-mcp");
        assert!(args.is_empty());
    }

    #[test]
    fn target_directory_is_read_from_cargo_metadata_json() {
        let json = r#"{"packages":[],"workspace_root":"C:\\src\\nie","target_directory":"C:\\src\\nie\\target","version":1}"#;
        assert_eq!(
            json_string_field(json, "target_directory").as_deref(),
            Some("C:\\src\\nie\\target")
        );
        let unicode = r#"{"target_directory":"/t/\u00e9\"q\""}"#;
        assert_eq!(
            json_string_field(unicode, "target_directory").as_deref(),
            Some("/t/\u{e9}\"q\"")
        );
        assert_eq!(json_string_field("{}", "target_directory"), None);
    }
}
