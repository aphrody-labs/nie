//! The supervisor configuration (`/etc/nie/supervise.toml`): services, periodic jobs and the paths
//! they need. One file describes a host; `deploy/supervise.toml` is the production one and is
//! embedded in the binary.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// The production configuration shipped inside the binary.
pub const EMBEDDED: &str = include_str!("../../../deploy/supervise.toml");

/// Where `nie-deploy up` installs the configuration.
pub const INSTALLED: &str = "/etc/nie/supervise.toml";

/// A whole host.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Named values substituted as `${name}` in paths, `exec` and `env` of the entries below.
    #[serde(default)]
    pub vars: BTreeMap<String, String>,
    /// Supervisor-wide settings.
    #[serde(default)]
    pub supervisor: Settings,
    /// Files and directories created or re-owned before the first service starts.
    #[serde(default, rename = "path")]
    pub paths: Vec<PathSpec>,
    /// Services (restarted forever) and periodic jobs (`every_secs`).
    #[serde(default, rename = "service")]
    pub services: Vec<Service>,
}

/// Supervisor-wide settings.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Settings {
    /// Log directory: one rotated file per service plus `supervisor.log`.
    pub log_dir: PathBuf,
    /// Runtime directory: lock, pid and `status.json`.
    pub run_dir: PathBuf,
    /// Size at which a log file rotates.
    pub log_max_bytes: u64,
    /// Rotated files kept per log.
    pub log_keep: u32,
    /// Where service binaries are installed by `nie-deploy up`.
    pub install_dir: PathBuf,
    /// Seconds `up` waits for every health probe to pass.
    pub up_timeout_secs: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            log_dir: "/var/log/nie".into(),
            run_dir: "/run/nie".into(),
            log_max_bytes: 10 * 1024 * 1024,
            log_keep: 5,
            install_dir: "/usr/local/bin".into(),
            up_timeout_secs: 180,
        }
    }
}

/// A path prepared before the services start.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathSpec {
    /// Absolute path.
    pub path: PathBuf,
    /// Create it as a directory when missing; otherwise it must exist (a secret file), or the entry
    /// is skipped when `optional`.
    #[serde(default)]
    pub create: bool,
    /// Skip a missing path.
    #[serde(default)]
    pub optional: bool,
    /// Owner user name.
    #[serde(default)]
    pub user: Option<String>,
    /// Owner group name (default: the primary group of `user`).
    #[serde(default)]
    pub group: Option<String>,
    /// Permission bits, octal in the file as a string (`"0750"`).
    #[serde(default)]
    pub mode: Option<String>,
}

/// A supervised program.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Service {
    /// Unique name; also the log file name.
    pub name: String,
    /// Program and arguments (no shell).
    pub exec: Vec<String>,
    /// Disabled entries are ignored.
    #[serde(default = "yes")]
    pub enabled: bool,
    /// User to drop to (supplementary groups included); the supervisor user when absent.
    #[serde(default)]
    pub user: Option<String>,
    /// Group to drop to (default: the primary group of `user`).
    #[serde(default)]
    pub group: Option<String>,
    /// Working directory.
    #[serde(default)]
    pub workdir: Option<PathBuf>,
    /// Environment variables.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// `KEY=value` files read at every start (a missing file is ignored).
    #[serde(default)]
    pub env_files: Vec<PathBuf>,
    /// Run again this many seconds after each exit instead of restarting with backoff.
    #[serde(default)]
    pub every_secs: Option<u64>,
    /// First restart delay.
    #[serde(default = "one")]
    pub backoff_min_secs: u64,
    /// Longest restart delay.
    #[serde(default = "sixty")]
    pub backoff_max_secs: u64,
    /// A run at least this long resets the backoff.
    #[serde(default = "sixty")]
    pub stable_secs: u64,
    /// Seconds between SIGTERM and SIGKILL.
    #[serde(default = "twenty")]
    pub stop_timeout_secs: u64,
    /// `RLIMIT_NOFILE`.
    #[serde(default)]
    pub nofile: Option<u64>,
    /// `RLIMIT_NPROC`.
    #[serde(default)]
    pub nproc: Option<u64>,
    /// `RLIMIT_AS` in MiB.
    #[serde(default)]
    pub memory_mib: Option<u64>,
    /// Keep core dumps off (`RLIMIT_CORE` 0); on by default.
    #[serde(default = "yes")]
    pub no_core: bool,
    /// Health probe.
    #[serde(default)]
    pub health: Option<Health>,
}

/// An HTTP probe: the service is restarted after `failures` consecutive failures.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Health {
    /// `http://host:port/path`.
    pub url: String,
    /// Text the body must contain.
    #[serde(default)]
    pub contains: Option<String>,
    /// Seconds between probes.
    #[serde(default = "fifteen")]
    pub interval_secs: u64,
    /// Seconds before a probe gives up.
    #[serde(default = "five")]
    pub timeout_secs: u64,
    /// Consecutive failures that restart the service.
    #[serde(default = "four")]
    pub failures: u32,
    /// Seconds after a start during which failures are not counted.
    #[serde(default = "grace")]
    pub grace_secs: u64,
}

const fn yes() -> bool {
    true
}
const fn one() -> u64 {
    1
}
const fn four() -> u32 {
    4
}
const fn five() -> u64 {
    5
}
const fn fifteen() -> u64 {
    15
}
const fn twenty() -> u64 {
    20
}
const fn sixty() -> u64 {
    60
}
const fn grace() -> u64 {
    120
}

impl Config {
    /// Parse and validate TOML text.
    pub fn parse(text: &str) -> Result<Self> {
        let mut config: Self =
            toml::from_str(text).context("parse the supervisor configuration")?;
        config.expand()?;
        config.validate()?;
        Ok(config)
    }

