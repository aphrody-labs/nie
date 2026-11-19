//! `cargo xtask fmt`: rustfmt for Rust, the `taplo` formatter library for every tracked TOML file.

use std::path::Path;

use anyhow::{Context, Result, bail};

use super::host::{Cmd, source_files};
use super::step::{Mode, Runner, Step, repo_root, run_with};

/// Arguments of `cargo xtask fmt`.
#[derive(clap::Args, Debug, Clone, Copy, Default)]
pub struct Args {
    /// Report what would change and fail instead of rewriting files.
    #[arg(long)]
    pub check: bool,
}

/// The steps of `cargo xtask fmt`: both are light, so they always run here.
#[must_use]
pub fn steps(check: bool) -> Vec<Step> {
    let mut rustfmt = Cmd::cargo(["fmt", "--all"]);
    if check {
        rustfmt = rustfmt.args(["--", "--check"]);
    }
    vec![
        Step::local("rustfmt", rustfmt),
        Step::call("toml", move || {
            let root = repo_root();
            let changed = format_tomls(&root, check)?;
            if changed.is_empty() {
                println!("toml: every file is formatted");
                return Ok(());
            }
            for file in &changed {
                println!(
                    "toml: {} {file}",
                    if check {
                        "would reformat"
                    } else {
                        "reformatted"
                    }
                );
            }
            if check {
                bail!(
                    "{} TOML file(s) need formatting, run `just fmt`",
                    changed.len()
                );
            }
            Ok(())
        }),
    ]
}

/// Runs `cargo xtask fmt`.
///
/// # Errors
/// When a formatter fails or, with `--check`, a file would change.
pub fn run(args: Args) -> Result<()> {
    let mut runner = Runner::new(Mode::Local);
    run_with(&mut runner, steps(args.check))
}

/// Formats TOML text with the default taplo options, keeping the line endings of the input.
#[must_use]
pub fn format_toml(text: &str) -> String {
    let options = taplo::formatter::Options {
        crlf: text.contains("\r\n"),
        ..taplo::formatter::Options::default()
    };
    taplo::formatter::format(text, options)
}

/// Formats every tracked `*.toml` under `root`; returns the files that differ from their
/// formatted form (rewritten unless `check`).
///
/// # Errors
/// When git cannot list files or a file cannot be read or written.
pub fn format_tomls(root: &Path, check: bool) -> Result<Vec<String>> {
    let mut changed = Vec::new();
    for file in source_files(root)? {
        if !file.ends_with(".toml") {
            continue;
        }
        let path = root.join(&file);
        let text = std::fs::read_to_string(&path).with_context(|| file.clone())?;
        let formatted = format_toml(&text);
        if formatted != text {
            if !check {
                std::fs::write(&path, formatted).with_context(|| file.clone())?;
            }
            changed.push(file);
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting_is_idempotent_and_normalises_spacing() {
        let once = format_toml("[package]\nname   =   \"x\"\n");
        assert_eq!(once, "[package]\nname = \"x\"\n");
        assert_eq!(format_toml(&once), once);
    }

    #[test]
    fn line_endings_are_kept() {
        let out = format_toml("a   = 1\r\nb = 2\r\n");
        assert!(out.contains("\r\n"));
        assert!(!out.replace("\r\n", "").contains('\n'));
    }

    #[test]
    fn check_step_passes_the_flag() {
        let steps = steps(true);
        let crate::dev::step::Action::Run(cmd) = &steps[0].action else {
            panic!("rustfmt is a command");
        };
        assert_eq!(cmd.display(), "cargo fmt --all -- --check");
        let plain = super::steps(false);
        let crate::dev::step::Action::Run(cmd) = &plain[0].action else {
            panic!("rustfmt is a command");
        };
        assert_eq!(cmd.display(), "cargo fmt --all");
    }

    #[test]
    fn tomls_in_a_temp_repo_are_checked_then_rewritten() {
        let dir = tempfile::tempdir().unwrap();
        let ok = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(dir.path())
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            return;
        }
        std::fs::write(dir.path().join("a.toml"), "x   = 1\n").unwrap();
        std::fs::write(dir.path().join("b.toml"), "y = 2\n").unwrap();
        assert_eq!(format_tomls(dir.path(), true).unwrap(), ["a.toml"]);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.toml")).unwrap(),
            "x   = 1\n"
        );
        assert_eq!(format_tomls(dir.path(), false).unwrap(), ["a.toml"]);
        assert!(format_tomls(dir.path(), true).unwrap().is_empty());
    }
}
