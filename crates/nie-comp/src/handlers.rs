//! Wayland protocol handlers: the glue between smithay's protocol states and [`State`].

use smithay::backend::renderer::utils::on_commit_buffer_handler;
use smithay::desktop::{
    LayerSurface as DesktopLayer, PopupKind, Window, WindowSurfaceType, layer_map_for_output,
};
use smithay::input::pointer::CursorImageStatus;
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::output::Output;
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode as DecorationMode;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::{Client, Resource};
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_seat::WlSeat;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::Serial;
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
    CompositorClientState, CompositorHandler, CompositorState, get_parent, is_sync_subsurface, with_states,
};
use smithay::wayland::dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier};
use smithay::wayland::output::OutputHandler;
use smithay::wayland::selection::SelectionHandler;
use smithay::wayland::selection::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler, set_data_device_focus,
};
use smithay::wayland::shell::wlr_layer::{
    Layer, LayerSurface, LayerSurfaceData, WlrLayerShellHandler, WlrLayerShellState,
};
use smithay::wayland::shell::xdg::decoration::XdgDecorationHandler;
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
};
use smithay::wayland::shm::{ShmHandler, ShmState};
use smithay::{
    delegate_compositor, delegate_data_device, delegate_dmabuf, delegate_layer_shell, delegate_output,
    delegate_presentation, delegate_seat, delegate_shm, delegate_viewporter, delegate_xdg_decoration,
    delegate_xdg_shell,
};

use crate::state::{ClientState, State};

impl CompositorHandler for State {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        if let Some(data) = client.get_data::<ClientState>() {
            return &data.compositor;
        }
        #[cfg(feature = "xwayland")]
        if let Some(data) = client.get_data::<smithay::xwayland::XWaylandClientData>() {
            return &data.compositor_state;
        }
        unreachable!("every client is created with ClientState or XWaylandClientData")
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            let window = self
                .space
                .elements()
                .find(|w| w.toplevel().is_some_and(|t| t.wl_surface() == &root))
                .cloned();
            if let Some(window) = window {
                window.on_commit();
                if let Some(toplevel) = window.toplevel()
                    && !toplevel.is_initial_configure_sent()
                {
                    toplevel.send_configure();
                }
            }
        }

        self.popups.commit(surface);
        if let Some(PopupKind::Xdg(popup)) = self.popups.find_popup(surface)
            && !popup.is_initial_configure_sent()
        {
            let _ = popup.send_configure();
        }

        let layer = layer_map_for_output(&self.output)
            .layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
            .cloned();
        if let Some(layer) = layer {
            let sent = with_states(surface, |states| {
                states
                    .data_map
                    .get::<LayerSurfaceData>()
                    .is_some_and(|data| data.lock().is_ok_and(|d| d.initial_configure_sent))
            });
            self.relayout();
            if !sent {
                layer.layer_surface().send_configure();
                if layer.can_receive_keyboard_focus() {
                    self.focus(Some(surface.clone()));
                }
            }
        }
        self.needs_redraw = true;
    }
}

impl BufferHandler for State {
    fn buffer_destroyed(&mut self, _buffer: &smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer) {}
}

impl ShmHandler for State {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl OutputHandler for State {}

impl SeatHandler for State {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<State> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, seat: &Seat<State>, focused: Option<&WlSurface>) {
        let client = focused.and_then(|surface| self.dh.get_client(surface.id()).ok());
        set_data_device_focus(&self.dh, seat, client);
    }

    fn cursor_image(&mut self, _seat: &Seat<State>, _image: CursorImageStatus) {}
}

impl SelectionHandler for State {
    type SelectionUserData = ();
}

impl DataDeviceHandler for State {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for State {}
impl ServerDndGrabHandler for State {}

impl DmabufHandler for State {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf_state
    }

    fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        dmabuf: smithay::backend::allocator::dmabuf::Dmabuf,
        notifier: ImportNotifier,
    ) {
        use smithay::backend::allocator::Buffer;
        if self.dmabuf_formats.contains(&dmabuf.format()) {
            let _ = notifier.successful::<State>();
        } else {
            notifier.failed();
        }
    }
}

impl XdgShellHandler for State {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let zone = self.usable_zone();
        surface.with_pending_state(|state| {
            state.size = Some(zone.size);
            state.bounds = Some(zone.size);
            state.states.set(xdg_toplevel::State::Maximized);
        });
        let wl = surface.wl_surface().clone();
        self.space.map_element(Window::new_wayland_window(surface), zone.loc, true);
        self.focus(Some(wl));
        self.needs_redraw = true;
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        let window = self.space.elements().find(|w| w.toplevel() == Some(&surface)).cloned();
        if let Some(window) = window {
            self.space.unmap_elem(&window);
        }
        let next = self.space.elements().next_back().and_then(|w| w.toplevel().map(|t| t.wl_surface().clone()));
        self.focus(next);
        self.needs_redraw = true;
    }

    fn new_popup(&mut self, surface: PopupSurface, positioner: PositionerState) {
        surface.with_pending_state(|state| state.geometry = positioner.get_geometry());
        let _ = self.popups.track_popup(PopupKind::Xdg(surface));
    }

    fn reposition_request(&mut self, surface: PopupSurface, positioner: PositionerState, token: u32) {
        surface.with_pending_state(|state| {
            state.geometry = positioner.get_geometry();
            state.positioner = positioner;
        });
        surface.send_repositioned(token);
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: WlSeat, _serial: Serial) {}
}

impl XdgDecorationHandler for State {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| state.decoration_mode = Some(DecorationMode::ClientSide));
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, _mode: DecorationMode) {
        self.new_decoration(toplevel.clone());
        if toplevel.is_initial_configure_sent() {
            toplevel.send_pending_configure();
        }
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        self.request_mode(toplevel, DecorationMode::ClientSide);
    }
}

impl WlrLayerShellHandler for State {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: LayerSurface,
        output: Option<WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        let output = output.as_ref().and_then(Output::from_resource).unwrap_or_else(|| self.output.clone());
        let mapped = layer_map_for_output(&output).map_layer(&DesktopLayer::new(surface, namespace));
        if let Err(error) = mapped {
            tracing::warn!(%error, "layer surface not mapped");
        }
    }

    fn layer_destroyed(&mut self, surface: LayerSurface) {
        {
            let mut map = layer_map_for_output(&self.output);
            let layer = map.layers().find(|l| l.layer_surface() == &surface).cloned();
            if let Some(layer) = layer {
                map.unmap_layer(&layer);
            }
        }
        self.relayout();
    }
}

delegate_compositor!(State);
delegate_shm!(State);
delegate_output!(State);
delegate_seat!(State);
delegate_data_device!(State);
delegate_dmabuf!(State);
delegate_xdg_shell!(State);
delegate_xdg_decoration!(State);
delegate_layer_shell!(State);
delegate_presentation!(State);
delegate_viewporter!(State);