    /// Read a file, or standard input for `-`.
    pub fn load(path: &Path) -> Result<Self> {
        let text = if path == Path::new("-") {
            std::io::read_to_string(std::io::stdin()).context("read the configuration from stdin")?
        } else {
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?
        };
        Self::parse(&text)
    }

    /// Replaces `${name}` from `vars` in every path, `exec` word and `env` value.
    fn expand(&mut self) -> Result<()> {
        let vars = std::mem::take(&mut self.vars);
        let sub = |text: &str| substitute(text, &vars);
        let sub_path = |path: &Path| -> Result<PathBuf> { Ok(sub(&path.to_string_lossy())?.into()) };
        for spec in &mut self.paths {
            spec.path = sub_path(&spec.path)?;
        }
        for svc in &mut self.services {
            for word in &mut svc.exec {
                *word = sub(word)?;
            }
            for value in svc.env.values_mut() {
                *value = sub(value)?;
            }
            for file in &mut svc.env_files {
                *file = sub_path(file)?;
            }
            if let Some(dir) = &svc.workdir {
                svc.workdir = Some(sub_path(dir)?);
            }
        }
        self.vars = vars;
        Ok(())
    }

    /// Reject what the supervisor cannot run.
    pub fn validate(&self) -> Result<()> {
        let mut names = std::collections::BTreeSet::new();
        for svc in &self.services {
            if svc.name.is_empty()
                || !svc.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                bail!("service name {:?}: use letters, digits, '-' and '_'", svc.name);
            }
            if svc.name == "supervisor" || !names.insert(svc.name.as_str()) {
                bail!("service name {:?} is reserved or duplicated", svc.name);
            }
            if svc.exec.is_empty() {
                bail!("service {}: empty exec", svc.name);
            }
            if svc.backoff_min_secs == 0 || svc.backoff_min_secs > svc.backoff_max_secs {
                bail!("service {}: backoff_min_secs must be in 1..=backoff_max_secs", svc.name);
            }
            if svc.every_secs == Some(0) {
                bail!("service {}: every_secs must be positive", svc.name);
            }
            if let Some(health) = &svc.health {
                nie_health::parse_url(&health.url)
                    .map_err(|e| anyhow::anyhow!("service {}: {e}", svc.name))?;
            }
        }
        for spec in &self.paths {
            if !spec.path.is_absolute() {
                bail!("path {}: not absolute", spec.path.display());
            }
            if let Some(mode) = &spec.mode {
                parse_mode(mode).with_context(|| format!("path {}", spec.path.display()))?;
            }
        }
        Ok(())
    }

    /// The enabled services.
    pub fn enabled(&self) -> impl Iterator<Item = &Service> {
        self.services.iter().filter(|s| s.enabled)
    }
}

/// `${name}` replaced by its value in `vars`; an unknown name is an error.
fn substitute(text: &str, vars: &BTreeMap<String, String>) -> Result<String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find('}').with_context(|| format!("unclosed `${{` in {text:?}"))?;
        let name = &after[..end];
        out.push_str(vars.get(name).with_context(|| format!("unknown variable ${{{name}}} in {text:?}"))?);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// An octal mode such as `"0750"`.
pub fn parse_mode(text: &str) -> Result<u32> {
    u32::from_str_radix(text.trim_start_matches("0o"), 8)
        .with_context(|| format!("mode {text:?} is not octal"))
}

/// `KEY=value` lines (`#` comments, optional `export `, optional quotes around the value).
#[must_use]
pub fn parse_env_file(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            let line = line.strip_prefix("export ").unwrap_or(line);
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                .unwrap_or(value);
            Some((key.trim().to_owned(), value.to_owned()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_configuration_is_valid() {
        let config = Config::parse(EMBEDDED).unwrap();
        assert!(config.enabled().any(|s| s.name == "nie-serve"));
    }

    #[test]
    fn vars_expand_in_exec_env_and_paths() {
        let text = "[vars]\ngame = '/g d'\n[[path]]\npath = '${game}/x'\n[[service]]\nname = 'a'\nexec = ['run', '--in=${game}']\nenv = { G = '${game}' }\n";
        let config = Config::parse(text).unwrap();
        assert_eq!(config.paths[0].path, Path::new("/g d/x"));
        assert_eq!(config.services[0].exec[1], "--in=/g d");
        assert_eq!(config.services[0].env["G"], "/g d");
        let unknown = "[[service]]\nname = 'a'\nexec = ['${nope}']\n";
        assert!(Config::parse(unknown).unwrap_err().to_string().contains("unknown variable"));
    }

    #[test]
    fn rejects_duplicates_and_empty_exec() {
        let dup = "[[service]]\nname='a'\nexec=['x']\n[[service]]\nname='a'\nexec=['y']\n";
        assert!(Config::parse(dup).unwrap_err().to_string().contains("duplicated"));
        assert!(Config::parse("[[service]]\nname='a'\nexec=[]\n").is_err());
        assert!(Config::parse("[[service]]\nname='a b'\nexec=['x']\n").is_err());
        assert!(Config::parse("[supervisor]\nunknown=1\n").is_err());
    }

    #[test]
    fn env_files_parse() {
        let parsed = parse_env_file("# c\nA=1\nexport B=\"two words\"\nC='x'\n\nbad\n");
        assert_eq!(
            parsed,
            vec![
                ("A".into(), "1".into()),
                ("B".into(), "two words".into()),
                ("C".into(), "x".into())
            ]
        );
        assert_eq!(parse_mode("0750").unwrap(), 0o750);
    }
}
