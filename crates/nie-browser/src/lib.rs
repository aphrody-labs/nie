//! nie browser: the kitesurf engine crates of `vendor/kitesurf` behind a Rust host.
//!
//! `nie-browser deps` compares every vendored engine crate with its newest crates.io release and
//! writes the report committed as `docs/specs/kitesurf-deps.md`.

pub mod deps;
pub mod tar;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "nie-browser", version, about = "nie browser host and engine-crate tooling")]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Report each vendored engine crate against the newest crates.io release.
    Deps(DepsArgs),
}

#[derive(clap::Args, Debug)]
pub struct DepsArgs {
    /// The kitesurf checkout (the submodule).
    #[arg(long, default_value = "vendor/kitesurf")]
    pub root: PathBuf,
    /// Write the Markdown report here instead of stdout.
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Emit JSON instead of Markdown.
    #[arg(long)]
    pub json: bool,
    /// Upstream commit to record; default: the checkout's HEAD.
    #[arg(long)]
    pub commit: Option<String>,
}

/// Run one subcommand.
///
/// # Errors
/// Any failure reading the checkout or querying crates.io.
pub fn run(args: &Args) -> Result<()> {
    match &args.command {
        Command::Deps(d) => deps::run(d),
    }
}
