//! Compositor state: the Wayland globals, the scene and the event loop that drives them.

use std::os::fd::AsFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use calloop::generic::Generic;
use calloop::signals::{Signal, Signals};
use calloop::{EventLoop, Interest, LoopHandle, LoopSignal, Mode, PostAction};
use smithay::backend::allocator::format::FormatSet;
use smithay::desktop::{PopupManager, Space, Window, WindowSurfaceType, layer_map_for_output};
use smithay::input::keyboard::XkbConfig;
use smithay::input::{Seat, SeatState};
use smithay::output::{Mode as OutputMode, Output, PhysicalProperties, Subpixel};
use smithay::reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Display, DisplayHandle, ListeningSocket};
use smithay::utils::{Clock, Logical, Monotonic, Point, Rectangle, Size, Transform};
use smithay::wayland::compositor::{CompositorClientState, CompositorState};
use smithay::wayland::dmabuf::{DmabufGlobal, DmabufState};
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::presentation::PresentationState;
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::shell::wlr_layer::{Layer, WlrLayerShellState};
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::viewporter::ViewporterState;
use tracing::info;

/// Compositor settings.
#[derive(Debug, Clone)]
pub struct Config {
    /// Socket name under `$XDG_RUNTIME_DIR`, or an absolute path when it contains a `/`.
    pub socket: String,
    /// Output size in pixels.
    pub size: (i32, i32),
    /// Output refresh rate in millihertz.
    pub refresh_mhz: i32,
    /// Spawn XWayland for X11 clients (feature `xwayland`).
    pub xwayland: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { socket: "nie-0".into(), size: (1920, 1080), refresh_mhz: 60_000, xwayland: false }
    }
}

/// Per-client data attached to every accepted connection.
#[derive(Default)]
pub struct ClientState {
    pub compositor: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client: ClientId) {}
    fn disconnected(&self, _client: ClientId, _reason: DisconnectReason) {}
}

/// Everything the Wayland handlers read and write.
pub struct State {
    pub dh: DisplayHandle,
    pub clock: Clock<Monotonic>,
    pub loop_signal: LoopSignal,
    pub loop_handle: LoopHandle<'static, State>,

    pub space: Space<Window>,
    pub output: Output,
    pub seat: Seat<State>,
    pub popups: PopupManager,
    pub pointer_location: Point<f64, Logical>,
    /// Set by every commit and layout change; cleared by the backend after a frame.
    pub needs_redraw: bool,
    pub frame_seq: u64,
    /// Set by SIGINT/SIGTERM once [`Compositor::quit_on_signals`] was called.
    pub quit: bool,

    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub layer_shell_state: WlrLayerShellState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<State>,
    pub data_device_state: DataDeviceState,
    pub presentation_state: PresentationState,
    pub xdg_decoration_state: XdgDecorationState,
    pub viewporter_state: ViewporterState,
    pub dmabuf_state: DmabufState,
    pub dmabuf_global: Option<DmabufGlobal>,
    pub dmabuf_formats: FormatSet,

    #[cfg(feature = "xwayland")]
    pub xwm: Option<smithay::xwayland::X11Wm>,
    #[cfg(feature = "xwayland")]
    pub xwayland_shell_state: smithay::wayland::xwayland_shell::XWaylandShellState,
    /// X display number once XWayland is ready.
    #[cfg(feature = "xwayland")]
    pub xdisplay: Option<u32>,
}

