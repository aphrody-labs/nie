//! `cargo xtask remote <recipe> [args...]`: syncs the sources to the build host and runs `just <recipe>`
//! there in a detached tmux session, logged to `<build dir>/log/<session>.log`.

use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};

use super::host::{BuildHost, Cmd, sh_quote};
use crate::util::repo_root;

/// Arguments of `cargo xtask remote`.
#[derive(clap::Args, Debug)]
pub struct Args {
    /// The justfile recipe and its arguments.
    #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
    pub recipe: Vec<String>,
}

/// A tmux session name for a recipe: letters, digits and dashes only.
#[must_use]
pub fn session_name(recipe: &str, epoch: u64) -> String {
    let slug: String = recipe
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    format!("nie-{slug}-{epoch}")
}

/// The shell command that starts the detached session on the host and the log path it writes.
#[must_use]
pub fn launch(host: &BuildHost, recipe: &[String], session: &str) -> (String, String) {
    let log = host.path(&format!("log/{session}.log"));
    let mut just = Cmd::new("just").args(recipe);
    just.env
        .push(("NIE_ON_BUILD_HOST".to_owned(), "1".to_owned()));
    let run = format!(
        "{} > {} 2>&1; echo \"exit $?\" >> {}",
        host.script(&just),
        sh_quote(&log),
        sh_quote(&log)
    );
    let outer = format!(
        "mkdir -p {} && tmux new-session -d -s {} {}",
        sh_quote(&host.path("log")),
        sh_quote(session),
        sh_quote(&run),
    );
    (outer, log)
}

/// Syncs the tree and starts the session.
pub fn run(args: Args) -> Result<()> {
    let root = repo_root();
    let host = BuildHost::from_env(&root);
    if !host.configured() {
        bail!("no build host: set NIE_BUILD_HOST and NIE_BUILD_DIR or fill deploy/build.toml");
    }
    let files = host.sync(&root)?;
    eprintln!("synced {files} files to {}:{}", host.name, host.tree());
    let epoch = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let session = session_name(&args.recipe[0], epoch);
    let (outer, log) = launch(&host, &args.recipe, &session);
    let status = Command::new("ssh")
        .args(host.ssh_args(&outer))
        .stdin(Stdio::null())
        .status()
        .context("starting ssh")?;
    if !status.success() {
        bail!(
            "starting tmux session {session} on {} failed ({status})",
            host.name
        );
    }
    println!("session {session} on {}", host.name);
    println!("log     {log}");
    println!("follow  ssh {} tail -f {log}", host.name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions_are_named_after_the_recipe() {
        assert_eq!(session_name("ci-native", 7), "nie-ci-native-7");
        assert_eq!(session_name("a b/c", 1), "nie-a-b-c-1");
    }

    #[test]
    fn the_launch_runs_just_in_the_tree_and_logs() {
        let host = BuildHost::from_vars(Some("box".into()), Some("/work/nie".into()));
        let (outer, log) = launch(
            &host,
            &["kernel-cycle".to_owned(), "x y".to_owned()],
            "nie-s-1",
        );
        assert_eq!(log, "/work/nie/log/nie-s-1.log");
        assert!(outer.starts_with("mkdir -p /work/nie/log && tmux new-session -d -s nie-s-1 "));
        assert!(outer.contains("NIE_ON_BUILD_HOST=1"));
        assert!(outer.contains("just kernel-cycle"));
        assert!(outer.contains("exit"));
    }
}
