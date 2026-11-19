//! `cargo xtask <sub>`: the one developer and CI entrypoint behind the `justfile`.
//!
//! Every subcommand is one module with `pub struct Args` (clap) and `pub fn run(Args) -> Result<()>`.
//! Heavy dispatch to the build host (`deploy/build.toml`) lives in `dev`; the rest runs where it is called.

mod app_pack;
mod bindings;
mod claude_stop;
mod dev;
mod kernel;
mod libernt;
mod map;
mod os;
mod release;
mod repro;
mod skill;
mod util;
mod web;
mod zed_branch;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask", version, about = "nie developer and CI tasks")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Write MAP.tsv, or fail when it is stale (`--check`).
    Map(map::Args),
    /// Regenerate the `nie` branch of the aphrody-labs/zed fork.
    ZedBranch(zed_branch::Args),
    /// Build the browser app bundle that `nie-app` embeds.
    AppPack(app_pack::Args),
    /// Kernel.org mainline pipeline: watch, build, boot-test, cycle, promote.
    Kernel(kernel::Args),
    /// Asterinas OS image: initramfs, QEMU arguments, boot.
    Os(os::Args),
    /// PE import closure of the launch chain over a LiberNT build tree.
    LiberntClosure(libernt::Args),
    /// Smoke test of every generated binding against the cdylib.
    BindingsSmoke(bindings::Args),
    /// Browser bundle of the GPUI apps: pack, serve, shot.
    Web(web::Args),
    /// Claude Code Stop hook: keep the session going while the plan has open tasks.
    ClaudeStop(claude_stop::Args),
    /// clippy, shear and gitleaks.
    Lint(dev::lint::Args),
    /// rustfmt and taplo.
    Fmt(dev::fmt::Args),
    /// The CI test job, the whole workspace, or coverage.
    Test(dev::test::Args),
    /// Release, musl, Windows, wasm32 builds and the crlib libraries.
    Compile(dev::compile::Args),
    /// fmt check, cargo check, bevy feature check, drift tests.
    Check(dev::check::Args),
    /// Run a justfile recipe on the build host in a detached, logged tmux session.
    Remote(dev::remote::Args),
    /// Build twice in two directories and compare the BLAKE3 of every artifact.
    Repro(repro::Args),
    /// Release job helpers: version and tag agreement, archive.
    Release(release::Args),
    /// Write the command, MCP tool and plugin table of the nie skill, or fail when it is stale (`--check`).
    Skill(skill::Args),
}

fn main() -> anyhow::Result<()> {
    nie_tls::install();
    match Cli::parse().cmd {
        Cmd::Map(a) => map::run(a),
        Cmd::ZedBranch(a) => zed_branch::run(a),
        Cmd::AppPack(a) => app_pack::run(a),
        Cmd::Kernel(a) => kernel::run(a),
        Cmd::Os(a) => os::run(a),
        Cmd::LiberntClosure(a) => libernt::run(a),
        Cmd::BindingsSmoke(a) => bindings::run(a),
        Cmd::Web(a) => web::run(a),
        Cmd::ClaudeStop(a) => claude_stop::run(a),
        Cmd::Lint(a) => dev::lint::run(a),
        Cmd::Fmt(a) => dev::fmt::run(a),
        Cmd::Test(a) => dev::test::run(a),
        Cmd::Compile(a) => dev::compile::run(a),
        Cmd::Check(a) => dev::check::run(a),
        Cmd::Remote(a) => dev::remote::run(a),
        Cmd::Repro(a) => repro::run(a),
        Cmd::Release(a) => release::run(a),
        Cmd::Skill(a) => skill::run(a),
    }
}
