//! `nie-deploy supervise`: a process supervisor with no init system underneath.
//!
//! Every service runs in its own session (`setsid`), as its own user with its own rlimits and
//! `no_new_privs`; the supervisor owns the PID tree (the process group of each service is killed as
//! a whole), restarts with exponential backoff, rotates one log file per service by size and
//! restarts a service whose HTTP probe fails. A flock on `<run_dir>/supervisor.lock` keeps one
//! instance per host; `<run_dir>/status.json` is the state `nie-deploy status` reads. SIGTERM and
//! SIGINT stop everything, SIGHUP restarts every service (new binaries are picked up).

use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use nix::fcntl::{Flock, FlockArg};
use nix::sys::resource::{Resource, setrlimit};
use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, killpg, sigaction};
use nix::unistd::{Gid, Pid, Uid, User};
use serde::{Deserialize, Serialize};

use crate::config::{Config, PathSpec, Service, Settings, parse_env_file, parse_mode};
use crate::logs::{RotatingLog, now_secs};

const TICK: Duration = Duration::from_millis(200);

/// Stop and restart requests, set from signal handlers (atomics only) or by tests.
#[derive(Debug, Default)]
pub struct Control {
    stop: AtomicBool,
    reload: AtomicU64,
}

impl Control {
    /// A control with nothing requested.
    #[must_use]
    pub const fn new() -> Self {
        Self { stop: AtomicBool::new(false), reload: AtomicU64::new(0) }
    }

    /// Stop every service and return from [`run`].
    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    /// Restart every service.
    pub fn request_reload(&self) {
        self.reload.fetch_add(1, Ordering::SeqCst);
    }

    fn stopping(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    fn generation(&self) -> u64 {
        self.reload.load(Ordering::SeqCst)
    }
}

/// The control the signal handlers of the supervisor process write to.
pub static SIGNALS: Control = Control::new();

extern "C" fn on_stop(_: nix::libc::c_int) {
    SIGNALS.request_stop();
}

extern "C" fn on_reload(_: nix::libc::c_int) {
    SIGNALS.request_reload();
}

/// Route SIGTERM and SIGINT to stop, SIGHUP to restart, into [`SIGNALS`].
pub fn install_signal_handlers() -> Result<()> {
    let action = |handler| SigAction::new(handler, SaFlags::SA_RESTART, SigSet::empty());
    // SAFETY: the handlers only touch atomics, which is async-signal-safe.
    unsafe {
        sigaction(Signal::SIGTERM, &action(SigHandler::Handler(on_stop)))?;
        sigaction(Signal::SIGINT, &action(SigHandler::Handler(on_stop)))?;
        sigaction(Signal::SIGHUP, &action(SigHandler::Handler(on_reload)))?;
    }
    Ok(())
}

/// State of one service, as written to `status.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    /// Service name.
    pub name: String,
    /// `starting`, `running`, `unhealthy`, `backoff`, `waiting` (periodic job between runs) or `stopped`.
    pub state: String,
    /// Leader of the process group while running.
    pub pid: Option<u32>,
    /// A periodic job (`every_secs`), not a service that must stay up.
    pub periodic: bool,
    /// Starts since the supervisor began.
    pub starts: u64,
    /// Unix time of the last start.
    pub started_unix: Option<u64>,
    /// How the last run ended.
    pub last_exit: Option<String>,
    /// Result of the last health probe.
    pub health: Option<String>,
    /// Probe URL, when the service has one.
    pub health_url: Option<String>,
    /// Text the probe body must contain.
    pub health_contains: Option<String>,
}

/// `status.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    /// Supervisor pid.
    pub pid: u32,
    /// Unix time the supervisor started.
    pub started_unix: u64,
    /// Configuration file it was started with.
    pub config: String,
    /// Every enabled service.
    pub services: Vec<ServiceStatus>,
}

/// `<run_dir>/status.json`.
#[must_use]
pub fn status_path(settings: &Settings) -> PathBuf {
    settings.run_dir.join("status.json")
}

fn lock_path(settings: &Settings) -> PathBuf {
    settings.run_dir.join("supervisor.lock")
}

