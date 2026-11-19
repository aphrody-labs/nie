#![cfg_attr(target_family = "wasm", no_main)]
//! `nie-viewer`: pictures, sprite sheets, films and models of the game's VFS.

#[cfg(not(target_family = "wasm"))]
fn main() -> anyhow::Result<()> {
    use anyhow::Context as _;
    use gpui::SharedString;
    use nie_ui::theme::Preferences;

    let source = std::env::args().nth(1).or_else(|| std::env::var("NIE_GAME_SOURCE").ok());
    let vfs = std::sync::Arc::new(nie_formats::vfs::open_source(source.as_deref()).context("mount the game")?);
    let theme = nie_apps::theme_of_game(Some(&vfs), Preferences::from_env());
    let paths: Vec<SharedString> = nie_apps::viewable_paths(&vfs).into_iter().map(SharedString::from).collect();
    gpui_platform::application().run(move |cx| {
        nie_ui::Settings::install(cx, theme);
        nie_apps::viewer::open(cx, vfs, paths);
        cx.activate(true);
    });
    Ok(())
}
