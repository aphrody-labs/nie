// SPDX-License-Identifier: Apache-2.0
//! Stdio round trips of the launcher against a fake nie-mcp. The fake is this test executable
//! itself (`harness = false`): with `NIE_FAKE_MCP` set it prints its arguments, working directory
//! and executable, echoes every stdin line and exits with `NIE_FAKE_EXIT`.

use std::env;
use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[path = "support/tmp.rs"]
mod tmp;

use tmp::TempDir;

const LAUNCHER: &str = env!("CARGO_BIN_EXE_nie-mcp-launch");
const EXE: &str = if cfg!(windows) {
    "nie-mcp.exe"
} else {
    "nie-mcp"
};
const SSH: &str = if cfg!(windows) { "ssh.exe" } else { "ssh" };

fn fake_server() -> ! {
    let mut out = std::io::stdout().lock();
    let args: Vec<String> = env::args().skip(1).collect();
    let _ = writeln!(out, "argc:{}", args.len());
    for arg in &args {
        let _ = writeln!(out, "arg:{arg}");
    }
    let _ = writeln!(out, "cwd:{}", env::current_dir().unwrap().display());
    let _ = writeln!(out, "exe:{}", env::current_exe().unwrap().display());
    let _ = out.flush();
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        let _ = writeln!(out, "echo:{line}");
        let _ = out.flush();
    }
    let code = env::var("NIE_FAKE_EXIT")
        .ok()
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    std::process::exit(code);
}

struct Run {
    code: Option<i32>,
    lines: Vec<String>,
}

impl Run {
    fn has(&self, line: &str) -> bool {
        self.lines.iter().any(|l| l == line)
    }
    fn value(&self, prefix: &str) -> Option<&str> {
        self.lines.iter().find_map(|l| l.strip_prefix(prefix))
    }
}

/// Launcher command with every resolution input cleared, the fake server mode on.
fn launcher() -> Command {
    let mut cmd = Command::new(LAUNCHER);
    for var in [
        "NIE_MCP_BIN",
        "CARGO_TARGET_DIR",
        "NIE_BUILD_HOST",
        "NIE_BUILD_DIR",
        "CLAUDE_PROJECT_DIR",
        "NIE_FAKE_EXIT",
    ] {
        cmd.env_remove(var);
    }
    cmd.env("NIE_FAKE_MCP", "1");
    cmd
}

fn run(mut cmd: Command, stdin: &str) -> Run {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn launcher");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    Run {
        code: out.status.code(),
        lines: String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::to_owned)
            .collect(),
    }
}

fn copy_self(dest: &Path) {
    fs::create_dir_all(dest.parent().unwrap()).unwrap();
    fs::copy(env::current_exe().unwrap(), dest).unwrap();
}