impl State {
    fn new(
        dh: DisplayHandle,
        loop_signal: LoopSignal,
        loop_handle: LoopHandle<'static, State>,
        config: &Config,
    ) -> Result<Self> {
        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(&dh, "nie-seat0");
        seat.add_keyboard(XkbConfig::default(), 200, 25).context("xkb keymap")?;
        seat.add_pointer();

        let output = Output::new(
            "nie-0".into(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "nie".into(),
                model: "nie-comp".into(),
            },
        );
        output.create_global::<State>(&dh);
        let mode = OutputMode { size: config.size.into(), refresh: config.refresh_mhz };
        output.change_current_state(
            Some(mode),
            Some(Transform::Normal),
            Some(smithay::output::Scale::Integer(1)),
            Some((0, 0).into()),
        );
        output.set_preferred(mode);
        let mut space = Space::default();
        space.map_output(&output, (0, 0));

        #[cfg(feature = "xwayland")]
        let dh_for_x = dh.clone();
        Ok(Self {
            clock: Clock::new(),
            space,
            popups: PopupManager::default(),
            pointer_location: Point::from((f64::from(config.size.0) / 2.0, f64::from(config.size.1) / 2.0)),
            needs_redraw: true,
            frame_seq: 0,
            quit: false,
            compositor_state: CompositorState::new::<State>(&dh),
            xdg_shell_state: XdgShellState::new::<State>(&dh),
            layer_shell_state: WlrLayerShellState::new::<State>(&dh),
            shm_state: ShmState::new::<State>(&dh, []),
            output_manager_state: OutputManagerState::new_with_xdg_output::<State>(&dh),
            data_device_state: DataDeviceState::new::<State>(&dh),
            // CLOCK_MONOTONIC, the clock of `Clock<Monotonic>`.
            presentation_state: PresentationState::new::<State>(&dh, 1),
            xdg_decoration_state: XdgDecorationState::new::<State>(&dh),
            viewporter_state: ViewporterState::new::<State>(&dh),
            dmabuf_state: DmabufState::new(),
            dmabuf_global: None,
            dmabuf_formats: FormatSet::default(),
            seat_state,
            seat,
            output,
            dh,
            loop_signal,
            loop_handle,
            #[cfg(feature = "xwayland")]
            xwm: None,
            #[cfg(feature = "xwayland")]
            xwayland_shell_state: smithay::wayland::xwayland_shell::XWaylandShellState::new::<State>(&dh_for_x),
            #[cfg(feature = "xwayland")]
            xdisplay: None,
        })
    }

    /// Advertise linux-dmabuf with the formats the active renderer can import.
    pub fn enable_dmabuf(&mut self, formats: FormatSet) {
        if self.dmabuf_global.is_none() && formats.iter().next().is_some() {
            self.dmabuf_global =
                Some(self.dmabuf_state.create_global::<State>(&self.dh, formats.iter().copied()));
            self.dmabuf_formats = formats;
        }
    }

    /// The part of the output the layer surfaces leave free (exclusive zones removed).
    pub fn usable_zone(&self) -> Rectangle<i32, Logical> {
        layer_map_for_output(&self.output).non_exclusive_zone()
    }

    /// Arrange the layer surfaces, then give every toplevel the free zone.
    pub fn relayout(&mut self) {
        layer_map_for_output(&self.output).arrange();
        let zone = self.usable_zone();
        let windows: Vec<Window> = self.space.elements().cloned().collect();
        for window in windows {
            if let Some(toplevel) = window.toplevel() {
                toplevel.with_pending_state(|state| {
                    state.size = Some(zone.size);
                    state.bounds = Some(zone.size);
                });
                if toplevel.is_initial_configure_sent() {
                    toplevel.send_pending_configure();
                }
            }
            self.space.map_element(window, zone.loc, false);
        }
        self.needs_redraw = true;
    }

    /// Change the output size (a resized nested window, a new DRM mode).
    pub fn set_output_size(&mut self, size: Size<i32, smithay::utils::Physical>) {
        let mode = OutputMode { size, refresh: self.output.current_mode().map_or(60_000, |m| m.refresh) };
        self.output.change_current_state(Some(mode), None, None, None);
        self.output.set_preferred(mode);
        self.relayout();
    }

    /// The surface, and its origin in output coordinates, under `pos`: overlay and top layers,
    /// then toplevels, then bottom and background layers.
    pub fn surface_under(&self, pos: Point<f64, Logical>) -> Option<(WlSurface, Point<f64, Logical>)> {
        let layers = layer_map_for_output(&self.output);
        let in_layer = |layer: Layer| {
            let surface = layers.layer_under(layer, pos)?;
            let geo = layers.layer_geometry(surface)?;
            let (wl, loc) = surface.surface_under(pos - geo.loc.to_f64(), WindowSurfaceType::ALL)?;
            Some((wl, (loc + geo.loc).to_f64()))
        };
        if let Some(hit) = in_layer(Layer::Overlay).or_else(|| in_layer(Layer::Top)) {
            return Some(hit);
        }
        if let Some((window, origin)) = self.space.element_under(pos) {
            let (wl, loc) = window.surface_under(pos - origin.to_f64(), WindowSurfaceType::ALL)?;
            return Some((wl, (loc + origin).to_f64()));
        }
        in_layer(Layer::Bottom).or_else(|| in_layer(Layer::Background))
    }

