//! Surfaces that show pixels: a still image, and a frame source that animates (a film, a
//! turntable of the 3D viewer of nie-render3d). Pixels arrive as RGBA8 and are shown through
//! GPUI's image atlas; every surface carries an accessible name.

use std::sync::Arc;
use std::time::Duration;

use gpui::{
    Context, InteractiveElement, IntoElement, ObjectFit, ParentElement, Render, RenderImage, Role, SharedString,
    Styled, StyledImage, Window, div, img, prelude::*,
};
use image::{Frame, RgbaImage};

use crate::a11y::Settings;

/// A picture, RGBA8, row-major.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba8 {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

/// Converts RGBA8 pixels to the image GPUI paints (its atlas stores BGRA).
///
/// Returns `None` when the buffer does not match the declared size.
#[must_use]
pub fn render_image(frame: &Rgba8) -> Option<Arc<RenderImage>> {
    let mut bgra = frame.rgba.clone();
    for px in bgra.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let buffer = RgbaImage::from_raw(frame.width, frame.height, bgra)?;
    Some(Arc::new(RenderImage::new(vec![Frame::new(buffer)])))
}

/// A still image with an accessible description.
#[derive(IntoElement)]
pub struct ImageSurface {
    id: SharedString,
    alt: SharedString,
    image: Arc<RenderImage>,
}

impl ImageSurface {
    /// An image named `alt` for assistive technology; `id` must be unique in the window.
    pub fn new(id: impl Into<SharedString>, alt: impl Into<SharedString>, image: Arc<RenderImage>) -> Self {
        Self { id: id.into(), alt: alt.into(), image }
    }
}

impl RenderOnce for ImageSurface {
    fn render(self, _window: &mut Window, _cx: &mut gpui::App) -> impl IntoElement {
        div()
            .id(self.id)
            .role(Role::Image)
            .aria_label(self.alt)
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(img(self.image).object_fit(ObjectFit::Contain).max_w_full().max_h_full())
    }
}

/// Where the frames of a [`FrameSurface`] come from: the frames of a video, or the turntable of
/// the 3D viewer. `frame` runs on the UI thread, so heavy sources keep their own worker and hand
/// over finished pictures.
pub trait FrameSource: 'static {
    /// Frames per second; 0 means a still source that is drawn once.
    fn fps(&self) -> u32;
    /// The picture at `index` (the surface wraps at `count`).
    fn frame(&mut self, index: u64) -> Option<Rgba8>;
    /// Number of distinct frames, or `None` for an endless source.
    fn count(&self) -> Option<u64>;
}

/// A surface that plays a [`FrameSource`].
pub struct FrameSurface {
    id: SharedString,
    label: SharedString,
    source: Box<dyn FrameSource>,
    index: u64,
    current: Option<Arc<RenderImage>>,
    playing: bool,
}

impl FrameSurface {
    /// A surface named `label` over `source`, paused on its first frame.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>, source: impl FrameSource) -> Self {
        let mut surface = Self {
            id: id.into(),
            label: label.into(),
            source: Box::new(source),
            index: 0,
            current: None,
            playing: false,
        };
        surface.load();
        surface
    }

    fn load(&mut self) {
        if let Some(frame) = self.source.frame(self.index) {
            self.current = render_image(&frame);
        }
    }

    /// Steps to the next frame.
    pub fn advance(&mut self, cx: &mut Context<Self>) {
        self.index = match self.source.count() {
            Some(n) if n > 0 => (self.index + 1) % n,
            _ => self.index + 1,
        };
        self.load();
        cx.notify();
    }

    /// Whether the surface is playing.
    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Starts or stops playback. Reduced motion (the reader's setting) keeps it stopped.
    pub fn set_playing(&mut self, playing: bool, cx: &mut Context<Self>) {
        let allowed = playing && !Settings::theme(cx).preferences.reduce_motion && self.source.fps() > 0;
        if allowed == self.playing {
            return;
        }
        self.playing = allowed;
        if allowed {
            let period = Duration::from_secs_f64(1.0 / f64::from(self.source.fps()));
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(period).await;
                    let keep = this
                        .update(cx, |this, cx| {
                            if this.playing {
                                this.advance(cx);
                            }
                            this.playing
                        })
                        .unwrap_or(false);
                    if !keep {
                        break;
                    }
                }
            })
            .detach();
        }
        cx.notify();
    }
}

impl Render for FrameSurface {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Settings::theme(cx).clone();
        let label = if self.playing {
            SharedString::from(format!("{}, playing", self.label))
        } else {
            self.label.clone()
        };
        div()
            .id(self.id.clone())
            .role(Role::Image)
            .aria_label(label)
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.background.hsla())
            .children(
                self.current
                    .clone()
                    .map(|image| img(image).object_fit(ObjectFit::Contain).max_w_full().max_h_full()),
            )
    }
}
