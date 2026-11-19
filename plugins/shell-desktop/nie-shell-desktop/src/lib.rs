//! The desktop shell plugin of nie: panels, launcher and notifications over wlr-layer-shell.
//!
//! This crate is the renderer-independent part: the apps the launcher offers, the notification
//! queue and the panel geometry ([`model`]). `gui/nie-shell` draws it with GPUI windows, and
//! `crates/nie-comp` is the compositor that hosts those windows. prd stays headless: the shell runs
//! on the build host (smithay's headless backend) and on desktops, never in production.

#![forbid(unsafe_code)]

pub mod model;

/// The shell plugin. It owns no `nie` subcommand: its programs are `nie-shell` and `nie-comp`.
pub struct ShellDesktopPlugin;

impl nie_plugin::Plugin for ShellDesktopPlugin {
    fn info(&self) -> nie_plugin::PluginInfo {
        nie_plugin::PluginInfo {
            id: "shell-desktop",
            summary: "Desktop shell: panel, launcher and notifications on wlr-layer-shell (programs nie-shell and nie-comp)",
            package: "nie-shell-desktop",
            commands: &[],
        }
    }
}

#[cfg(test)]
mod tests {
    use nie_plugin::Plugin as _;

    #[test]
    fn the_plugin_declares_itself() {
        let info = super::ShellDesktopPlugin.info();
        assert_eq!(info.id, "shell-desktop");
        assert!(info.commands.is_empty());
    }
}
