//! `nie fm`: the terminal explorer of the game VFS. The command only carries the arguments; the
//! explorer is the `nie-tui` plugin over the `nie_explore::fm` model.

use std::path::PathBuf;

/// Arguments of `nie fm`.
#[derive(clap::Args, Debug)]
pub struct FmArgs {
    /// Where the game lives (`ssh://host/path`, `rclone://remote/path`, a local path). Defaults to
    /// NIE_GAME_SOURCE, then the saved source and automatic discovery.
    #[arg(long)]
    pub source: Option<String>,
    /// Keymap merged over the default one (also NIE_FM_KEYMAP).
    #[arg(long)]
    pub keymap: Option<PathBuf>,
    /// Graphics protocol: kitty, sixel, iterm2 or halfblocks. Detected from the terminal by default.
    #[arg(long, value_parser = ["kitty", "sixel", "iterm2", "halfblocks"])]
    pub protocol: Option<String>,
    /// Do not use the BLAKE3 thumbnail cache of the user cache directory.
    #[arg(long)]
    pub no_cache: bool,
    /// Open this VFS directory first.
    #[arg(long)]
    pub cd: Option<String>,
    /// Put the cursor on this VFS file first.
    #[arg(long)]
    pub select: Option<String>,
    /// Render one frame of WIDTHxHEIGHT cells as text on standard output, without a terminal.
    #[arg(long, value_name = "WIDTHxHEIGHT")]
    pub snapshot: Option<String>,
    /// Write the preview timings as JSON to this file when the session ends.
    #[arg(long)]
    pub metrics: Option<PathBuf>,
}

/// Runs the explorer.
pub fn run(args: FmArgs) -> anyhow::Result<()> {
    nie_tui::app::run_fm(&nie_tui::app::FmOptions {
        source: args.source,
        keymap: args.keymap,
        protocol: args.protocol,
        no_cache: args.no_cache,
        cd: args.cd,
        select: args.select,
        snapshot: args.snapshot,
        metrics: args.metrics,
    })
}
