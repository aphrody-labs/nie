//! Graceful reload by pingora's upgrade mechanism: start a new instance with `--upgrade` (it
//! takes the listening sockets from the running one over the upgrade socket), then ask the old
//! instance to finish its requests and exit with SIGQUIT.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;

use crate::config::Config;
use crate::serve::server_conf;

fn read_pid(path: &Path) -> Option<i32> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn alive(pid: i32) -> bool {
    kill(Pid::from_raw(pid), None).is_ok()
}

/// Reload the instance whose pid file is `[server] pid_file`. Fails, leaving the old instance
/// running, when the new one does not come up within `timeout`.
pub fn run(config: &Config, config_path: Option<&Path>, timeout: Duration) -> Result<()> {
    let conf = server_conf(config)?;
    let pid_file = Path::new(&conf.pid_file);
    let old = read_pid(pid_file).with_context(|| format!("pid file {}", pid_file.display()))?;
    if !alive(old) {
        bail!("instance {old} from {} is not running", pid_file.display());
    }
    let exe = std::env::current_exe().context("current executable")?;
    let mut cmd = Command::new(exe);
    cmd.arg("serve");
    if let Some(path) = config_path {
        cmd.arg("--config").arg(path);
    }
    cmd.args(["--upgrade", "--daemon"]);
    let status = cmd.status().context("start the new instance")?;
    if !status.success() {
        bail!("new instance exited with {status}");
    }
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(new) = read_pid(pid_file)
            && new != old
            && alive(new)
        {
            break;
        }
        if Instant::now() > deadline {
            bail!(
                "new instance did not write {} within {timeout:?}; old instance {old} still serves",
                pid_file.display()
            );
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    kill(Pid::from_raw(old), Signal::SIGQUIT).context("SIGQUIT to the old instance")?;
    Ok(())
}
