// SPDX-License-Identifier: Apache-2.0
//! `rgd`: deploys one service of infra/services.toml to the host of its role (infra/hosts.toml).
//!
//! deploy = `git archive <rev>` -> zstd -> one ssh stream unpacked into `<srv_root>/deploy/<service>/releases/<rev>`,
//! the service `prepare` command (install, build) run inside the release, then, under the `deploy.lock` flock:
//! unit files installed, the service's own `current` swapped atomically,
//! the unit restarted and its health URL probed; a failed probe swaps `current` back and restarts.
//! Nothing runs on the build side besides git and ssh; the host needs tar, zstd, flock, curl, systemd.

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Parser)]
#[command(name = "rgd", version, about = "Rose Griffon deploy")]
struct Cli {
    /// Directory holding hosts.toml and services.toml.
    #[arg(long, default_value = "infra", global = true)]
    infra: PathBuf,
    /// Run on the target host itself: remote steps go to a local bash instead of ssh (no upload over the network).
    #[arg(long, global = true)]
    here: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Print what a deploy would do, change nothing.
    Plan { service: String, #[arg(long, default_value = "HEAD")] rev: String },
    /// Upload the revision, switch `current`, restart, probe, roll back on failure.
    Deploy { service: String, #[arg(long, default_value = "HEAD")] rev: String },
    /// Switch `current` back to the previous release and restart.
    Rollback { service: String },
    /// Show the current and available releases.
    Status { service: String },
}

#[derive(Deserialize)]
struct Hosts {
    #[serde(default)]
    roles: BTreeMap<String, Role>,
    hosts: BTreeMap<String, Host>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Role {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize)]
struct Host {
    ssh: String,
    srv_root: String,
}

#[derive(Deserialize)]
struct Services {
    services: BTreeMap<String, Service>,
}

#[derive(Deserialize)]
struct Service {
    role: Option<String>,
    unit: Option<String>,
    port: Option<u16>,
    health: Option<String>,
    /// Shell command run inside the unpacked release before the switch (dependencies, build).
    prepare: Option<String>,
}

/// Everything a deploy needs, resolved from the two manifests.
#[derive(Debug, PartialEq)]
struct Target {
    ssh: String,
    /// `<srv_root>/deploy/<service>`: releases, `current` and `previous` of this service only.
    root: String,
    unit: String,
    probe: Option<String>,
    prepare: Option<String>,
    /// Steps run through a local bash (rgd runs on the target host).
    here: bool,
}

fn load<T: for<'de> Deserialize<'de>>(p: &Path) -> Result<T> {
    let text = std::fs::read_to_string(p).with_context(|| format!("read {}", p.display()))?;
    toml::from_str(&text).with_context(|| format!("parse {}", p.display()))
}

fn resolve(hosts: &Hosts, services: &Services, name: &str) -> Result<Target> {
    let svc = services.services.get(name).with_context(|| format!("unknown service {name}"))?;
    let role = svc.role.as_deref().unwrap_or("prod");
    let key = match hosts.roles.get(role) {
        Some(Role::One(k)) => k.clone(),
        Some(Role::Many(v)) => v.first().cloned().with_context(|| format!("role {role} has no host"))?,
        None => bail!("role {role} is not in hosts.toml"),
    };
    let h = hosts.hosts.get(&key).with_context(|| format!("host {key} is not in hosts.toml"))?;
    let probe = match (svc.port, svc.health.as_deref()) {
        (Some(p), Some(path)) => Some(format!("http://127.0.0.1:{p}{path}")),
        _ => None,
    };
    Ok(Target {
        ssh: h.ssh.clone(),
        root: format!("{}/deploy/{name}", h.srv_root.trim_end_matches('/')),
        unit: svc.unit.clone().unwrap_or_else(|| format!("rg-{name}")),
        probe,
        prepare: svc.prepare.clone(),
        here: false,
    })
}

fn sq(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn ssh_cmd(t: &Target) -> Command {
    if t.here {
        let mut c = Command::new("bash");
        c.arg("-c");
        return c;
    }
    let mut c = Command::new("ssh");
    c.args(["-o", "BatchMode=yes"]);
    if !cfg!(windows) {
        c.args(["-o", "ControlMaster=auto", "-o", "ControlPath=~/.ssh/rgd-%C", "-o", "ControlPersist=60"]);
    }
    c.arg(&t.ssh);
    c
}

/// Remote side of the upload: unpack into a temp dir, then rename (a release dir is complete or absent).
fn unpack_script(t: &Target, rev: &str) -> String {
    let d = format!("{}/releases/{rev}", t.root);
    let prepare = match &t.prepare {
        Some(cmd) => format!("; (cd {d}.tmp && {cmd})", d = sq(&d)),
        None => String::new(),
    };
    format!(
        "set -e; mkdir -p {r}/releases; rm -rf {d}.tmp; mkdir -p {d}.tmp; zstd -dc | tar -x -C {d}.tmp{prepare}; rm -rf {d}; mv {d}.tmp {d}",
        r = sq(&t.root),
        d = sq(&d)
    )
}

/// Remote side of the switch. `rev` is None for a rollback (previous release from the `previous` link).
fn switch_script(t: &Target, rev: Option<&str>, tries: u32) -> String {
    let r = sq(&t.root);
    let unit = sq(&t.unit);
    let probe = match &t.probe {
        Some(u) => format!(
            "for i in $(seq 1 {tries}); do curl -fsS -m 2 {u} >/dev/null 2>&1 && ok=1 && break; sleep 1; done",
            u = sq(u)
        ),
        None => "systemctl is-active --quiet \"$U\" && ok=1".into(),
    };
    let pick = match rev {
        Some(v) => format!("new=releases/{}", v.replace('\'', "")),
        None => "new=$(readlink previous) || { echo no-previous-release; exit 1; }".into(),
    };
    format!(
        r#"set -e; cd {r}; U={unit}
exec 9>deploy.lock; flock -w 120 9
cur=$(readlink current || true)
{pick}
[ -d "$new" ] || {{ echo "missing $new"; exit 1; }}
for f in "$new"/infra/systemd/"$U".service "$new"/infra/systemd/"$U".timer; do [ -f "$f" ] && install -m 0644 "$f" /etc/systemd/system/; done
systemctl daemon-reload
ln -sfn "$new" current.new; mv -T current.new current
systemctl restart "$U"
ok=0; {probe}
if [ "$ok" = 1 ]; then
  [ -n "$cur" ] && [ "$cur" != "$new" ] && ln -sfn "$cur" previous
  ls -1dt releases/* | grep -v -x -e "$new" -e "${{cur:-none}}" | tail -n +3 | xargs -r rm -rf
  echo "ok $new"; exit 0
fi
if [ -n "$cur" ]; then ln -sfn "$cur" current.new; mv -T current.new current; systemctl restart "$U"; else rm -f current; systemctl stop "$U"; fi
echo "FAILED $new, back to ${{cur:-nothing}}"; exit 1"#
    )
}

fn run_remote(t: &Target, script: &str) -> Result<()> {
    let st = ssh_cmd(t).arg(script).status().context("ssh")?;
    if !st.success() {
        bail!("remote step failed ({st})");
    }
    Ok(())
}

fn short_rev(rev: &str) -> Result<String> {
    let o = Command::new("git").args(["rev-parse", "--short=12", rev]).output().context("git")?;
    if !o.status.success() {
        bail!("git rev-parse {rev}: {}", String::from_utf8_lossy(&o.stderr).trim());
    }
    Ok(String::from_utf8(o.stdout)?.trim().to_string())
}

fn upload(t: &Target, rev: &str, full_rev: &str) -> Result<()> {
    let mut git = Command::new("git")
        .args(["archive", "--format=tar", full_rev])
        .stdout(Stdio::piped())
        .spawn()
        .context("git archive")?;
    let mut remote = ssh_cmd(t).arg(unpack_script(t, rev)).stdin(Stdio::piped()).spawn().context("ssh")?;
    {
        let mut enc = zstd::stream::Encoder::new(remote.stdin.take().context("ssh stdin")?, 9)?;
        enc.multithread(std::thread::available_parallelism().map_or(1, |n| n.get() as u32))?;
        std::io::copy(&mut git.stdout.take().context("git stdout")?, &mut enc)?;
        enc.finish()?.flush()?;
    }
    if !git.wait()?.success() || !remote.wait()?.success() {
        bail!("upload of {rev} failed");
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let hosts: Hosts = load(&cli.infra.join("hosts.toml"))?;
    let services: Services = load(&cli.infra.join("services.toml"))?;
    let resolve = |h: &Hosts, s: &Services, n: &str| resolve(h, s, n).map(|t| Target { here: cli.here, ..t });
    match cli.cmd {
        Cmd::Plan { service, rev } => {
            let t = resolve(&hosts, &services, &service)?;
            let r = short_rev(&rev)?;
            println!("service {service} -> {} (unit {})", t.ssh, t.unit);
            println!("1. git archive {r} | zstd | ssh {} unpack to {}/releases/{r}", t.ssh, t.root);
            if let Some(p) = &t.prepare {
                println!("   then inside the release: {p}");
            }
            println!("2. flock deploy.lock, install infra/systemd/{}.service, swap current, restart", t.unit);
            match &t.probe {
                Some(u) => println!("3. probe {u} up to 30 s, else swap back and restart"),
                None => println!("3. no health URL: require the unit to be active"),
            }
        }
        Cmd::Deploy { service, rev } => {
            let t = resolve(&hosts, &services, &service)?;
            let r = short_rev(&rev)?;
            let started = std::time::Instant::now();
            upload(&t, &r, &rev)?;
            println!("uploaded {r} in {:.1}s", started.elapsed().as_secs_f32());
            run_remote(&t, &switch_script(&t, Some(&r), 30))?;
            println!("deployed {service} {r} in {:.1}s", started.elapsed().as_secs_f32());
        }
        Cmd::Rollback { service } => {
            let t = resolve(&hosts, &services, &service)?;
            run_remote(&t, &switch_script(&t, None, 30))?;
        }
        Cmd::Status { service } => {
            let t = resolve(&hosts, &services, &service)?;
            let r = sq(&t.root);
            run_remote(&t, &format!("cd {r}; echo current=$(readlink current); echo previous=$(readlink previous); ls -1t releases"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> (Hosts, Services) {
        let h: Hosts = toml::from_str(
            r#"[roles]
prod = "prd"
build = ["a", "b"]
[hosts.prd]
ssh = "prd"
srv_root = "/srv/rosegriffon"
[hosts.a]
ssh = "fr1"
srv_root = "/srv/x""#,
        )
        .unwrap();
        let s: Services = toml::from_str(
            r#"[services.core]
role = "prod"
port = 8810
health = "/health"
prepare = "bun install --frozen-lockfile"
[services.job]
role = "build""#,
        )
        .unwrap();
        (h, s)
    }

    #[test]
    fn resolves_role_host_unit_and_probe() {
        let (h, s) = fixtures();
        let t = resolve(&h, &s, "core").unwrap();
        assert_eq!(t.ssh, "prd");
        assert_eq!(t.unit, "rg-core");
        assert_eq!(t.root, "/srv/rosegriffon/deploy/core");
        assert!(unpack_script(&t, "abc").contains("&& bun install --frozen-lockfile)"));
        assert!(resolve(&h, &s, "job").unwrap().prepare.is_none());
        assert_eq!(t.probe.as_deref(), Some("http://127.0.0.1:8810/health"));
        assert_eq!(resolve(&h, &s, "job").unwrap().ssh, "fr1");
        assert!(resolve(&h, &s, "nope").is_err());
    }

    #[test]
    fn switch_script_guards_the_lock_the_probe_and_the_rollback() {
        let (h, s) = fixtures();
        let t = resolve(&h, &s, "core").unwrap();
        let sh = switch_script(&t, Some("abc123"), 30);
        assert!(sh.contains("flock -w 120 9"));
        assert!(sh.contains("mv -T current.new current"));
        assert!(sh.contains("curl -fsS -m 2 'http://127.0.0.1:8810/health'"));
        assert!(sh.contains("FAILED"));
        assert!(sh.contains("systemctl stop \"$U\""), "a first release that fails must not leave the unit restarting");
        assert!(switch_script(&t, None, 5).contains("readlink previous"));
    }

    #[test]
    fn switch_script_is_valid_bash() {
        // On Windows `bash` can be the WSL stub; check syntax only where a working bash exists.
        if !Command::new(std::env::var("RGD_BASH").unwrap_or_else(|_| "bash".into())).args(["-c", "true"]).output().is_ok_and(|o| o.status.success()) {
            eprintln!("no working bash, syntax check skipped");
            return;
        }
        let (h, s) = fixtures();
        let t = resolve(&h, &s, "core").unwrap();
        for sh in [switch_script(&t, Some("abc123"), 3), switch_script(&t, None, 3), unpack_script(&t, "abc123")] {
            let f = std::env::temp_dir().join("rgd-test.sh");
            std::fs::write(&f, &sh).unwrap();
            let mut c = Command::new(std::env::var("RGD_BASH").unwrap_or_else(|_| "bash".into())).arg("-n").arg(&f).output().unwrap();
            assert!(c.status.success(), "{:?} {} {}", c.status, String::from_utf8_lossy(&c.stdout), String::from_utf8_lossy(&std::mem::take(&mut c.stderr)));
        }
    }

    #[test]
    fn quoting_survives_a_quote() {
        assert_eq!(sq("a'b"), "'a'\\''b'");
    }
}
