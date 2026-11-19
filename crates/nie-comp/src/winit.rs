//! The winit backend: the compositor in a window of a host Wayland or X11 session (GLES).

use std::time::Duration;

use anyhow::{Result, anyhow};
use smithay::backend::renderer::ImportDma;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::winit::{self, WinitEvent};
use smithay::reexports::winit::platform::pump_events::PumpStatus;

use crate::render::{finish_frame, render_scene};
use crate::state::{Compositor, Config};

/// Open the window and serve clients until it is closed.
pub fn run(config: &Config) -> Result<()> {
    let mut comp = Compositor::new(config)?;
    let (mut backend, mut events) =
        winit::init::<GlesRenderer>().map_err(|e| anyhow!("winit backend: {e:?}"))?;
    comp.state.set_output_size(backend.window_size());
    let formats = backend.renderer().dmabuf_formats();
    comp.state.enable_dmabuf(formats);
    let mut tracker = OutputDamageTracker::from_output(&comp.state.output);

    comp.quit_on_signals()?;
    let mut closed = false;
    while !closed && !comp.state.quit {
        let mut redraw = comp.state.needs_redraw;
        let status = events.dispatch_new_events(|event| match event {
            WinitEvent::Resized { size, .. } => {
                comp.state.set_output_size(size);
                redraw = true;
            }
            WinitEvent::Input(input) => comp.state.process_input_event(input),
            WinitEvent::Redraw => redraw = true,
            WinitEvent::CloseRequested => closed = true,
            WinitEvent::Focus(_) => {}
        });
        if matches!(status, PumpStatus::Exit(_)) {
            break;
        }
        if redraw || comp.state.needs_redraw {
            let age = backend.buffer_age().unwrap_or(0);
            let damage = {
                let (renderer, mut framebuffer) = backend.bind().map_err(|e| anyhow!("bind: {e:?}"))?;
                render_scene(&comp.state, renderer, &mut framebuffer, &mut tracker, age)?
            };
            backend.submit(damage.as_deref()).map_err(|e| anyhow!("submit: {e:?}"))?;
            finish_frame(&mut comp.state);
            comp.flush()?;
        }
        comp.dispatch(Some(Duration::from_millis(8)))?;
    }
    Ok(())
}
