//! The build host: where heavy steps run, how a command reaches it and how the sources get there.
//!
//! The host is named by `NIE_BUILD_HOST` and keeps its work under `NIE_BUILD_DIR`, both falling
//! back to the `[build]` table of `deploy/build.toml`: the synced sources in `tree/`, one cargo
//! target directory per purpose next to it. With neither source the host is unconfigured and
//! heavy steps run on this machine. Sources travel as a tar stream of the files git knows about
//! (tracked or untracked, never ignored); files that disappeared since the previous sync are
//! pruned from the stored file list, so the remote tree equals the local one.

use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use toml_edit::DocumentMut;

/// The repository manifest that names the build host.
pub const MANIFEST: &str = "deploy/build.toml";

/// A program with its arguments and environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cmd {
    /// Program name or path.
    pub program: String,
    /// Arguments.
    pub args: Vec<String>,
    /// Environment variables set for the command.
    pub env: Vec<(String, String)>,
    /// Cargo target directory name under the build directory (`target`, `target-clippy`).
    pub target: &'static str,
}

impl Cmd {
    /// A command with no arguments, using the default target directory.
    #[must_use]
    pub fn new(program: &str) -> Self {
        Self {
            program: program.to_owned(),
            args: Vec::new(),
            env: Vec::new(),
            target: "target",
        }
    }

    /// `cargo <args...>`.
    #[must_use]
    pub fn cargo<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self::new("cargo").args(args)
    }

    /// Appends arguments.
    #[must_use]
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.args
            .extend(args.into_iter().map(|a| a.as_ref().to_owned()));
        self
    }

    /// Uses another cargo target directory on the build host.
    #[must_use]
    pub fn target_dir(mut self, name: &'static str) -> Self {
        self.target = name;
        self
    }

    /// Human readable form, for reports.
    #[must_use]
    pub fn display(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Quotes one word for a POSIX shell.
#[must_use]
pub fn sh_quote(word: &str) -> String {
    if !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./=:,+@%".contains(c))
    {
        return word.to_owned();
    }
    format!("'{}'", word.replace('\'', "'\\''"))
}

/// The machine that runs heavy steps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildHost {
    /// ssh destination (a name from `~/.ssh/config`); empty when unconfigured.
    pub name: String,
    /// Working directory on the host; empty when unconfigured.
    pub dir: String,
}

impl BuildHost {
    /// Reads `NIE_BUILD_HOST` and `NIE_BUILD_DIR`, then the manifest of the repository at `root`.
    #[must_use]
    pub fn from_env(root: &Path) -> Self {
        let manifest = std::fs::read_to_string(root.join(MANIFEST)).unwrap_or_default();
        Self::resolve(
            std::env::var("NIE_BUILD_HOST").ok(),
            std::env::var("NIE_BUILD_DIR").ok(),
            &manifest,
        )
    }

    /// Settings win over the manifest text; blank values count as unset.
    #[must_use]
    pub fn resolve(name: Option<String>, dir: Option<String>, manifest: &str) -> Self {
        let doc = manifest.parse::<DocumentMut>().unwrap_or_default();
        let field = |key: &str| doc["build"][key].as_str().map(str::to_owned);
        Self::from_vars(name.or_else(|| field("host")), dir.or_else(|| field("dir")))
    }

    /// Builds the host from optional settings; blank values leave it unconfigured.
    #[must_use]
    pub fn from_vars(name: Option<String>, dir: Option<String>) -> Self {
        let pick = |value: Option<String>| value.map(|v| v.trim().to_owned()).unwrap_or_default();
        Self {
            name: pick(name),
            dir: pick(dir).trim_end_matches('/').to_owned(),
        }
    }

    /// Whether both the host name and its directory are known.
    #[must_use]
    pub fn configured(&self) -> bool {
        !self.name.is_empty() && !self.dir.is_empty()
    }

    /// Directory holding the synced sources.
    #[must_use]
    pub fn tree(&self) -> String {
        format!("{}/tree", self.dir)
    }

    /// A directory next to the tree (cargo target directories, `dist`).
    #[must_use]
    pub fn path(&self, name: &str) -> String {
        format!("{}/{name}", self.dir)
    }