fn canonical(path: &Path) -> String {
    let p = fs::canonicalize(path).unwrap();
    p.to_string_lossy().trim_start_matches(r"\\?\").to_owned()
}

fn explicit_binary_round_trip() {
    let root = TempDir::new();
    let mut cmd = launcher();
    cmd.env("NIE_MCP_BIN", env::current_exe().unwrap())
        .env("CLAUDE_PROJECT_DIR", root.path());
    let r = run(cmd, "hello\nworld\n");
    assert_eq!(r.code, Some(0));
    assert!(r.has("argc:0"), "{:?}", r.lines);
    assert!(r.has("echo:hello") && r.has("echo:world"), "{:?}", r.lines);
    assert_eq!(
        r.value("cwd:").map(|c| c.trim_start_matches(r"\\?\")),
        Some(canonical(root.path()).as_str())
    );
}

fn exit_code_is_propagated() {
    let mut cmd = launcher();
    cmd.env("NIE_MCP_BIN", env::current_exe().unwrap())
        .env("NIE_FAKE_EXIT", "7");
    assert_eq!(run(cmd, "").code, Some(7));
}

fn cargo_target_dir_picks_the_built_binary() {
    let target = TempDir::new();
    copy_self(&target.path().join("release").join(EXE));
    let mut cmd = launcher();
    cmd.env("CARGO_TARGET_DIR", target.path());
    let r = run(cmd, "ping\n");
    assert_eq!(r.code, Some(0));
    assert!(r.has("echo:ping"), "{:?}", r.lines);
    assert!(
        r.value("exe:")
            .unwrap()
            .ends_with(&format!("release{}{EXE}", std::path::MAIN_SEPARATOR))
    );
}

fn cargo_metadata_finds_the_target_dir() {
    let project = TempDir::new();
    fs::write(
        project.path().join("Cargo.toml"),
        "[package]\nname = \"probe\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::create_dir_all(project.path().join("src")).unwrap();
    fs::write(project.path().join("src").join("lib.rs"), "").unwrap();
    copy_self(&project.path().join("target").join("debug").join(EXE));
    let mut cmd = launcher();
    cmd.env("CLAUDE_PROJECT_DIR", project.path());
    let r = run(cmd, "meta\n");
    assert_eq!(r.code, Some(0));
    assert!(r.has("echo:meta"), "{:?}", r.lines);
    assert!(
        r.value("exe:")
            .unwrap()
            .ends_with(&format!("debug{}{EXE}", std::path::MAIN_SEPARATOR))
    );
}

fn build_host_fallback_runs_ssh() {
    let bin = TempDir::new();
    copy_self(&bin.path().join(SSH));
    let empty_target = TempDir::new();
    let mut paths = vec![PathBuf::from(bin.path())];
    paths.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
    let mut cmd = launcher();
    cmd.env("CARGO_TARGET_DIR", empty_target.path())
        .env("PATH", env::join_paths(paths).unwrap())
        .env("NIE_BUILD_HOST", "host9")
        .env("NIE_BUILD_DIR", "/work/nie")
        .env("NIE_FAKE_EXIT", "127");
    let r = run(cmd, "remote\n");
    assert_eq!(r.code, Some(127), "{:?}", r.lines);
    assert!(r.has("argc:5"), "{:?}", r.lines);
    let args: Vec<&str> = r
        .lines
        .iter()
        .filter_map(|l| l.strip_prefix("arg:"))
        .collect();
    assert_eq!(args[..4], ["-T", "-o", "BatchMode=yes", "host9"]);
    assert!(args[4].starts_with("cd /work/nie && "), "{}", args[4]);
    assert!(args[4].contains("nie-mcp not built in host9:/work/nie"));
    assert!(r.has("echo:remote"));
}

#[cfg(unix)]
fn sigterm_reaches_the_server() {
    use std::os::unix::process::ExitStatusExt;
    let mut child = launcher()
        .env("NIE_MCP_BIN", env::current_exe().unwrap())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    for _ in 0..3 {
        line.clear();
        stdout.read_line(&mut line).unwrap();
    }
    let status = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
    let exit = child.wait().unwrap();
    assert_eq!(exit.signal(), Some(15));
}

fn main() {
    if env::var_os("NIE_FAKE_MCP").is_some() {
        fake_server();
    }
    let tests: &[(&str, fn())] = &[
        ("explicit_binary_round_trip", explicit_binary_round_trip),
        ("exit_code_is_propagated", exit_code_is_propagated),
        (
            "cargo_target_dir_picks_the_built_binary",
            cargo_target_dir_picks_the_built_binary,
        ),
        (
            "cargo_metadata_finds_the_target_dir",
            cargo_metadata_finds_the_target_dir,
        ),
        ("build_host_fallback_runs_ssh", build_host_fallback_runs_ssh),
        #[cfg(unix)]
        ("sigterm_reaches_the_server", sigterm_reaches_the_server),
    ];
    for (name, test) in tests {
        test();
        println!("test {name} ... ok");
    }
    println!("\ntest result: ok. {} passed", tests.len());
}
