#![cfg(target_os = "linux")]
//! The nie session compositor.
//!
//! A Wayland compositor on smithay 0.7: xdg-shell, wlr-layer-shell (the protocol GPUI panels
//! attach through), wl_seat with keyboard and pointer, wl_shm, linux-dmabuf, xdg-output, data
//! device, presentation-time, viewporter and xdg-decoration (always client side).
//!
//! The scene is one output. Layer surfaces keep their own geometry and exclusive zones; every
//! xdg toplevel fills the zone the layers leave free. The render path is generic over the
//! smithay renderer, so the same scene is drawn by:
//!
//! - [`Headless`]: smithay's CPU `PixmanRenderer` into an offscreen image. No GPU, no DRM, no
//!   display. [`Headless::screenshot`] composites the real client surfaces and returns the whole
//!   output as RGBA, which [`Screenshot::write_png`] saves.
//! - `winit` (feature `winit`): a nested window on a host Wayland or X11 session, through GLES.
//! - `drm` (feature `drm`): DRM/KMS scanout with libinput on a libseat session.
//!
//! `DummyRenderer` was evaluated and rejected: it imports buffers but draws nothing, so it cannot
//! produce a screenshot.

mod handlers;
mod headless;
mod input;
mod render;
mod state;

#[cfg(feature = "drm")]
pub mod drm;
#[cfg(feature = "winit")]
pub mod winit;
#[cfg(feature = "xwayland")]
pub mod xwm;

pub use headless::{Headless, Screenshot};
pub use state::{Compositor, Config, State};
