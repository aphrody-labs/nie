//! `cargo xtask lint`: clippy, unused dependencies and secrets. The coverage lints that need the game
//! (`nie vfs lint`, `nie exe lint`) live in the `nie` CLI.

use anyhow::Result;
use clap::ValueEnum;

use super::host::Cmd;
use super::step::{Action, Exec, Runner, Step, run_with};

/// What to lint.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LintTarget {
    /// clippy, shear and secrets.
    #[default]
    All,
    /// `cargo clippy --workspace --all-targets -- -D warnings`.
    Clippy,
    /// `cargo shear`: unused dependencies.
    Shear,
    /// `gitleaks detect`.
    Secrets,
}

/// Arguments of `cargo xtask lint`.
#[derive(clap::Args, Debug, Clone)]
pub struct Args {
    /// What to lint.
    #[arg(value_enum, default_value_t = LintTarget::All)]
    pub target: LintTarget,
    #[command(flatten)]
    pub exec: Exec,
}
/// `cargo clippy` as CI runs it.
#[must_use]
pub fn clippy_step() -> Step {
    Step::build(
        "clippy",
        Cmd::cargo([
            "clippy",
            "--locked",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ])
        .target_dir("target-clippy"),
    )
}

/// `cargo shear`.
#[must_use]
pub fn shear_step() -> Step {
    Step::build("shear", Cmd::cargo(["shear"]))
}

/// `gitleaks detect`: the git history here, the synced files on the build host (no `.git` there).
#[must_use]
pub fn secrets_step() -> Step {
    let flags = ["detect", "--no-banner", "--redact"];
    Step::build(
        "secrets",
        Cmd::new("gitleaks")
            .args(flags)
            .args(["--no-git", "--source", "."]),
    )
    .or_locally(Action::Run(Cmd::new("gitleaks").args(flags)))
}

/// The steps for a target.
#[must_use]
pub fn steps(args: &Args) -> Vec<Step> {
    match args.target {
        LintTarget::All => vec![clippy_step(), shear_step(), secrets_step()],
        LintTarget::Clippy => vec![clippy_step()],
        LintTarget::Shear => vec![shear_step()],
        LintTarget::Secrets => vec![secrets_step()],
    }
}

/// Runs `cargo xtask lint`.
///
/// # Errors
/// When a step fails.
pub fn run(args: Args) -> Result<()> {
    let mut runner = Runner::new(args.exec.mode());
    run_with(&mut runner, steps(&args))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Wrapper {
        #[command(flatten)]
        args: Args,
    }

    fn parse(args: &[&str]) -> Args {
        Wrapper::try_parse_from(std::iter::once("lint").chain(args.iter().copied()))
            .unwrap()
            .args
    }

    #[test]
    fn the_default_target_is_all() {
        let args = parse(&[]);
        assert_eq!(args.target, LintTarget::All);
        let names: Vec<_> = steps(&args).into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["clippy", "shear", "secrets"]);
    }

    #[test]
    fn local_and_remote_conflict() {
        assert!(Wrapper::try_parse_from(["lint", "--local", "--remote"]).is_err());
    }

    #[test]
    fn clippy_matches_the_ci_job() {
        let Action::Run(cmd) = clippy_step().action else {
            panic!("a command");
        };
        assert_eq!(
            cmd.display(),
            "cargo clippy --locked --workspace --all-targets -- -D warnings"
        );
        assert_eq!(cmd.target, "target-clippy");
    }

    #[test]
    fn secrets_scan_files_remotely_and_history_locally() {
        let step = secrets_step();
        let Action::Run(remote) = &step.action else {
            panic!("a command");
        };
        assert!(remote.args.contains(&"--no-git".to_owned()));
        let Some(Action::Run(local)) = &step.local else {
            panic!("a local variant");
        };
        assert!(!local.args.contains(&"--no-git".to_owned()));
    }
}
