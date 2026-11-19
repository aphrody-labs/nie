//! The shell's windows.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gpui::layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions};
use gpui::{
    App, AppContext as _, Bounds, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, Role,
    SharedString, StatefulInteractiveElement, Styled, Subscription, Window, WindowBackgroundAppearance, WindowBounds,
    WindowKind, WindowOptions, div, point, px, size,
};
use nie_ui::prelude::*;

use crate::model::{APPS, Notifications, PANEL_HEIGHT};

/// Seconds since the epoch to `HH:MM` in UTC; the shell has no time zone database of its own.
fn clock() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    format!("{:02}:{:02} UTC", (secs / 3600) % 24, (secs / 60) % 60)
}

/// The top panel: the launcher button, the title and the clock.
pub struct Bar {
    on_launcher: Box<dyn Fn(&mut App)>,
}

impl Bar {
    /// A bar whose launcher button calls `on_launcher`.
    pub fn new(on_launcher: impl Fn(&mut App) + 'static, cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(10)).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        })
        .detach();
        Self { on_launcher: Box::new(on_launcher) }
    }
}

impl Render for Bar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Settings::theme(cx).clone();
        let time = clock();
        div()
            .id("nie-bar")
            .role(Role::Toolbar)
            .aria_label("nie panel")
            .size_full()
            .flex()
            .items_center()
            .justify_between()
            .px(theme.px(10.0))
            .bg(theme.surface.hsla())
            .text_color(theme.text.hsla())
            .font_family(theme.ui_font)
            .text_size(theme.px(theme.text_size))
            .border_b_1()
            .border_color(theme.border.hsla())
            .child(
                div()
                    .id("nie-bar-launcher")
                    .role(Role::Button)
                    .aria_label("Applications")
                    .focusable()
                    .tab_stop(true)
                    .px(theme.px(10.0))
                    .py(theme.px(4.0))
                    .rounded_md()
                    .cursor_pointer()
                    .text_color(theme.accent.hsla())
                    .hover(|d| d.bg(theme.surface_hover.hsla()))
                    .focus_visible(|d| d.border_2().border_color(theme.focus.hsla()))
                    .on_click(cx.listener(|this, _, _window, cx| (this.on_launcher)(cx)))
                    .child("nie"),
            )
            .child(div().id("nie-bar-clock").role(Role::Status).aria_label(SharedString::from(format!("Time {time}"))).child(SharedString::from(time)))
    }
}

/// The launcher: a picker over the apps.
pub struct Launcher {
    picker: Entity<Picker>,
    _subscription: Subscription,
}

impl Launcher {
    /// A launcher; `on_launch` receives the index into [`APPS`].
    pub fn new(window: &mut Window, on_launch: impl Fn(usize, &mut App) + 'static, cx: &mut Context<Self>) -> Self {
        let items: Vec<SharedString> =
            APPS.iter().map(|a| SharedString::from(format!("{}  {}", a.name, a.summary))).collect();
        let picker = cx.new(|cx| Picker::new("Applications", items, cx));
        let subscription = cx.subscribe_in(&picker, window, move |_, _, event: &PickerEvent, window, cx| {
            if let PickerEvent::Chosen(ix) = event {
                on_launch(*ix, cx);
            }
            window.remove_window();
        });
        Self { picker, _subscription: subscription }
    }

    /// The picker's focus handle.
    pub fn focus_handle(&self, cx: &App) -> gpui::FocusHandle {
        self.picker.read(cx).focus_handle().clone()
    }
}

impl Render for Launcher {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Settings::theme(cx).clone();
        div()
            .id("nie-launcher")
            .role(Role::Dialog)
            .aria_label("Application launcher")
            .size_full()
            .border_1()
            .border_color(theme.border.hsla())
            .child(self.picker.clone())
    }
}

/// The notification stack.
pub struct Toasts {
    queue: Notifications,
}

impl Toasts {
    /// An empty stack that expires its items by itself.
    pub fn new(cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this.update(cx, |this, cx| if this.queue.expire(Instant::now()) { cx.notify() }).is_err() {
                    break;
                }
            }
        })
        .detach();
        Self { queue: Notifications::new(5, Duration::from_secs(8)) }
    }

    /// Shows a notification.
    pub fn notify(&mut self, source: &str, text: &str, cx: &mut Context<Self>) {
        self.queue.push(source, text, Instant::now());
        cx.notify();
    }
}

