//! `cargo xtask check`: formatting, type checking of the whole workspace, the Bevy feature build and the
//! generated-file drift tests.

use anyhow::Result;

use super::fmt;
use super::host::Cmd;
use super::step::{Exec, Runner, Step, run_with};

/// Arguments of `cargo xtask check`.
#[derive(clap::Args, Debug, Clone, Copy, Default)]
pub struct Args {
    #[command(flatten)]
    pub exec: Exec,
}

/// The steps of `cargo xtask check`.
#[must_use]
pub fn steps() -> Vec<Step> {
    let mut steps = fmt::steps(true);
    steps.push(Step::build(
        "check",
        Cmd::cargo(["check", "--locked", "--workspace", "--all-targets"]),
    ));
    steps.push(Step::build(
        "check-bevy",
        Cmd::cargo([
            "check",
            "--locked",
            "-p",
            "nie-bevy",
            "--features",
            "bevy",
            "--all-targets",
        ]),
    ));
    steps.push(Step::build(
        "drift",
        Cmd::cargo([
            "test",
            "--locked",
            "-p",
            "nie-shell",
            "-p",
            "nie-abi",
            "-p",
            "nie-ffi",
        ]),
    ));
    steps
}

/// Runs `cargo xtask check`.
///
/// # Errors
/// When a step fails.
pub fn run(args: Args) -> Result<()> {
    let mut runner = Runner::new(args.exec.mode());
    run_with(&mut runner, steps())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_runs_format_check_types_features_and_drift() {
        let names: Vec<_> = steps().into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["rustfmt", "toml", "check", "check-bevy", "drift"]);
    }
}
