//! The apps of nie on the GPUI toolkit.
//!
//! - [`explorer`]: the GUI twin of `nie fm`, on the same `nie_explore::fm` view model (native).
//! - [`http_explorer`]: the same view over `nie serve` (the browser build and native clients).
//! - [`viewer`]: pictures, sprite sheets, films and model turntables of one VFS file (native).
//! - [`wiki`]: characters, skills and teams computed from the VFS (native).
//! - `web`: the browser entry point, compiled for `wasm32-unknown-unknown` only.
//!
//! Every app is a function that opens a window on a GPUI `App`; the binaries and the browser entry
//! point only choose the platform.

#[cfg(not(target_family = "wasm"))]
pub mod explorer;
pub mod http_explorer;
#[cfg(not(target_family = "wasm"))]
pub mod lua_tabs;
#[cfg(not(target_family = "wasm"))]
pub mod viewer;
#[cfg(not(target_family = "wasm"))]
pub mod wiki;
#[cfg(target_family = "wasm")]
pub mod web;

/// The theme of the installed game: the palette of `data/common/font/font_color.cfg.bin`, or the
/// contrast floor when no game is mounted or the file cannot be read.
#[cfg(not(target_family = "wasm"))]
#[must_use]
pub fn theme_of_game(vfs: Option<&nie_formats::vfs::Vfs>, preferences: nie_ui::Preferences) -> nie_ui::Theme {
    vfs.and_then(|v| v.read("data/common/font/font_color.cfg.bin").ok())
        .and_then(|bytes| nie_ui::Theme::from_font_color_bin(&bytes, preferences).ok())
        .unwrap_or_else(|| nie_ui::Theme::bootstrap(preferences))
}

/// The VFS paths the viewer can draw: textures, atlases, models and films.
#[cfg(not(target_family = "wasm"))]
#[must_use]
pub fn viewable_paths(vfs: &nie_formats::vfs::Vfs) -> Vec<String> {
    const EXTENSIONS: [&str; 6] = [".g4tx", ".g4md", ".usm", ".png", ".dds", ".jpg"];
    let mut paths: Vec<String> =
        vfs.iter().filter(|(p, _)| EXTENSIONS.iter().any(|e| p.ends_with(e))).map(|(p, _)| p.to_owned()).collect();
    paths.sort();
    paths
}
