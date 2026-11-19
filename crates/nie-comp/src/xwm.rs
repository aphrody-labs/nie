//! XWayland: X11 clients (Steam and its games) as windows of the same scene.

use std::process::Stdio;

use anyhow::{Context, Result, anyhow};
use smithay::desktop::Window;
use smithay::utils::{Logical, Rectangle};
use smithay::wayland::xwayland_shell::{XWaylandShellHandler, XWaylandShellState};
use smithay::xwayland::xwm::{Reorder, ResizeEdge, XwmId};
use smithay::xwayland::{X11Surface, X11Wm, XWayland, XWaylandEvent, XwmHandler};
use calloop::LoopHandle;
use smithay::delegate_xwayland_shell;

use crate::state::{Compositor, State};

/// Spawn `Xwayland` and attach the window manager once it is ready. The X display number is
/// logged and kept in [`State::xdisplay`]; clients need `DISPLAY=:<number>`.
pub fn start(comp: &mut Compositor) -> Result<()> {
    let (xwayland, client) = XWayland::spawn(
        &comp.state.dh,
        None,
        std::iter::empty::<(&str, &str)>(),
        true,
        Stdio::null(),
        Stdio::null(),
        |_| {},
    )
    .context("spawn Xwayland")?;
    let handle: LoopHandle<'static, State> = comp.state.loop_handle.clone();
    handle
        .insert_source(xwayland, move |event, _, state| match event {
            XWaylandEvent::Ready { x11_socket, display_number } => {
                match X11Wm::start_wm(state.loop_handle.clone(), x11_socket, client.clone()) {
                    Ok(wm) => {
                        state.xwm = Some(wm);
                        state.xdisplay = Some(display_number);
                        tracing::info!(display = display_number, "xwayland ready (DISPLAY=:{display_number})");
                    }
                    Err(error) => tracing::error!(%error, "xwayland window manager"),
                }
            }
            XWaylandEvent::Error => tracing::error!("xwayland exited during startup"),
        })
        .map_err(|e| anyhow!("xwayland source: {e}"))?;
    Ok(())
}

impl State {
    fn x11_window(&self, surface: &X11Surface) -> Option<Window> {
        self.space.elements().find(|w| w.x11_surface() == Some(surface)).cloned()
    }
}

impl XWaylandShellHandler for State {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        &mut self.xwayland_shell_state
    }
}

impl XwmHandler for State {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        self.xwm.as_mut().expect("XwmHandler is only called while the window manager runs")
    }

    fn new_window(&mut self, _xwm: XwmId, _window: X11Surface) {}
    fn new_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn map_window_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let zone = self.usable_zone();
        let _ = window.set_mapped(true);
        let _ = window.configure(zone);
        self.space.map_element(Window::new_x11_window(window), zone.loc, true);
        self.needs_redraw = true;
    }

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        let location = window.geometry().loc;
        self.space.map_element(Window::new_x11_window(window), location, true);
        self.needs_redraw = true;
    }

    fn unmapped_window(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(element) = self.x11_window(&window) {
            self.space.unmap_elem(&element);
        }
        if !window.is_override_redirect() {
            let _ = window.set_mapped(false);
        }
        self.needs_redraw = true;
    }

    fn destroyed_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn configure_request(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        x: Option<i32>,
        y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        _reorder: Option<Reorder>,
    ) {
        let mut geometry = window.geometry();
        if let Some(x) = x {
            geometry.loc.x = x;
        }
        if let Some(y) = y {
            geometry.loc.y = y;
        }
        if let Some(w) = w {
            geometry.size.w = w as i32;
        }
        if let Some(h) = h {
            geometry.size.h = h as i32;
        }
        let _ = window.configure(geometry);
    }

    fn configure_notify(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        geometry: Rectangle<i32, Logical>,
        _above: Option<u32>,
    ) {
        if window.is_override_redirect()
            && let Some(element) = self.x11_window(&window)
        {
            self.space.map_element(element, geometry.loc, false);
            self.needs_redraw = true;
        }
    }

    fn resize_request(&mut self, _xwm: XwmId, _window: X11Surface, _button: u32, _edge: ResizeEdge) {}
    fn move_request(&mut self, _xwm: XwmId, _window: X11Surface, _button: u32) {}
}

delegate_xwayland_shell!(State);
