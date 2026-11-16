//! `nie-studio`: native manager and editor of craft manifests.
//!
//! Lists the manifests of a games directory, edits every field of the typed manifest, validates,
//! saves back to TOML and plays the match headless to preview the result.

mod studio;

use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Directory holding the game manifests.
    #[arg(long, default_value = "games")]
    games: PathBuf,
    /// Manifest to open first.
    #[arg(long)]
    manifest: Option<PathBuf>,
}

fn main() -> eframe::Result {
    let args = Args::parse();
    eframe::run_native(
        "nie studio",
        eframe::NativeOptions::default(),
        Box::new(move |_| Ok(Box::new(studio::Studio::new(args.games, args.manifest)))),
    )
}
