//! `nie-shell`: the panel, the launcher and the notifications of the nie desktop, as layer-shell
//! windows. Linux (Wayland) only.

#[cfg(target_os = "linux")]
fn main() {
    use nie_shell_gui::model::APPS;
    use nie_shell_gui::views::{open_bar, open_launcher, open_toasts};
    use nie_ui::theme::{Preferences, Theme};

    let preferences = Preferences::from_env();
    let theme = Theme::bootstrap(preferences);
    gpui_platform::application().run(move |cx| {
        nie_ui::Settings::install(cx, theme);
        let toasts = open_toasts(cx);
        let scale = preferences.scale;
        open_bar(cx, scale, |cx| {
            open_launcher(cx, |ix, _cx| {
                let app = &APPS[ix];
                if let Err(e) = std::process::Command::new(app.binary).args(app.args).spawn() {
                    eprintln!("nie-shell: cannot start {}: {e}", app.binary);
                }
            });
        });
        if let Some(toasts) = toasts {
            toasts.update(cx, |t, cx| t.notify("nie", "Desktop shell started", cx));
        }
        cx.activate(true);
    });
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("nie-shell needs Linux and a wlr-layer-shell compositor");
}
