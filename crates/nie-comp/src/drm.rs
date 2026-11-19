//! The DRM/KMS backend: scanout on the primary GPU of the seat, input through libinput, device
//! access through a libseat session. One output, on the first connected connector.
//!
//! This path needs a seat (a console login or a display manager session) and is never started on
//! a headless host; CI only builds it.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use smithay::backend::allocator::Fourcc;
use smithay::backend::allocator::gbm::{GbmAllocator, GbmBufferFlags, GbmDevice};
use smithay::backend::drm::compositor::{DrmCompositor, FrameFlags};
use smithay::backend::drm::exporter::gbm::GbmFramebufferExporter;
use smithay::backend::drm::{DrmDevice, DrmDeviceFd, DrmEvent};
use smithay::backend::egl::{EGLContext, EGLDisplay};
use smithay::backend::input::InputEvent;
use smithay::backend::libinput::{LibinputInputBackend, LibinputSessionInterface};
use smithay::backend::renderer::ImportDma;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::session::libseat::LibSeatSession;
use smithay::backend::session::{Event as SessionEvent, Session};
use smithay::backend::udev::primary_gpu;
use smithay::desktop::space::space_render_elements;
use smithay::output::Mode as OutputMode;
use smithay::reexports::drm::control::{Device as ControlDevice, ModeTypeFlags, connector};
use smithay::reexports::input::Libinput;
use smithay::reexports::rustix::fs::OFlags;
use smithay::utils::DeviceFd;

use crate::render::{CLEAR_COLOR, finish_frame};
use crate::state::{Compositor, Config, State};

type Gbm = GbmDevice<DrmDeviceFd>;
type Scanout = DrmCompositor<GbmAllocator<DrmDeviceFd>, GbmFramebufferExporter<DrmDeviceFd>, (), DrmDeviceFd>;

struct Backend {
    device: DrmDevice,
    renderer: GlesRenderer,
    scanout: Scanout,
    libinput: Libinput,
    /// A frame is queued and its vblank has not arrived.
    pending: bool,
}

impl Backend {
    fn render(&mut self, state: &mut State) -> Result<()> {
        if self.pending || !self.device.is_active() {
            return Ok(());
        }
        let elements = space_render_elements(&mut self.renderer, [&state.space], &state.output, 1.0)
            .map_err(|e| anyhow!("scene: {e:?}"))?;
        let frame = self
            .scanout
            .render_frame(&mut self.renderer, &elements, CLEAR_COLOR, FrameFlags::DEFAULT)
            .map_err(|e| anyhow!("render frame: {e:?}"))?;
        let empty = frame.is_empty;
        drop(frame);
        if !empty {
            self.scanout.queue_frame(()).map_err(|e| anyhow!("queue frame: {e:?}"))?;
            self.pending = true;
        }
        finish_frame(state);
        Ok(())
    }
}