/// The pid of the running supervisor: the one whose flock is held, else `None`.
pub fn running_pid(settings: &Settings) -> Option<u32> {
    let file = File::open(lock_path(settings)).ok()?;
    match Flock::lock(file, FlockArg::LockSharedNonblock) {
        Ok(_) => None,
        Err(_) => std::fs::read_to_string(settings.run_dir.join("supervisor.pid"))
            .ok()?
            .trim()
            .parse()
            .ok(),
    }
}

/// The last `status.json`, when there is one.
pub fn read_status(settings: &Settings) -> Option<Status> {
    serde_json::from_slice(&std::fs::read(status_path(settings)).ok()?).ok()
}

struct Identity {
    uid: Uid,
    gid: Gid,
    groups: Vec<Gid>,
    home: PathBuf,
    name: String,
}

fn identity(user: Option<&str>, group: Option<&str>) -> Result<Option<Identity>> {
    let Some(name) = user else {
        if group.is_some() {
            bail!("group without user");
        }
        return Ok(None);
    };
    let entry = User::from_name(name)?.with_context(|| format!("unknown user {name}"))?;
    let gid = match group {
        Some(g) => nix::unistd::Group::from_name(g)?
            .with_context(|| format!("unknown group {g}"))?
            .gid,
        None => entry.gid,
    };
    let c_name = std::ffi::CString::new(name)?;
    let groups = nix::unistd::getgrouplist(&c_name, gid).unwrap_or_else(|_| vec![gid]);
    Ok(Some(Identity { uid: entry.uid, gid, groups, home: entry.dir, name: entry.name }))
}

fn limits(svc: &Service) -> Vec<(Resource, u64)> {
    let mut out = Vec::new();
    if let Some(n) = svc.nofile {
        out.push((Resource::RLIMIT_NOFILE, n));
    }
    if let Some(n) = svc.nproc {
        out.push((Resource::RLIMIT_NPROC, n));
    }
    if let Some(mib) = svc.memory_mib {
        out.push((Resource::RLIMIT_AS, mib * 1024 * 1024));
    }
    if svc.no_core {
        out.push((Resource::RLIMIT_CORE, 0));
    }
    out
}

/// Start `svc`: its own session, user, rlimits and `no_new_privs`, stdout and stderr on one pipe
/// read by a thread into `log`.
fn launch(svc: &Service, log: &std::sync::Arc<RotatingLog>) -> Result<(Child, JoinHandle<()>)> {
    let ident = identity(svc.user.as_deref(), svc.group.as_deref())?;
    let (mut reader, writer) = io::pipe()?;
    let mut cmd = Command::new(&svc.exec[0]);
    cmd.args(&svc.exec[1..])
        .env_clear()
        .env("PATH", "/usr/local/bin:/usr/bin:/bin")
        .stdin(Stdio::null())
        .stdout(writer.try_clone()?)
        .stderr(writer);
    if let Some(id) = &ident {
        cmd.env("HOME", &id.home).env("USER", &id.name).env("LOGNAME", &id.name);
    }
    if let Some(dir) = &svc.workdir {
        cmd.current_dir(dir);
    }
    for file in &svc.env_files {
        if let Ok(text) = std::fs::read_to_string(file) {
            cmd.envs(parse_env_file(&text));
        }
    }
    cmd.envs(&svc.env);
    let limits = limits(svc);
    let creds = ident.map(|i| (i.uid, i.gid, i.groups));
    // SAFETY: the closure only issues syscalls (setsid, setrlimit, setgroups, setgid, setuid,
    // prctl) on data prepared before the fork.
    unsafe {
        cmd.pre_exec(move || {
            nix::unistd::setsid().map_err(io::Error::from)?;
            for (resource, value) in &limits {
                setrlimit(*resource, *value, *value).map_err(io::Error::from)?;
            }
            if let Some((uid, gid, groups)) = &creds {
                nix::unistd::setgroups(groups).map_err(io::Error::from)?;
                nix::unistd::setgid(*gid).map_err(io::Error::from)?;
                nix::unistd::setuid(*uid).map_err(io::Error::from)?;
            }
            nix::sys::prctl::set_no_new_privs().map_err(io::Error::from)
        });
    }
    let child = cmd.spawn().with_context(|| format!("start {}", svc.exec[0]))?;
    // the command holds the write ends of the pipe; dropping it lets the reader see EOF
    drop(cmd);
    let log = std::sync::Arc::clone(log);
    let pump = thread::spawn(move || {
        let mut buf = [0u8; 16 * 1024];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || log.write(&buf[..n]).is_err() {
                break;
            }
        }
    });
    Ok((child, pump))
}

