//! `nie-deploy up` and `nie-deploy status`: install the supervisor, its configuration and the
//! binaries on a host (over `nie-ssh` for `--host`), start it detached and report what runs.
//! Nothing here touches systemd or a container runtime.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use graviola::hashing::{Hash, Sha256};
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use serde::Serialize;

use crate::config::{Config, EMBEDDED, INSTALLED};
use crate::host::{is_root, write_if_changed};
use crate::logs::now_secs;
use crate::supervise::{self, Status};

const RC_LOCAL: &str = "/etc/rc.local";
const CRON_FILE: &str = "/etc/cron.d/nie-deploy";
const BEGIN: &str = "# BEGIN nie-deploy";
const END: &str = "# END nie-deploy";

/// How the supervisor is started at boot, with no unit of nie in the init system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BootHook {
    /// `/etc/cron.d/nie-deploy`: an `@reboot` entry plus a one-minute watchdog for a dead supervisor.
    Cron,
    /// A managed block in `/etc/rc.local`.
    RcLocal,
    /// The host offers neither.
    None,
}

impl BootHook {
    /// The mechanism this host has: a cron daemon with `/etc/cron.d`, else the rc.local generator.
    #[must_use]
    pub fn detect() -> Self {
        let daemon = ["/usr/bin/crond", "/usr/sbin/crond", "/usr/sbin/cron", "/usr/bin/cron"]
            .iter()
            .any(|p| Path::new(p).exists());
        if daemon && Path::new("/etc/cron.d").is_dir() {
            return Self::Cron;
        }
        let generator = ["/usr/lib/systemd/system-generators", "/lib/systemd/system-generators"]
            .iter()
            .any(|d| Path::new(d).join("systemd-rc-local-generator").exists());
        if generator { Self::RcLocal } else { Self::None }
    }

    /// Whether the hook file is installed.
    #[must_use]
    pub fn installed(self) -> bool {
        match self {
            Self::Cron => Path::new(CRON_FILE).exists(),
            Self::RcLocal => std::fs::read_to_string(RC_LOCAL).is_ok_and(|t| t.contains(BEGIN)),
            Self::None => false,
        }
    }
}

/// The cron.d file for `exe` and `config`.
fn cron_entry(exe: &Path, config: &Path) -> String {
    format!(
        "# managed by nie-deploy up: starts the supervisor at boot and whenever it is gone\n\
         @reboot root {0} supervise --config {1} --detach >/dev/null 2>&1\n\
         * * * * * root {0} supervise --config {1} --detach >/dev/null 2>&1\n",
        exe.display(),
        config.display()
    )
}

/// Install the start-at-boot hook this host supports; returns what it did.
fn install_boot_hook(exe: &Path, config: &Path) -> Result<BootHook> {
    let hook = BootHook::detect();
    match hook {
        BootHook::Cron => {
            if write_if_changed(Path::new(CRON_FILE), cron_entry(exe, config).as_bytes(), 0o644)? {
                println!("boot hook {CRON_FILE} written");
            }
        }
        BootHook::RcLocal => {
            let existing = std::fs::read_to_string(RC_LOCAL).ok();
            let wanted = with_hook(existing.as_deref(), &hook_block(exe, config));
            if write_if_changed(Path::new(RC_LOCAL), wanted.as_bytes(), 0o755)? {
                println!("boot hook {RC_LOCAL} written");
            }
        }
        BootHook::None => println!(
            "no boot hook: this host has no cron daemon and no rc.local generator; \
             the supervisor will not return after a reboot"
        ),
    }
    Ok(hook)
}

fn config_text(path: Option<&Path>) -> Result<String> {
    match path {
        None => Ok(EMBEDDED.to_owned()),
        Some(p) if p == Path::new("-") => {
            std::io::read_to_string(std::io::stdin()).context("read the configuration from stdin")
        }
        Some(p) => std::fs::read_to_string(p).with_context(|| format!("read {}", p.display())),
    }
}

/// The boot hook body for `exe` and `config`.
fn hook_block(exe: &Path, config: &Path) -> String {
    format!(
        "{BEGIN}\n{} supervise --config {} --detach\n{END}\n",
        exe.display(),
        config.display()
    )
}

/// `existing` with the managed block set: replaced between its markers, else inserted before a
/// trailing `exit 0`, else appended; a missing file becomes a shell script.
fn with_hook(existing: Option<&str>, block: &str) -> String {
    let Some(text) = existing else {
        return format!("#!/bin/sh\n{block}");
    };
    if let (Some(start), Some(end)) = (text.find(BEGIN), text.find(END)) {
        let end = end + END.len();
        let end = text[end..].strip_prefix('\n').map_or(end, |_| end + 1);
        return format!("{}{block}{}", &text[..start], &text[end..]);
    }
    let body = text.trim_end();
    match body.rsplit_once('\n') {
        Some((head, last)) if last.trim() == "exit 0" => format!("{head}\n{block}exit 0\n"),
        _ if body.trim() == "exit 0" => format!("{block}exit 0\n"),
        _ => format!("{body}\n{block}"),
    }
}