/// Take the seat and the primary GPU, light the first connected connector, serve clients.
pub fn run(config: &Config) -> Result<()> {
    let mut comp = Compositor::new(config)?;
    let (session, session_notifier) = LibSeatSession::new().map_err(|e| anyhow!("libseat session: {e:?}"))?;
    let seat = session.seat();

    let gpu = primary_gpu(&seat)?.context("no GPU on the seat")?;
    let fd = session
        .clone()
        .open(&gpu, OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK)
        .map_err(|e| anyhow!("open {}: {e:?}", gpu.display()))?;
    let fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let (mut device, drm_notifier) = DrmDevice::new(fd.clone(), true).map_err(|e| anyhow!("drm device: {e:?}"))?;
    let gbm: Gbm = GbmDevice::new(fd).context("gbm device")?;

    // SAFETY: the EGL display is created from a live GBM device that outlives it (kept in the backend).
    let egl = unsafe { EGLDisplay::new(gbm.clone()) }.map_err(|e| anyhow!("egl display: {e:?}"))?;
    let context = EGLContext::new(&egl).map_err(|e| anyhow!("egl context: {e:?}"))?;
    // SAFETY: the context was just created and is bound to this thread only.
    let renderer = unsafe { GlesRenderer::new(context) }.map_err(|e| anyhow!("gles renderer: {e:?}"))?;

    let resources = device.resource_handles().context("drm resources")?;
    let (connector, drm_mode, crtc) = resources
        .connectors()
        .iter()
        .find_map(|&handle| {
            let info = device.get_connector(handle, true).ok()?;
            if info.state() != connector::State::Connected {
                return None;
            }
            let mode = info
                .modes()
                .iter()
                .find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
                .or_else(|| info.modes().first())
                .copied()?;
            let crtc = info.encoders().iter().find_map(|&encoder| {
                let encoder = device.get_encoder(encoder).ok()?;
                resources.filter_crtcs(encoder.possible_crtcs()).first().copied()
            })?;
            Some((handle, mode, crtc))
        })
        .context("no connected connector with a usable CRTC")?;

    let surface = device.create_surface(crtc, drm_mode, &[connector]).map_err(|e| anyhow!("drm surface: {e:?}"))?;
    let mode = OutputMode::from(drm_mode);
    comp.state.output.change_current_state(Some(mode), None, None, None);
    comp.state.output.set_preferred(mode);
    comp.state.relayout();

    let formats = renderer.dmabuf_formats();
    comp.state.enable_dmabuf(formats.clone());
    let allocator = GbmAllocator::new(gbm.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);
    let exporter = GbmFramebufferExporter::new(gbm.clone(), None);
    let scanout: Scanout = DrmCompositor::new(
        &comp.state.output,
        surface,
        None,
        allocator,
        exporter,
        [Fourcc::Xrgb8888, Fourcc::Argb8888],
        formats.iter().copied(),
        device.cursor_size(),
        Some(gbm),
    )
    .map_err(|e| anyhow!("drm compositor: {e:?}"))?;

    let mut libinput = Libinput::new_with_udev(LibinputSessionInterface::from(session.clone()));
    libinput.udev_assign_seat(&seat).map_err(|()| anyhow!("libinput seat {seat}"))?;
    let input = LibinputInputBackend::new(libinput.clone());

    let backend = Rc::new(RefCell::new(Backend { device, renderer, scanout, libinput, pending: false }));
    let handle = comp.event_loop.handle();

    handle
        .insert_source(input, |event, _, state| {
            if !matches!(event, InputEvent::DeviceAdded { .. } | InputEvent::DeviceRemoved { .. }) {
                state.process_input_event(event);
            }
        })
        .map_err(|e| anyhow!("libinput source: {e}"))?;

    let on_vblank = backend.clone();
    handle
        .insert_source(drm_notifier, move |event, _, state| {
            if let DrmEvent::VBlank(_) = event {
                let mut backend = on_vblank.borrow_mut();
                if let Err(error) = backend.scanout.frame_submitted() {
                    tracing::warn!(?error, "frame_submitted");
                }
                backend.pending = false;
                state.needs_redraw = true;
            }
        })
        .map_err(|e| anyhow!("drm source: {e}"))?;

    let on_session = backend.clone();
    handle
        .insert_source(session_notifier, move |event, _, state| {
            let mut backend = on_session.borrow_mut();
            match event {
                SessionEvent::PauseSession => {
                    backend.libinput.suspend();
                    backend.device.pause();
                }
                SessionEvent::ActivateSession => {
                    if backend.libinput.resume().is_err() {
                        tracing::warn!("libinput resume failed");
                    }
                    if let Err(error) = backend.device.activate(true) {
                        tracing::warn!(?error, "drm activate");
                    }
                    backend.scanout.reset_buffers();
                    backend.pending = false;
                    state.needs_redraw = true;
                }
            }
        })
        .map_err(|e| anyhow!("session source: {e}"))?;

    comp.quit_on_signals()?;
    while !comp.state.quit {
        if comp.state.needs_redraw {
            backend.borrow_mut().render(&mut comp.state)?;
            comp.flush()?;
        }
        comp.dispatch(Some(Duration::from_millis(8)))?;
    }
    Ok(())
}
