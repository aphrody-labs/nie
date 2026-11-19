//! The terminal explorer of the game VFS (`nie fm`) and the GPU terminal that hosts it
//! (`nie term`).
//!
//! The renderer-independent model lives in `nie_explore::fm`; this crate draws it with ratatui and
//! `ratatui-image` (kitty, sixel, iTerm2 and half-block pictures), runs the previews on tokio, and
//! owns the two commands.

#![forbid(unsafe_code)]

pub mod app;
pub mod pane;
pub mod sched;
pub mod session;
#[cfg(feature = "term")]
pub mod term;
pub mod theme;
pub mod ui;

use nie_plugin::{Plugin, PluginInfo};

/// The top-level commands of this build: `term` exists with the `term` feature.
#[cfg(feature = "term")]
const COMMANDS: &[&str] = &["fm", "term"];
#[cfg(not(feature = "term"))]
const COMMANDS: &[&str] = &["fm"];

/// The plugin declaration.
pub struct TuiPlugin;

impl Plugin for TuiPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: "tui",
            summary: "terminal file explorer and previewers of the game VFS, and the GPU terminal that hosts it",
            package: "nie-tui",
            commands: COMMANDS,
        }
    }
}