    /// Give the keyboard to `surface` (a toplevel or a layer surface that asked for it).
    pub fn focus(&mut self, surface: Option<WlSurface>) {
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        if let Some(keyboard) = self.seat.get_keyboard() {
            keyboard.set_focus(self, surface, serial);
        }
        let focused = self.seat.get_keyboard().and_then(|k| k.current_focus());
        for window in self.space.elements() {
            let active = window.toplevel().is_some_and(|t| focused.as_ref() == Some(t.wl_surface()));
            window.set_activated(active);
            if let Some(toplevel) = window.toplevel() {
                toplevel.send_pending_configure();
            }
        }
    }
}

/// The compositor: its state, the Wayland display and the calloop event loop.
pub struct Compositor {
    pub state: State,
    pub display: Display<State>,
    pub event_loop: EventLoop<'static, State>,
    socket_path: PathBuf,
}

impl Compositor {
    /// Create the globals, bind the listening socket and register it on the event loop.
    pub fn new(config: &Config) -> Result<Self> {
        let event_loop = EventLoop::<State>::try_new().context("calloop event loop")?;
        let display = Display::<State>::new().context("wayland display")?;
        let handle = event_loop.handle();
        let state = State::new(display.handle(), event_loop.get_signal(), handle.clone(), config)?;

        let (listener, socket_path) = if config.socket.contains('/') {
            let path = PathBuf::from(&config.socket);
            (ListeningSocket::bind_absolute(path.clone()), path)
        } else {
            let runtime = std::env::var_os("XDG_RUNTIME_DIR").context("XDG_RUNTIME_DIR is not set")?;
            (ListeningSocket::bind(&config.socket), PathBuf::from(runtime).join(&config.socket))
        };
        let listener = listener.with_context(|| format!("bind {}", socket_path.display()))?;
        handle
            .insert_source(Generic::new(listener, Interest::READ, Mode::Level), |_, listener, state| {
                while let Some(stream) = listener.accept()? {
                    state.dh.insert_client(stream, Arc::new(ClientState::default()))?;
                }
                Ok(PostAction::Continue)
            })
            .map_err(|e| anyhow::anyhow!("socket source: {e}"))?;
        // The display's poll fd only wakes the loop; requests are dispatched in `dispatch`.
        let wake = display.as_fd().try_clone_to_owned().context("display fd")?;
        handle
            .insert_source(Generic::new(wake, Interest::READ, Mode::Level), |_, _, _| Ok(PostAction::Continue))
            .map_err(|e| anyhow::anyhow!("display source: {e}"))?;

        info!(socket = %socket_path.display(), "wayland socket ready");
        #[allow(unused_mut, reason = "mutated only when the `xwayland` feature starts the window manager")]
        let mut comp = Self { state, display, event_loop, socket_path };
        if config.xwayland {
            #[cfg(feature = "xwayland")]
            crate::xwm::start(&mut comp)?;
            #[cfg(not(feature = "xwayland"))]
            anyhow::bail!("built without the `xwayland` feature");
        }
        Ok(comp)
    }

    /// Stop the serving loops (`State::quit`) on SIGINT and SIGTERM.
    pub fn quit_on_signals(&mut self) -> Result<()> {
        let signals = Signals::new(&[Signal::SIGINT, Signal::SIGTERM]).context("signals")?;
        self.event_loop
            .handle()
            .insert_source(signals, |_, _, state| state.quit = true)
            .map_err(|e| anyhow::anyhow!("signal source: {e}"))?;
        Ok(())
    }

    /// Path of the listening socket; `WAYLAND_DISPLAY` may be set to it.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Run the event loop once (at most `timeout`), then serve the connected clients.
    pub fn dispatch(&mut self, timeout: Option<Duration>) -> Result<()> {
        self.event_loop.dispatch(timeout, &mut self.state).context("event loop")?;
        self.display.dispatch_clients(&mut self.state).context("dispatch clients")?;
        self.state.popups.cleanup();
        self.state.space.refresh();
        self.flush()
    }

    /// Send the queued events to the clients.
    pub fn flush(&mut self) -> Result<()> {
        self.display.flush_clients().context("flush clients")
    }
}