    /// Shell script that runs `cmd` in the synced tree.
    #[must_use]
    pub fn script(&self, cmd: &Cmd) -> String {
        let mut script = format!(
            "cd {} && export PATH=\"$HOME/.cargo/bin:$HOME/.local/bin:$PATH\" CARGO_TARGET_DIR={} && ",
            sh_quote(&self.tree()),
            sh_quote(&self.path(cmd.target)),
        );
        if !cmd.env.is_empty() {
            script.push_str("env ");
            for (key, value) in &cmd.env {
                script.push_str(&format!("{key}={} ", sh_quote(value)));
            }
        }
        script.push_str(&sh_quote(&cmd.program));
        for arg in &cmd.args {
            script.push(' ');
            script.push_str(&sh_quote(arg));
        }
        script
    }

    /// Arguments of the `ssh` call that runs `script` on the host.
    #[must_use]
    pub fn ssh_args(&self, script: &str) -> Vec<String> {
        vec![
            "-o".into(),
            "BatchMode=yes".into(),
            self.name.clone(),
            script.to_owned(),
        ]
    }

    /// Arguments of the reachability probe.
    #[must_use]
    pub fn probe_args(&self) -> Vec<String> {
        vec![
            "-o".into(),
            "BatchMode=yes".into(),
            "-o".into(),
            "ConnectTimeout=5".into(),
            self.name.clone(),
            "true".into(),
        ]
    }

    /// Whether this process already runs on the build host: `NIE_ON_BUILD_HOST` is set (the justfile sets it
    /// for `just fr1 <recipe>`), or the machine name equals the host name.
    #[must_use]
    pub fn is_here(&self) -> bool {
        if std::env::var_os("NIE_ON_BUILD_HOST").is_some_and(|v| !v.is_empty()) {
            return true;
        }
        let Ok(out) = Command::new("hostname").output() else {
            return false;
        };
        let here = String::from_utf8_lossy(&out.stdout)
            .trim()
            .to_ascii_lowercase();
        let name = self.name.to_ascii_lowercase();
        self.configured()
            && !here.is_empty()
            && (here == name || here.strip_prefix(&name).is_some_and(|r| r.starts_with('.')))
    }

    /// Whether the host answers a non-interactive ssh call.
    #[must_use]
    pub fn reachable(&self) -> bool {
        self.configured()
            && Command::new("ssh")
                .args(self.probe_args())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
    }

    /// Copies the working tree to `tree/` on the host and prunes files that no longer exist.
    ///
    /// # Errors
    /// When git, tar or ssh fail.
    pub fn sync(&self, root: &Path) -> Result<usize> {
        if !self.configured() {
            bail!("no build host: set NIE_BUILD_HOST and NIE_BUILD_DIR or fill {MANIFEST}");
        }
        let files = source_files(root)?;
        let list = std::env::temp_dir().join(format!("nie-sync-{}.list", std::process::id()));
        let mut nul = Vec::new();
        for file in &files {
            nul.extend_from_slice(file.as_bytes());
            nul.push(0);
        }
        std::fs::write(&list, nul).context("writing the file list")?;
        let result = self.push(root, &list, &files);
        let _ = std::fs::remove_file(&list);
        result.map(|()| files.len())
    }

    fn push(&self, root: &Path, list: &Path, files: &[String]) -> Result<()> {
        let tree = sh_quote(&self.tree());
        let mut tar = Command::new("tar")
            .args(["--null", "-T"])
            .arg(list)
            .args(["-cf", "-"])
            .current_dir(root)
            .stdout(Stdio::piped())
            .spawn()
            .context("starting tar")?;
        let stream = tar.stdout.take().context("tar stdout")?;
        let unpack = Command::new("ssh")
            .args(self.ssh_args(&format!("mkdir -p {tree} && cd {tree} && tar -xf -")))
            .stdin(Stdio::from(stream))
            .status()
            .context("starting ssh")?;
        let packed = tar.wait().context("waiting for tar")?;
        if !packed.success() || !unpack.success() {
            bail!("sync failed (tar {packed}, ssh {unpack})");
        }
        let stored = sh_quote(&self.path("tree.files"));
        let fresh = sh_quote(&self.path("tree.files.new"));
        let mut upload = Command::new("ssh")
            .args(self.ssh_args(&format!("cat > {fresh}")))
            .stdin(Stdio::piped())
            .spawn()
            .context("starting ssh")?;
        {
            let mut stdin = upload.stdin.take().context("ssh stdin")?;
            for file in files {
                writeln!(stdin, "{file}")?;
            }
        }
        if !upload.wait()?.success() {
            bail!("uploading the file list failed");
        }
        let prune = format!(
            "cd {dir} && if [ -f {stored} ]; then LC_ALL=C comm -23 {stored} {fresh} | (cd tree && xargs -r -d '\\n' rm -f); fi; mv {fresh} {stored}",
            dir = sh_quote(&self.dir)
        );
        let status = Command::new("ssh")
            .args(self.ssh_args(&prune))
            .status()
            .context("starting ssh")?;
        if !status.success() {
            bail!("pruning removed files failed");
        }
        Ok(())
    }
}

