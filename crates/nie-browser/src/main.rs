//! `nie-browser`: the native host of the kitesurf engine crates.

use clap::Parser as _;

fn main() -> anyhow::Result<()> {
    nie_tls::install();
    nie_browser::run(&nie_browser::Args::parse())
}
