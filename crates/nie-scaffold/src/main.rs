//! `nie-new`: the standalone form of `nie new`.

use clap::Parser as _;

fn main() -> anyhow::Result<()> {
    nie_scaffold::run(&nie_scaffold::Args::parse())
}
