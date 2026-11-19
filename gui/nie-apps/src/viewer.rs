//! The asset viewer: a picture, a sprite sheet, a film or a model turntable of one VFS file, built
//! by the previewers of `nie_explore::fm` and shown on the toolkit's surfaces with the file's
//! facts beside it. Open another file with the picker.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use gpui::{
    App, AppContext as _, Bounds, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, Role,
    SharedString, Styled, Subscription, Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use nie_explore::fm::preview::{self, Body, Context as PreviewContext, PreviewRequest};
use nie_formats::vfs::Vfs;
use nie_ui::prelude::*;

/// A fixed list of pictures played at a rate: the frames of a film, the sprites of a sheet or the
/// turntable of a model.
struct Frames {
    images: Vec<Rgba8>,
    fps: u32,
}

impl FrameSource for Frames {
    fn fps(&self) -> u32 {
        self.fps
    }
    fn count(&self) -> Option<u64> {
        Some(self.images.len() as u64)
    }
    fn frame(&mut self, index: u64) -> Option<Rgba8> {
        self.images.get(usize::try_from(index).ok()?).cloned()
    }
}

fn picture(i: &preview::Image) -> Rgba8 {
    Rgba8 { width: i.width, height: i.height, rgba: i.rgba.clone() }
}

/// The viewer.
pub struct Viewer {
    vfs: Arc<Vfs>,
    picker: Entity<Picker>,
    surface: Option<Entity<FrameSurface>>,
    meta: Vec<(String, String)>,
    title: SharedString,
    status: SharedString,
    serial: u64,
    _subscription: Subscription,
}

impl Viewer {
    /// A viewer over a mounted game; `paths` feeds the picker.
    pub fn new(vfs: Arc<Vfs>, paths: Vec<SharedString>, cx: &mut Context<Self>) -> Self {
        let picker = cx.new(|cx| Picker::new("Open a file", paths.clone(), cx));
        let subscription = cx.subscribe(&picker, move |this, _, event: &PickerEvent, cx| {
            if let PickerEvent::Chosen(ix) = event {
                this.open(paths[*ix].to_string(), cx);
            }
        });
        Self {
            vfs,
            picker,
            surface: None,
            meta: Vec::new(),
            title: SharedString::from("Nothing open"),
            status: SharedString::from("Pick a file"),
            serial: 0,
            _subscription: subscription,
        }
    }

    /// Opens a VFS path.
    pub fn open(&mut self, path: String, cx: &mut Context<Self>) {
        self.serial += 1;
        let serial = self.serial;
        self.status = SharedString::from(format!("Decoding {path}..."));
        cx.notify();
        let vfs = Arc::clone(&self.vfs);
        let request = PreviewRequest { path: path.clone(), size: 0, hex: false, index: 0 };
        cx.spawn(async move |this, cx| {
            let built = cx
                .background_spawn(async move {
                    let never = AtomicBool::new(false);
                    let cancelled = || never.load(std::sync::atomic::Ordering::Relaxed);
                    let ctx = PreviewContext { vfs: &vfs, cache: None, cancelled: &cancelled };
                    preview::build(&ctx, &request)
                })
                .await;
            this.update(cx, |this, cx| {
                if this.serial != serial {
                    return;
                }
                let (images, fps) = match &built.body {
                    Body::Image(i) => (vec![picture(i)], 0),
                    Body::Frames(f) => (f.images.iter().map(picture).collect(), f.fps),
                    _ => (Vec::new(), 0),
                };
                this.title = SharedString::from(path.clone());
                this.meta = built.meta.clone();
                this.status = SharedString::from(if images.is_empty() {
                    format!("{}: nothing to draw ({})", built.kind, path)
                } else {
                    format!("{}, {} frame(s) in {:.0} ms", built.kind, images.len(), built.took.as_secs_f64() * 1000.0)
                });
                this.surface = (!images.is_empty()).then(|| {
                    let label = SharedString::from(format!("{}: {}", built.kind, path));
                    let surface = cx.new(|_| FrameSurface::new("nie-viewer-surface", label, Frames { images, fps }));
                    surface.update(cx, |s, cx| s.set_playing(fps > 0, cx));
                    surface
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for Viewer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Settings::theme(cx).clone();
        div()
            .id("nie-viewer")
            .role(Role::Application)
            .aria_label("nie asset viewer")
            .size_full()
            .flex()
            .bg(theme.background.hsla())
            .text_color(theme.text.hsla())
            .font_family(theme.ui_font)
            .text_size(theme.px(theme.text_size))
            .child(div().w_1_3().h_full().border_r_1().border_color(theme.border.hsla()).child(self.picker.clone()))
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("nie-viewer-title")
                            .role(Role::Heading)
                            .aria_level(1)
                            .aria_label(self.title.clone())
                            .h(theme.px(36.0))
                            .px(theme.px(10.0))
                            .flex()
                            .items_center()
                            .border_b_1()
                            .border_color(theme.border.hsla())
                            .text_color(theme.accent.hsla())
                            .child(self.title.clone()),
                    )
                    .child(div().flex_1().min_h_0().children(self.surface.clone()))
                    .child(
                        div()
                            .id("nie-viewer-meta")
                            .role(Role::Status)
                            .aria_label(SharedString::from(format!(
                                "{}. {}",
                                self.status,
                                self.meta.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ")
                            )))
                            .px(theme.px(10.0))
                            .py(theme.px(6.0))
                            .border_t_1()
                            .border_color(theme.border.hsla())
                            .text_color(theme.text_dim.hsla())
                            .child(SharedString::from(format!(
                                "{}  {}",
                                self.status,
                                self.meta.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join("  ")
                            ))),
                    ),
            )
    }
}

/// Opens the viewer window.
pub fn open(cx: &mut App, vfs: Arc<Vfs>, paths: Vec<SharedString>) {
    let bounds = Bounds::centered(None, size(px(1280.0), px(760.0)), cx);
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(gpui::TitlebarOptions { title: Some("nie viewer".into()), ..Default::default() }),
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(|cx| Viewer::new(vfs, paths, cx));
            let focus = view.read(cx).picker.read(cx).focus_handle().clone();
            window.focus(&focus, cx);
            view
        },
    );
    if let Err(error) = opened {
        eprintln!("nie-viewer: cannot open a window: {error:#}");
        cx.quit();
    }
}
