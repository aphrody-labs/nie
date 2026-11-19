#![cfg_attr(target_family = "wasm", no_main)]
//! `nie-wiki`: the wiki of the game, computed from the VFS.

#[cfg(not(target_family = "wasm"))]
fn main() -> anyhow::Result<()> {
    use anyhow::Context as _;
    use nie_ui::theme::Preferences;

    let source = std::env::args().nth(1).or_else(|| std::env::var("NIE_GAME_SOURCE").ok());
    let wiki = std::sync::Arc::new(nie_wiki_vfs::Wiki::open(source.as_deref()).context("mount the game")?);
    let theme = nie_apps::theme_of_game(None, Preferences::from_env());
    gpui_platform::application().run(move |cx| {
        nie_ui::Settings::install(cx, theme);
        nie_apps::wiki::open(cx, wiki);
        cx.activate(true);
    });
    Ok(())
}
