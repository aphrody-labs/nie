#![cfg_attr(target_family = "wasm", no_main)]
//! `nie-explorer`: the GUI twin of `nie fm`.

#[cfg(not(target_family = "wasm"))]
use std::sync::Arc;

use anyhow::{Context as _, Result};
use nie_apps::explorer::{FmBackend, open};
use nie_explore::fm::thumbs::ThumbCache;
use nie_ui::theme::Preferences;
use nie_ui::Settings;

fn main() -> Result<()> {
    let source = std::env::args().nth(1).or_else(|| std::env::var("NIE_GAME_SOURCE").ok());
    let vfs = Arc::new(nie_formats::vfs::open_source(source.as_deref()).context("mount the game")?);
    let cache = ThumbCache::open(Some(vfs.game_data_dir())).ok().map(Arc::new);
    let theme = nie_apps::theme_of_game(Some(&vfs), Preferences::from_env());
    let backend = FmBackend::new(vfs, cache);
    gpui_platform::application().run(move |cx| {
        Settings::install(cx, theme);
        open(cx, backend);
        cx.activate(true);
    });
    Ok(())
}
