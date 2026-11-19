//! `nie-bindgen`: the standalone form of `nie bindgen`.

use clap::Parser as _;

fn main() -> anyhow::Result<()> {
    nie_bindgen::run(&nie_bindgen::Args::parse())
}