fn exe_replaced(pid: u32) -> bool {
    std::fs::read_link(format!("/proc/{pid}/exe"))
        .is_ok_and(|p| p.to_string_lossy().ends_with(" (deleted)"))
}

fn probe(url: &str, contains: Option<&str>) -> Result<(), String> {
    let answer = nie_health::get(url, Duration::from_secs(5))?;
    if nie_health::passes(&answer, contains) {
        Ok(())
    } else {
        Err(format!("status {}", answer.status))
    }
}

/// Wait until every enabled service that is not a periodic job runs and passes its probe.
fn wait_healthy(config: &Config, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        let report = report(&config.supervisor);
        let pending: Vec<String> = report
            .services
            .iter()
            .filter(|s| !s.periodic && !s.ok)
            .map(|s| format!("{} ({}{})", s.name, s.state, s.health.as_ref().map_or(String::new(), |h| format!(", {}", h.detail))))
            .collect();
        if report.running && pending.is_empty() {
            for s in &report.services {
                println!("ok   {:<16} {:<9} {}", s.name, s.state, s.health.as_ref().map_or("", |h| h.url.as_str()));
            }
            return Ok(());
        }
        if Instant::now() > deadline {
            bail!("not healthy after {timeout:?}: {}", pending.join(", "));
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

fn stop_supervisor(config: &Config, pid: u32) -> Result<()> {
    kill(Pid::from_raw(i32::try_from(pid)?), Signal::SIGTERM)?;
    let deadline = Instant::now() + Duration::from_secs(120);
    while supervise::running_pid(&config.supervisor).is_some() {
        if Instant::now() > deadline {
            bail!("supervisor pid {pid} did not stop within 120s");
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Ok(())
}

/// Install the configuration and the boot hook on this host, start or refresh the supervisor and
/// wait for health. Idempotent: an unchanged host is left running as it is.
pub fn up_local(config_path: Option<&Path>, boot_hook: bool) -> Result<()> {
    if !is_root() {
        bail!("up needs root on the target host");
    }
    let text = config_text(config_path)?;
    let config = Config::parse(&text)?;
    let installed = PathBuf::from(INSTALLED);
    let changed = write_if_changed(&installed, text.as_bytes(), 0o644)?;
    let exe = config.supervisor.install_dir.join("nie-deploy");
    if !exe.exists() {
        bail!("{} is missing: ship the binary first (nie-deploy up --host)", exe.display());
    }
    if boot_hook {
        install_boot_hook(&exe, &installed)?;
    }
    match supervise::running_pid(&config.supervisor) {
        Some(pid) => {
            let services_stale = supervise::read_status(&config.supervisor)
                .is_some_and(|s| s.services.iter().any(|svc| svc.pid.is_some_and(exe_replaced)));
            if changed || exe_replaced(pid) {
                println!("supervisor pid {pid}: configuration or binary changed, restarting it");
                stop_supervisor(&config, pid)?;
                let new = supervise::spawn_detached(&exe, &installed, &config.supervisor)?;
                println!("supervisor started, pid {new}");
            } else if services_stale {
                println!("supervisor pid {pid}: service binaries changed, restarting services");
                kill(Pid::from_raw(i32::try_from(pid)?), Signal::SIGHUP)?;
            } else {
                println!("supervisor pid {pid}: up to date");
            }
        }
        None => {
            let pid = supervise::spawn_detached(&exe, &installed, &config.supervisor)?;
            println!("supervisor started, pid {pid}");
        }
    }
    wait_healthy(&config, Duration::from_secs(config.supervisor.up_timeout_secs))
}

fn shell_word(text: &str) -> Result<&str> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b)) {
        bail!("{text:?}: not a plain path");
    }
    Ok(text)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::hash(bytes).as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

fn remote(host: &str, command: &str, stdin: Vec<u8>) -> Result<nie_ssh::Output> {
    nie_ssh::exec_blocking(host, command, stdin).map_err(|e| anyhow::anyhow!("{host}: {e}"))
}

/// Upload `local` to `dest` on `host` unless the SHA-256 there already matches; atomic rename, so
/// a running binary is replaced without a "text file busy".
fn ship(host: &str, local: &Path, dest: &str) -> Result<bool> {
    let data = std::fs::read(local).with_context(|| format!("read {}", local.display()))?;
    let dest = shell_word(dest)?;
    let there = remote(host, &format!("sha256sum {dest} 2>/dev/null"), Vec::new())?;
    let there = there.stdout_text();
    if there.split_whitespace().next() == Some(sha256_hex(&data).as_str()) {
        return Ok(false);
    }
    let out = remote(
        host,
        &format!("cat > {dest}.new && chmod 0755 {dest}.new && mv -f {dest}.new {dest}"),
        data,
    )?;
    if out.status != Some(0) {
        bail!("upload {dest}: {}", out.stderr_text().trim());
    }
    Ok(true)
}

/// Ship the binaries found in `bin_dir`, then run `nie-deploy up` on `host`.
pub fn up_remote(host: &str, config_path: Option<&Path>, bin_dir: Option<&Path>, boot_hook: bool) -> Result<()> {
    let text = config_text(config_path)?;
    let config = Config::parse(&text)?;
    let bin_dir = match bin_dir {
        Some(d) => d.to_path_buf(),
        None => std::env::current_exe()?.parent().context("executable directory")?.to_path_buf(),
    };
    let install = &config.supervisor.install_dir;
    let mut names: Vec<String> = vec!["nie-deploy".into()];
    for svc in config.enabled() {
        let exec = Path::new(&svc.exec[0]);
        if exec.parent() == Some(install.as_path())
            && let Some(name) = exec.file_name().and_then(|n| n.to_str())
            && !names.iter().any(|n| n == name)
        {
            names.push(name.to_owned());
        }
    }
    for name in &names {
        let local = bin_dir.join(name);
        if !local.is_file() {
            if name == "nie-deploy" {
                bail!("{} not found: pass --bin-dir with a Linux build of nie-deploy", local.display());
            }
            continue;
        }
        let dest = install.join(name);
        let dest = dest.to_str().context("install path")?;
        let sent = ship(host, &local, dest)?;
        println!("{} {name} -> {host}:{dest}", if sent { "sent     " } else { "unchanged" });
    }
    let exe = install.join("nie-deploy");
    let mut command = format!("{} up", shell_word(exe.to_str().context("install path")?)?);
    if !boot_hook {
        command.push_str(" --no-boot-hook");
    }
    let stdin = if config_path.is_some() {
        command.push_str(" --config -");
        text.into_bytes()
    } else {
        Vec::new()
    };
    let out = remote(host, &command, stdin)?;
    print!("{}", out.stdout_text());
    eprint!("{}", out.stderr_text());
    if out.status != Some(0) {
        bail!("{host}: nie-deploy up exited with {:?}", out.status);
    }
    Ok(())
}

/// Live state of one probe.
#[derive(Debug, Serialize)]
pub struct HealthReport {
    /// Probe URL.
    pub url: String,
    /// Whether it answered as required.
    pub ok: bool,
    /// `ok` or why not.
    pub detail: String,
}

/// The boot hook of a host.
#[derive(Debug, Serialize)]
pub struct BootHookReport {
    /// Mechanism available.
    pub kind: BootHook,
    /// Hook file present.
    pub installed: bool,
}

/// One service in a [`Report`].
#[derive(Debug, Serialize)]
pub struct ServiceReport {
    /// Service name.
    pub name: String,
    /// Supervisor state.
    pub state: String,
    /// Process group leader.
    pub pid: Option<u32>,
    /// Seconds since the last start.
    pub uptime_secs: Option<u64>,
    /// Starts since the supervisor began.
    pub starts: u64,
    /// How the last run ended.
    pub last_exit: Option<String>,
    /// A periodic job.
    pub periodic: bool,
    /// Live probe result.
    pub health: Option<HealthReport>,
    /// Running (and healthy where probed), or a periodic job that is not failing.
    pub ok: bool,
    /// The service's first cgroup line, to see it is no systemd unit of nie.
    pub cgroup: Option<String>,
}

/// What `status` prints.
#[derive(Debug, Serialize)]
pub struct Report {
    /// A supervisor holds the lock.
    pub running: bool,
    /// Its pid.
    pub supervisor_pid: Option<u32>,
    /// Unix time it started.
    pub supervisor_started_unix: Option<u64>,
    /// Its cgroup.
    pub supervisor_cgroup: Option<String>,
    /// The start-at-boot mechanism of this host and whether it is installed.
    pub boot_hook: BootHookReport,
    /// Every enabled service.
    pub services: Vec<ServiceReport>,
    /// Every service is ok.
    pub healthy: bool,
}

fn cgroup_of(pid: u32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?.lines().next().map(str::to_owned)
}

/// Combine `status.json` with live probes.
pub fn report(settings: &crate::config::Settings) -> Report {
    let pid = supervise::running_pid(settings);
    let status: Option<Status> = pid.and_then(|_| supervise::read_status(settings));
    let now = now_secs();
    let services: Vec<ServiceReport> = status
        .iter()
        .flat_map(|s| &s.services)
        .map(|svc| {
            let health = svc.health_url.as_ref().map(|url| {
                let result = probe(url, svc.health_contains.as_deref());
                HealthReport { url: url.clone(), ok: result.is_ok(), detail: result.err().unwrap_or_else(|| "ok".into()) }
            });
            let periodic = svc.periodic;
            let running = svc.state == "running";
            let ok = if periodic {
                svc.last_exit.as_deref().is_none_or(|e| e == "exit 0" || e == "restart requested")
            } else {
                running && health.as_ref().is_none_or(|h| h.ok)
            };
            ServiceReport {
                name: svc.name.clone(),
                state: svc.state.clone(),
                pid: svc.pid,
                uptime_secs: svc.started_unix.map(|t| now.saturating_sub(t)).filter(|_| svc.pid.is_some()),
                starts: svc.starts,
                last_exit: svc.last_exit.clone(),
                periodic,
                health,
                ok,
                cgroup: svc.pid.and_then(cgroup_of),
            }
        })
        .collect();
    Report {
        running: pid.is_some(),
        supervisor_pid: pid,
        supervisor_started_unix: status.as_ref().map(|s| s.started_unix),
        supervisor_cgroup: pid.and_then(cgroup_of),
        boot_hook: {
            let kind = BootHook::detect();
            BootHookReport { kind, installed: kind.installed() }
        },
        healthy: pid.is_some() && services.iter().all(|s| s.ok),
        services,
    }
}

/// `nie-deploy status`: this host, or `host` over SSH.
pub fn status(host: Option<&str>, json: bool) -> Result<()> {
    if let Some(host) = host {
        let exe = crate::config::Config::parse(EMBEDDED)?.supervisor.install_dir.join("nie-deploy");
        let mut command = format!("{} status", shell_word(exe.to_str().context("install path")?)?);
        if json {
            command.push_str(" --json");
        }
        let out = remote(host, &command, Vec::new())?;
        print!("{}", out.stdout_text());
        eprint!("{}", out.stderr_text());
        if out.status != Some(0) {
            bail!("{host}: status exited with {:?}", out.status);
        }
        return Ok(());
    }
    let config = match std::fs::read_to_string(INSTALLED) {
        Ok(text) => Config::parse(&text)?,
        Err(_) => Config::parse(EMBEDDED)?,
    };
    let report = report(&config.supervisor);
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        match report.supervisor_pid {
            Some(pid) => println!("supervisor pid {pid}"),
            None => println!("supervisor not running"),
        }
        for s in &report.services {
            println!(
                "{:<16} {:<9} pid {:<7} starts {:<3} {}",
                s.name,
                s.state,
                s.pid.map_or_else(|| "-".into(), |p| p.to_string()),
                s.starts,
                s.health.as_ref().map_or(String::new(), |h| format!("{} {}", h.url, h.detail)),
            );
        }
    }
    if !report.healthy {
        bail!("not healthy");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_is_created_replaced_and_kept_before_exit() {
        let block = hook_block(Path::new("/usr/local/bin/nie-deploy"), Path::new("/etc/nie/supervise.toml"));
        let fresh = with_hook(None, &block);
        assert!(fresh.starts_with("#!/bin/sh\n") && fresh.contains("--detach"));
        assert_eq!(with_hook(Some(&fresh), &block), fresh, "idempotent");
        let other = "#!/bin/sh\nmodprobe x\nexit 0\n";
        let merged = with_hook(Some(other), &block);
        assert!(merged.ends_with("exit 0\n") && merged.contains("modprobe x") && merged.contains(BEGIN));
        assert!(merged.find(BEGIN).unwrap() < merged.rfind("exit 0").unwrap());
        let moved = hook_block(Path::new("/opt/nie-deploy"), Path::new("/etc/nie/supervise.toml"));
        let replaced = with_hook(Some(&merged), &moved);
        assert!(replaced.contains("/opt/nie-deploy") && !replaced.contains("/usr/local/bin/nie-deploy"));
        assert_eq!(replaced.matches(BEGIN).count(), 1);
    }

    #[test]
    fn cron_entry_runs_the_detach_every_minute() {
        let entry = cron_entry(Path::new("/usr/local/bin/nie-deploy"), Path::new("/etc/nie/supervise.toml"));
        assert!(entry.contains("@reboot root /usr/local/bin/nie-deploy supervise --config /etc/nie/supervise.toml --detach"));
        assert!(entry.contains("* * * * * root /usr/local/bin/nie-deploy supervise --config /etc/nie/supervise.toml --detach"));
    }

    #[test]
    fn shell_words_reject_metacharacters() {
        assert!(shell_word("/usr/local/bin/nie-deploy").is_ok());
        assert!(shell_word("/tmp/a b").is_err());
        assert!(shell_word("x;rm").is_err());
    }

    #[test]
    fn sha256_matches_the_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
