// SPDX-License-Identifier: Apache-2.0
//! Stdio launcher for the native nie MCP server (`crates/nie-cli/src/bin/nie-mcp.rs`).
//!
//! Resolution order, first hit wins:
//!   1. `NIE_MCP_BIN`                       explicit binary
//!   2. `<cargo target dir>/{release,debug}/nie-mcp[.exe]`, newest first; the target dir is
//!      `CARGO_TARGET_DIR`, else the `target_directory` of `cargo metadata`
//!   3. the build host over SSH             `NIE_BUILD_HOST` (default `fr1`), `NIE_BUILD_DIR`
//!      (default `/srv/xbuild/nie`)
//!
//! Heavy builds stay off the PC: build with
//! `ssh fr1 'cd /srv/xbuild/nie && cargo build -p nie-cli --bin nie-mcp --features host'`.
//! Paths passed to tools resolve on the machine that runs the binary.
//!
//! stdio is inherited. On Unix the launcher replaces itself with the server (`exec`), so signals
//! reach it directly and its exit status is the launcher's. On Windows it waits for the child and
//! returns its exit code; the child sees EOF on stdin when the launcher goes away.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

const DEFAULT_HOST: &str = "fr1";
const DEFAULT_DIR: &str = "/srv/xbuild/nie";
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
    host: String,
    dir: String,
}

impl Config {
    fn from_env() -> Self {
        let var = |name: &str| env::var_os(name).filter(|v| !v.is_empty());
        let text = |name: &str, default: &str| {
            var(name).map_or_else(|| default.to_owned(), |v| v.to_string_lossy().into_owned())
        };
        Self {
            bin: var("NIE_MCP_BIN").map(PathBuf::from),
            target_dir: var("CARGO_TARGET_DIR").map(PathBuf::from),
            host: text("NIE_BUILD_HOST", DEFAULT_HOST),
            dir: text("NIE_BUILD_DIR", DEFAULT_DIR),
        }
    }
}

/// What to run.
#[derive(Debug, PartialEq, Eq)]
enum Launch {
    Local(PathBuf),
    Remote { host: String, dir: String },
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
    Launch::Remote {
        host: cfg.host.clone(),
        dir: cfg.dir.clone(),
    }
}

/// The remote shell snippet: exec the newest built binary of the build directory.
fn remote_script(host: &str, dir: &str) -> String {
    format!(
        "cd {dir} && bin=$(ls -t target/release/nie-mcp target/debug/nie-mcp 2>/dev/null | head -n1) && [ -n \"$bin\" ] && exec \"$bin\" || {{ echo \"nie-mcp not built in {host}:{dir}\" >&2; exit 127; }}"
    )
}

/// Program and arguments of a [`Launch`].
fn argv(launch: &Launch) -> (OsString, Vec<OsString>) {
    match launch {
        Launch::Local(bin) => (bin.clone().into_os_string(), Vec::new()),
        Launch::Remote { host, dir } => (
            "ssh".into(),
            ["-T", "-o", "BatchMode=yes", host]
                .into_iter()
                .map(OsString::from)
                .chain([remote_script(host, dir).into()])
                .collect(),
        ),
    }
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
    let launch = resolve(&Config::from_env(), || cargo_target_dir(&root));
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
            host: DEFAULT_HOST.into(),
            dir: DEFAULT_DIR.into(),
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
            host: "fr1".into(),
            dir: "/srv/xbuild/nie".into(),
        };
        assert_eq!(resolve(&cfg(), || Some(empty)), remote);
        assert_eq!(resolve(&cfg(), || None), remote);
    }

    #[test]
    fn host_and_dir_overrides_reach_the_ssh_command() {
        let launch = Launch::Remote {
            host: "fr2".into(),
            dir: "/work/nie".into(),
        };
        let (program, args) = argv(&launch);
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