fn group_signal(pid: u32, signal: Signal) {
    if let Ok(raw) = i32::try_from(pid) {
        let _ = killpg(Pid::from_raw(raw), signal);
    }
}

/// SIGTERM to the process group, SIGKILL after `timeout`, then whatever is left of the group.
fn terminate(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
    let pid = child.id();
    group_signal(pid, Signal::SIGTERM);
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            _ => {
                group_signal(pid, Signal::SIGKILL);
                break child.wait().ok();
            }
        }
    };
    group_signal(pid, Signal::SIGKILL);
    status
}

fn describe(status: ExitStatus) -> String {
    use std::os::unix::process::ExitStatusExt;
    match (status.code(), status.signal()) {
        (Some(code), _) => format!("exit {code}"),
        (None, Some(sig)) => format!("signal {sig}"),
        _ => status.to_string(),
    }
}

enum Outcome {
    Exited(String),
    Stopped,
    Reloaded,
    Unhealthy(String),
}

struct Ctx<'a> {
    control: &'a Control,
    settings: &'a Settings,
    supervisor_log: &'a RotatingLog,
}

fn update(status: &Mutex<ServiceStatus>, f: impl FnOnce(&mut ServiceStatus)) {
    f(&mut status.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
}

/// Sleep `secs` unless a stop or restart is requested; true when the wait was cut short.
fn nap(ctx: &Ctx<'_>, secs: u64, generation: u64) -> bool {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        if ctx.control.stopping() || ctx.control.generation() != generation {
            return true;
        }
        thread::sleep(TICK);
    }
    false
}

fn probe(svc: &Service) -> Result<(), String> {
    let health = svc.health.as_ref().ok_or("no probe")?;
    let answer = nie_health::get(&health.url, Duration::from_secs(health.timeout_secs))?;
    if nie_health::passes(&answer, health.contains.as_deref()) {
        Ok(())
    } else {
        Err(format!("status {}", answer.status))
    }
}

/// Watch one run until it exits, fails its probe `failures` times, or a stop or restart arrives.
fn watch(
    svc: &Service,
    child: &mut Child,
    ctx: &Ctx<'_>,
    status: &Mutex<ServiceStatus>,
    generation: u64,
) -> Outcome {
    let started = Instant::now();
    let stop_timeout = Duration::from_secs(svc.stop_timeout_secs);
    let mut failures = 0u32;
    let mut next_probe = started;
    loop {
        if ctx.control.stopping() {
            terminate(child, stop_timeout);
            return Outcome::Stopped;
        }
        if ctx.control.generation() != generation {
            terminate(child, stop_timeout);
            return Outcome::Reloaded;
        }
        match child.try_wait() {
            Ok(Some(exit)) => return Outcome::Exited(describe(exit)),
            Ok(None) => {}
            Err(e) => return Outcome::Exited(format!("wait: {e}")),
        }
        if let Some(health) = &svc.health
            && Instant::now() >= next_probe
        {
            next_probe = Instant::now() + Duration::from_secs(health.interval_secs);
            match probe(svc) {
                Ok(()) => {
                    failures = 0;
                    update(status, |s| {
                        s.health = Some("ok".into());
                        "running".clone_into(&mut s.state);
                    });
                }
                Err(reason) => {
                    update(status, |s| s.health = Some(format!("fail: {reason}")));
                    if started.elapsed() >= Duration::from_secs(health.grace_secs) {
                        failures += 1;
                        update(status, |s| "unhealthy".clone_into(&mut s.state));
                        if failures >= health.failures {
                            terminate(child, stop_timeout);
                            return Outcome::Unhealthy(reason);
                        }
                    }
                }
            }
        }
        thread::sleep(TICK);
    }
}

