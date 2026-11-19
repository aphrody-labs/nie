//! The desktop shell of the nie OS: a top panel, an app launcher and notifications, each a GPUI
//! window over wlr-layer-shell (`gpui_linux`'s `layer_shell`). `nie-comp` is the compositor that
//! hosts them; any wlr-layer-shell compositor works. The model (apps, notification queue, panel
//! geometry) is `plugins/shell-desktop/nie-shell-desktop`.

pub use nie_shell_desktop::model;
#[cfg(target_os = "linux")]
pub mod views;