impl Render for Toasts {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Settings::theme(cx).clone();
        div()
            .id("nie-toasts")
            .role(Role::Log)
            .aria_label("Notifications")
            .size_full()
            .flex()
            .flex_col()
            .gap(theme.px(6.0))
            .p(theme.px(6.0))
            .font_family(theme.ui_font)
            .text_size(theme.px(theme.text_size))
            .children(self.queue.items().iter().map(|n| {
                let id = n.id;
                div()
                    .id(("nie-toast", id as usize))
                    .role(Role::Status)
                    .aria_label(SharedString::from(format!("{}: {}", n.source, n.text)))
                    .p(theme.px(10.0))
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border.hsla())
                    .bg(theme.surface.hsla())
                    .text_color(theme.text.hsla())
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.queue.dismiss(id);
                        cx.notify();
                    }))
                    .child(div().text_color(theme.accent.hsla()).child(SharedString::from(n.source.clone())))
                    .child(SharedString::from(n.text.clone()))
            }))
    }
}

fn layer_window(
    cx: &mut App,
    name: &str,
    options: LayerShellOptions,
    (w, h): (f32, f32),
) -> WindowOptions {
    let _ = cx;
    WindowOptions {
        titlebar: None,
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.0), px(0.0)), size: size(px(w), px(h)) })),
        app_id: Some(format!("nie-shell-{name}")),
        window_background: WindowBackgroundAppearance::Opaque,
        kind: WindowKind::LayerShell(options),
        ..Default::default()
    }
}

/// Opens the top panel; its exclusive zone keeps toplevels out from under it.
pub fn open_bar(cx: &mut App, theme_scale: f32, on_launcher: impl Fn(&mut App) + 'static) {
    let height = PANEL_HEIGHT * theme_scale;
    let options = LayerShellOptions {
        namespace: "nie-bar".into(),
        layer: Layer::Top,
        anchor: Anchor::TOP | Anchor::LEFT | Anchor::RIGHT,
        exclusive_zone: Some(px(height)),
        keyboard_interactivity: KeyboardInteractivity::OnDemand,
        ..Default::default()
    };
    let window = layer_window(cx, "bar", options, (1280.0, height));
    if let Err(e) = cx.open_window(window, |_, cx| cx.new(|cx| Bar::new(on_launcher, cx))) {
        eprintln!("nie-shell: cannot open the panel: {e:#}");
    }
}

/// Opens the launcher overlay.
pub fn open_launcher(cx: &mut App, on_launch: impl Fn(usize, &mut App) + 'static) {
    let options = LayerShellOptions {
        namespace: "nie-launcher".into(),
        layer: Layer::Overlay,
        anchor: Anchor::TOP,
        margin: Some((px(80.0), px(0.0), px(0.0), px(0.0))),
        keyboard_interactivity: KeyboardInteractivity::Exclusive,
        ..Default::default()
    };
    let window = layer_window(cx, "launcher", options, (560.0, 320.0));
    if let Err(e) = cx.open_window(window, |window, cx| {
        let view = cx.new(|cx| Launcher::new(window, on_launch, cx));
        window.focus(&view.read(cx).focus_handle(cx), cx);
        view
    }) {
        eprintln!("nie-shell: cannot open the launcher: {e:#}");
    }
}

/// Opens the notification stack and returns its handle.
pub fn open_toasts(cx: &mut App) -> Option<Entity<Toasts>> {
    let options = LayerShellOptions {
        namespace: "nie-notifications".into(),
        layer: Layer::Overlay,
        anchor: Anchor::TOP | Anchor::RIGHT,
        margin: Some((px(PANEL_HEIGHT + 8.0), px(8.0), px(0.0), px(0.0))),
        keyboard_interactivity: KeyboardInteractivity::None,
        ..Default::default()
    };
    let window = layer_window(cx, "notifications", options, (360.0, 320.0));
    let handle = cx.open_window(window, |_, cx| cx.new(Toasts::new)).ok()?;
    handle.entity(cx).ok()
}
