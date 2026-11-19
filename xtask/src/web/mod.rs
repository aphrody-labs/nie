//! `web`: the browser build of the GPUI apps. `pack` makes the content-hashed bundle that `nie serve`
//! embeds, `serve` hosts a bundle next to a running `nie serve` for local proofs, `shot` loads a page in
//! headless Chrome and reports `navigator.gpu`, console errors and a screenshot.

mod cdp;
mod pack;
mod serve;
mod shot;

use anyhow::Result;

/// Arguments of `web`.
#[derive(clap::Args, Debug)]
pub struct Args {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(clap::Subcommand, Debug)]
pub enum Cmd {
    /// Build nie-apps for wasm32 into a content-hashed bundle with brotli copies and manifest.json.
    Pack(pack::Args),
    /// Serve a bundle and forward /v1/* and /tex/* to a running `nie serve`.
    Serve(serve::Args),
    /// Headless Chrome proof of a page: navigator.gpu, console errors, screenshot.
    Shot(shot::Args),
}

pub fn run(args: Args) -> Result<()> {
    match args.cmd {
        Cmd::Pack(a) => pack::run(&a),
        Cmd::Serve(a) => serve::run(&a),
        Cmd::Shot(a) => shot::run(&a),
    }
}
