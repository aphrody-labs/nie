//! `craft`: validate, plan, simulate, build and launch nie games from their manifests.

use std::path::PathBuf;
use std::process::{Command, ExitCode};

use clap::{Parser, Subcommand};
use nie_craft::{CraftError, Manifest};

#[derive(Parser)]
#[command(name = "craft", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Parse and validate a manifest.
    Check { manifest: PathBuf },
    /// Print the resolved build and launch commands as JSON.
    Plan {
        manifest: PathBuf,
        #[arg(long)]
        release: bool,
    },
    /// Play the manifest headless (the AI controls every player) and print the report as JSON.
    Run {
        manifest: PathBuf,
        /// Cap every match at this many ticks instead of the regulation length.
        #[arg(long)]
        ticks: Option<u32>,
        /// Fail unless the total score equals HOME-AWAY, for example `2-1`.
        #[arg(long)]
        expect: Option<String>,
    },
    /// Build the frontend the manifest names.
    Build {
        manifest: PathBuf,
        #[arg(long)]
        release: bool,
    },
    /// Build and start the frontend the manifest names.
    Launch { manifest: PathBuf },
}

fn main() -> ExitCode {
    match run(Cli::parse().command) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("craft: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Cmd) -> Result<ExitCode, CraftError> {
    match command {
        Cmd::Check { manifest } => {
            let m = Manifest::load(&manifest)?;
            println!(
                "{} {} ok: {:?} on {:?}, {} match(es)",
                m.game.id,
                m.game.version,
                m.game.mode,
                m.game.frontend,
                m.match_count()
            );
        }
        Cmd::Plan { manifest, release } => {
            let m = Manifest::load(&manifest)?;
            print_json(&m.plan(&manifest, release)?);
        }
        Cmd::Run {
            manifest,
            ticks,
            expect,
        } => {
            let m = Manifest::load(&manifest)?;
            let reports = m.play(ticks)?;
            let (home, away) = reports
                .iter()
                .fold((0, 0), |(h, a), r| (h + r.home, a + r.away));
            print_json(&reports);
            if let Some(expected) = expect {
                let actual = format!("{home}-{away}");
                if actual != expected {
                    eprintln!("craft: expected score {expected}, got {actual}");
                    return Ok(ExitCode::FAILURE);
                }
            }
        }
        Cmd::Build { manifest, release } => {
            let plan = Manifest::load(&manifest)?.plan(&manifest, release)?;
            return Ok(spawn(&plan.build));
        }
        Cmd::Launch { manifest } => {
            let plan = Manifest::load(&manifest)?.plan(&manifest, false)?;
            return Ok(spawn(&plan.launch));
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn print_json<T: serde::Serialize>(value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(text) => println!("{text}"),
        Err(error) => eprintln!("craft: {error}"),
    }
}

fn spawn(argv: &[String]) -> ExitCode {
    let program = if argv[0] == "cargo" {
        std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
    } else {
        argv[0].clone()
    };
    match Command::new(program).args(&argv[1..]).status() {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("craft: cannot start `{}`: {error}", argv[0]);
            ExitCode::FAILURE
        }
    }
}
