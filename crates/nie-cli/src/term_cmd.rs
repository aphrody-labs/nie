//! `nie term`: the GPU terminal window that hosts `nie fm` or any command. The command only carries
//! the arguments; the window is the `nie-tui` plugin (feature `term`) over the `nie_explore::term`
//! model.

use std::path::PathBuf;

/// Arguments of `nie term`.
#[derive(clap::Args, Debug)]
pub struct TermArgs {
    /// Columns of the terminal.
    #[arg(long, default_value_t = 120)]
    pub cols: u16,
    /// Rows of the terminal.
    #[arg(long, default_value_t = 36)]
    pub rows: u16,
    /// Font size in points.
    #[arg(long, default_value_t = 16.0)]
    pub font_size: f32,
    /// Render one frame offscreen to this PNG file and exit, for hosts without a display.
    #[arg(long, value_name = "PNG")]
    pub screenshot: Option<PathBuf>,
    /// Seconds the command runs before the screenshot is taken.
    #[arg(long, default_value_t = 5.0, requires = "screenshot")]
    pub wait: f32,
    /// The command to run, after `--`. Without one the window runs `nie fm --protocol kitty`.
    #[arg(last = true)]
    pub command: Vec<String>,
}

/// Runs the terminal window.
pub fn run(args: TermArgs) -> anyhow::Result<()> {
    nie_tui::term::run(&nie_tui::term::TermOptions {
        cols: args.cols,
        rows: args.rows,
        font_size: args.font_size,
        screenshot: args.screenshot,
        wait: args.wait,
        command: args.command,
    })
}