/// Files git knows about (tracked or untracked, not ignored) that exist on disk, sorted bytewise.
///
/// # Errors
/// When `git ls-files` cannot run.
pub fn source_files(root: &Path) -> Result<Vec<String>> {
    let out = Command::new("git")
        .args(["ls-files", "-z", "-c", "-o", "--exclude-standard"])
        .current_dir(root)
        .output()
        .context("running git ls-files")?;
    if !out.status.success() {
        bail!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let names: BTreeSet<String> = out
        .stdout
        .split(|b| *b == 0)
        .filter(|name| !name.is_empty())
        .map(|name| String::from_utf8_lossy(name).into_owned())
        .filter(|name| !name.contains('\n') && root.join(name).is_file())
        .collect();
    Ok(names.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_and_overrides() {
        let none = BuildHost::from_vars(None, None);
        assert!(!none.configured());
        assert!(!none.reachable());
        let manifest = "[build]\nhost = \"box\"\ndir = \"/work/nie/\"\n";
        let host = BuildHost::resolve(None, None, manifest);
        assert_eq!(
            (host.name.as_str(), host.dir.as_str()),
            ("box", "/work/nie")
        );
        assert!(host.configured());
        let host = BuildHost::resolve(Some(" other ".into()), Some("/x/y".into()), manifest);
        assert_eq!(host.name, "other");
        assert_eq!(host.tree(), "/x/y/tree");
        assert_eq!(host.path("target"), "/x/y/target");
        let blank = BuildHost::resolve(Some(String::new()), Some("  ".into()), "");
        assert_eq!(blank, none);
    }

    #[test]
    fn repository_manifest_names_a_host() {
        let text = include_str!("../../../deploy/build.toml");
        assert!(BuildHost::resolve(None, None, text).configured());
    }

    #[test]
    fn quoting_is_shell_safe() {
        assert_eq!(sh_quote("cargo"), "cargo");
        assert_eq!(sh_quote("a b"), "'a b'");
        assert_eq!(sh_quote("it's"), "'it'\\''s'");
        assert_eq!(sh_quote(""), "''");
        assert_eq!(sh_quote("$(rm)"), "'$(rm)'");
    }

    #[test]
    fn script_runs_in_the_tree_with_the_target_dir() {
        let host = BuildHost::from_vars(Some("box".into()), Some("/work/nie".into()));
        let cmd = Cmd::cargo(["clippy", "--", "-D", "warnings"]).target_dir("target-clippy");
        let script = host.script(&cmd);
        assert!(script.starts_with("cd /work/nie/tree && export PATH="));
        assert!(script.contains("CARGO_TARGET_DIR=/work/nie/target-clippy"));
        assert!(script.ends_with("cargo clippy -- -D warnings"));
        let mut with_env = Cmd::new("gitleaks");
        with_env.env.push(("A".into(), "b c".into()));
        assert!(host.script(&with_env).ends_with("env A='b c' gitleaks"));
    }

    #[test]
    fn ssh_arguments_never_prompt() {
        let host = BuildHost::from_vars(Some("h".into()), None);
        assert_eq!(host.ssh_args("ls"), ["-o", "BatchMode=yes", "h", "ls"]);
        assert_eq!(
            host.probe_args(),
            ["-o", "BatchMode=yes", "-o", "ConnectTimeout=5", "h", "true"]
        );
    }

    #[test]
    fn cmd_display_joins_words() {
        assert_eq!(Cmd::cargo(["fmt", "--all"]).display(), "cargo fmt --all");
    }
}