/// The life of one service: start, watch, restart with backoff (or wait `every_secs`).
fn service_loop(svc: &Service, ctx: &Ctx<'_>, status: &Mutex<ServiceStatus>) {
    let log = match RotatingLog::open(
        &ctx.settings.log_dir,
        &svc.name,
        ctx.settings.log_max_bytes,
        ctx.settings.log_keep,
    ) {
        Ok(log) => std::sync::Arc::new(log),
        Err(e) => {
            ctx.supervisor_log.line("supervisor", &format!("{}: log: {e}", svc.name));
            return;
        }
    };
    let say = |message: &str| ctx.supervisor_log.line(&svc.name, message);
    let mut delay = svc.backoff_min_secs;
    while !ctx.control.stopping() {
        let generation = ctx.control.generation();
        update(status, |s| {
            "starting".clone_into(&mut s.state);
            s.pid = None;
        });
        let started = Instant::now();
        let outcome = match launch(svc, &log) {
            Err(e) => Outcome::Exited(format!("spawn: {e:#}")),
            Ok((mut child, pump)) => {
                let pid = child.id();
                say(&format!("started pid {pid}: {}", svc.exec.join(" ")));
                update(status, |s| {
                    s.pid = Some(pid);
                    s.starts += 1;
                    s.started_unix = Some(now_secs());
                    "running".clone_into(&mut s.state);
                    s.health = None;
                });
                let outcome = watch(svc, &mut child, ctx, status, generation);
                group_signal(pid, Signal::SIGKILL);
                for _ in 0..10 {
                    if pump.is_finished() {
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                outcome
            }
        };
        let reason = match &outcome {
            Outcome::Exited(r) => r.clone(),
            Outcome::Stopped => "stopped".into(),
            Outcome::Reloaded => "restart requested".into(),
            Outcome::Unhealthy(r) => format!("health probe failed: {r}"),
        };
        say(&format!("ended: {reason}"));
        update(status, |s| {
            s.pid = None;
            s.last_exit = Some(reason);
            "backoff".clone_into(&mut s.state);
        });
        match outcome {
            Outcome::Stopped => break,
            Outcome::Reloaded => {
                delay = svc.backoff_min_secs;
                continue;
            }
            Outcome::Exited(_) | Outcome::Unhealthy(_) => {}
        }
        if let Some(every) = svc.every_secs {
            update(status, |s| "waiting".clone_into(&mut s.state));
            nap(ctx, every, generation);
            continue;
        }
        if started.elapsed() >= Duration::from_secs(svc.stable_secs) {
            delay = svc.backoff_min_secs;
        }
        say(&format!("restart in {delay}s"));
        nap(ctx, delay, generation);
        delay = (delay * 2).min(svc.backoff_max_secs);
    }
    update(status, |s| {
        "stopped".clone_into(&mut s.state);
        s.pid = None;
    });
}

fn owner(spec: &PathSpec) -> Result<(Option<Uid>, Option<Gid>)> {
    let Some(name) = &spec.user else {
        return Ok((None, None));
    };
    let user = User::from_name(name)?.with_context(|| format!("unknown user {name}"))?;
    let gid = match &spec.group {
        Some(g) => nix::unistd::Group::from_name(g)?
            .with_context(|| format!("unknown group {g}"))?
            .gid,
        None => user.gid,
    };
    Ok((Some(user.uid), Some(gid)))
}

/// Create, re-own and re-mode the configured paths. A failure is logged, never fatal: the service
/// that needs the path reports its own error.
fn prepare_paths(paths: &[PathSpec], log: &RotatingLog) {
    for spec in paths {
        let apply = || -> Result<()> {
            if !spec.path.exists() {
                if spec.create {
                    std::fs::create_dir_all(&spec.path)?;
                } else if spec.optional {
                    return Ok(());
                } else {
                    bail!("missing");
                }
            }
            let (uid, gid) = owner(spec)?;
            if uid.is_some() {
                std::os::unix::fs::chown(&spec.path, uid.map(Uid::as_raw), gid.map(Gid::as_raw))?;
            }
            if let Some(mode) = &spec.mode {
                std::fs::set_permissions(&spec.path, std::fs::Permissions::from_mode(parse_mode(mode)?))?;
            }
            Ok(())
        };
        if let Err(e) = apply() {
            log.line("supervisor", &format!("path {}: {e:#}", spec.path.display()));
        }
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(tmp, path)
}

/// Run the supervisor until `control` asks to stop. Fails when another supervisor holds the lock.
pub fn run(config: &Config, config_path: &Path, control: &Control) -> Result<()> {
    let settings = &config.supervisor;
    std::fs::create_dir_all(&settings.run_dir)
        .with_context(|| format!("create {}", settings.run_dir.display()))?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_path(settings))
        .context("open the supervisor lock")?;
    let _lock = Flock::lock(lock, FlockArg::LockExclusiveNonblock).map_err(|(_, e)| {
        anyhow::anyhow!(
            "another supervisor runs (pid {}): {e}",
            running_pid(settings).map_or_else(|| "?".into(), |p| p.to_string())
        )
    })?;
    std::fs::write(settings.run_dir.join("supervisor.pid"), format!("{}\n", std::process::id()))?;
    let supervisor_log =
        RotatingLog::open(&settings.log_dir, "supervisor", settings.log_max_bytes, settings.log_keep)
            .context("open the supervisor log")?;
    supervisor_log.line("supervisor", &format!("pid {} config {}", std::process::id(), config_path.display()));
    prepare_paths(&config.paths, &supervisor_log);

    let started_unix = now_secs();
    let enabled: Vec<&Service> = config.enabled().collect();
    let states: Vec<Mutex<ServiceStatus>> = enabled
        .iter()
        .map(|svc| {
            Mutex::new(ServiceStatus {
                name: svc.name.clone(),
                state: "starting".into(),
                pid: None,
                periodic: svc.every_secs.is_some(),
                starts: 0,
                started_unix: None,
                last_exit: None,
                health: None,
                health_url: svc.health.as_ref().map(|h| h.url.clone()),
                health_contains: svc.health.as_ref().and_then(|h| h.contains.clone()),
            })
        })
        .collect();
    let ctx = Ctx { control, settings, supervisor_log: &supervisor_log };
    let snapshot = || Status {
        pid: std::process::id(),
        started_unix,
        config: config_path.display().to_string(),
        services: states
            .iter()
            .map(|s| s.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone())
            .collect(),
    };
    let publish = || {
        if let Ok(json) = serde_json::to_vec_pretty(&snapshot()) {
            let _ = write_atomic(&status_path(settings), &json);
        }
    };
    thread::scope(|scope| {
        for (svc, state) in enabled.iter().zip(&states) {
            let ctx = &ctx;
            scope.spawn(move || service_loop(svc, ctx, state));
        }
        while !states.iter().all(|s| {
            s.lock().unwrap_or_else(std::sync::PoisonError::into_inner).state == "stopped"
        }) {
            publish();
            thread::sleep(Duration::from_secs(1));
        }
    });
    publish();
    supervisor_log.line("supervisor", "stopped");
    let _ = std::fs::remove_file(settings.run_dir.join("supervisor.pid"));
    Ok(())
}

/// Start `nie-deploy supervise` detached: a new session, no terminal, output into
/// `<log_dir>/supervisor.out`. Returns once the supervisor holds its lock.
pub fn spawn_detached(exe: &Path, config_path: &Path, settings: &Settings) -> Result<u32> {
    std::fs::create_dir_all(&settings.log_dir)?;
    let out = OpenOptions::new()
        .create(true)
        .append(true)
        .open(settings.log_dir.join("supervisor.out"))?;
    let mut cmd = Command::new(exe);
    cmd.arg("supervise")
        .arg("--config")
        .arg(config_path)
        .stdin(Stdio::null())
        .stdout(out.try_clone()?)
        .stderr(out);
    // SAFETY: setsid is a plain syscall; it detaches the supervisor from the caller's session.
    unsafe {
        cmd.pre_exec(|| nix::unistd::setsid().map(drop).map_err(io::Error::from));
    }
    let child = cmd.spawn().with_context(|| format!("start {}", exe.display()))?;
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Some(pid) = running_pid(settings) {
            return Ok(pid);
        }
        thread::sleep(Duration::from_millis(100));
    }
    bail!("supervisor pid {} did not take its lock; see {}", child.id(), settings.log_dir.join("supervisor.out").display())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(dir: &Path, services: &str) -> Config {
        Config::parse(&format!(
            "[supervisor]\nlog_dir = '{0}/log'\nrun_dir = '{0}/run'\n{services}",
            dir.display()
        ))
        .unwrap()
    }

    fn supervised<R>(config: &Config, body: impl FnOnce(&Control) -> R) -> R {
        let control = Control::new();
        thread::scope(|scope| {
            let handle = scope.spawn(|| run(config, Path::new("test.toml"), &control).unwrap());
            let result = body(&control);
            control.request_stop();
            handle.join().unwrap();
            result
        })
    }

    fn wait_for(what: &str, mut ok: impl FnMut() -> bool) {
        let end = Instant::now() + Duration::from_secs(20);
        while !ok() {
            assert!(Instant::now() < end, "timed out: {what}");
            thread::sleep(Duration::from_millis(50));
        }
    }

    #[test]
    fn restarts_with_backoff_logs_and_rotates() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = config(
            dir.path(),
            "[[service]]\nname = 'flap'\nexec = ['/bin/sh', '-c', 'echo hello-out; echo hello-err >&2; exit 3']\nbackoff_min_secs = 1\nbackoff_max_secs = 2\n",
        );
        supervised(&cfg, |_| {
            let status = || read_status(&cfg.supervisor);
            wait_for("two starts", || {
                status().is_some_and(|s| s.services[0].starts >= 2)
            });
            let s = status().unwrap();
            assert_eq!(s.services[0].last_exit.as_deref(), Some("exit 3"));
            let log = std::fs::read_to_string(dir.path().join("log/flap.log")).unwrap();
            assert!(log.contains("hello-out") && log.contains("hello-err"), "{log}");
            let sup = std::fs::read_to_string(dir.path().join("log/supervisor.log")).unwrap();
            assert!(sup.contains("[flap] ended: exit 3") && sup.contains("restart in 1s"), "{sup}");
        });
    }

    #[test]
    fn stop_kills_the_whole_process_group() {
        let dir = tempfile::tempdir().unwrap();
        let pidfile = dir.path().join("grandchild.pid");
        let cfg = config(
            dir.path(),
            &format!(
                "[[service]]\nname = 'tree'\nexec = ['/bin/sh', '-c', 'sleep 300 & echo $! > {}; wait']\nstop_timeout_secs = 2\n",
                pidfile.display()
            ),
        );
        let grandchild = supervised(&cfg, |_| {
            wait_for("grandchild", || {
                std::fs::read_to_string(&pidfile).is_ok_and(|t| t.trim().parse::<u32>().is_ok())
            });
            std::fs::read_to_string(&pidfile).unwrap().trim().parse::<u32>().unwrap()
        });
        let proc = PathBuf::from(format!("/proc/{grandchild}"));
        wait_for("grandchild gone", || {
            std::fs::read_to_string(proc.join("stat")).map_or(true, |s| s.contains(") Z"))
        });
        assert!(running_pid(&cfg.supervisor).is_none());
    }

    #[test]
    fn a_failing_probe_restarts_the_service_and_one_supervisor_per_host() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = config(
            dir.path(),
            "[[service]]\nname = 'sick'\nexec = ['/bin/sleep', '300']\nstop_timeout_secs = 2\n[service.health]\nurl = 'http://127.0.0.1:1/health'\ninterval_secs = 1\ntimeout_secs = 1\nfailures = 2\ngrace_secs = 0\n",
        );
        supervised(&cfg, |_| {
            wait_for("probe restart", || {
                read_status(&cfg.supervisor).is_some_and(|s| {
                    s.services[0].starts >= 2
                        && s.services[0]
                            .last_exit
                            .as_deref()
                            .is_some_and(|e| e.starts_with("health probe failed"))
                })
            });
            let second = run(&cfg, Path::new("test.toml"), &Control::new());
            assert!(second.unwrap_err().to_string().contains("another supervisor"));
        });
    }

    #[test]
    fn periodic_jobs_run_again_after_their_interval() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = config(
            dir.path(),
            "[[service]]\nname = 'job'\nexec = ['/bin/true']\nevery_secs = 1\n",
        );
        supervised(&cfg, |_| {
            wait_for("three runs", || {
                read_status(&cfg.supervisor).is_some_and(|s| s.services[0].starts >= 3)
            });
        });
    }
}
