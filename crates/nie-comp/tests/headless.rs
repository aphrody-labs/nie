//! End to end: a real Wayland client (wayland-client) connects to the in-process headless
//! compositor, maps a layer-shell panel and an xdg toplevel with shm buffers of known colours, and
//! the PNG screenshot of the whole output is checked pixel by pixel.
#![cfg(target_os = "linux")]

use std::fs::File;
use std::io::{Seek, Write};
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};
use nie_comp::{Config, Headless};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_buffer, wl_compositor, wl_registry, wl_shm, wl_shm_pool, wl_surface};
use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

const OUTPUT: (i32, i32) = (800, 600);
const PANEL_HEIGHT: u32 = 32;
const PANEL: [u8; 4] = [0xC8, 0x28, 0x28, 0xFF];
const TOPLEVEL: [u8; 4] = [0x20, 0x90, 0x40, 0xFF];

#[derive(Default)]
struct App {
    panel_size: Option<(u32, u32)>,
    toplevel_size: Option<(i32, i32)>,
    toplevel_configured: bool,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for App {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for App {
    fn event(
        _: &mut Self,
        base: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            base.pong(serial);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for App {
    fn event(
        app: &mut Self,
        surface: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            surface.ack_configure(serial);
            app.toplevel_configured = true;
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for App {
    fn event(
        app: &mut Self,
        _: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_toplevel::Event::Configure { width, height, .. } = event {
            app.toplevel_size = Some((width, height));
        }
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for App {
    fn event(
        app: &mut Self,
        surface: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_layer_surface_v1::Event::Configure { serial, width, height } = event {
            surface.ack_configure(serial);
            app.panel_size = Some((width, height));
        }
    }
}

delegate_noop!(App: ignore wl_compositor::WlCompositor);
delegate_noop!(App: ignore wl_shm::WlShm);
delegate_noop!(App: ignore wl_shm_pool::WlShmPool);
delegate_noop!(App: ignore wl_buffer::WlBuffer);
delegate_noop!(App: ignore wl_surface::WlSurface);
delegate_noop!(App: ignore zwlr_layer_shell_v1::ZwlrLayerShellV1);

/// A wl_shm buffer of `w` x `h` pixels filled with `rgba` (Argb8888); the file backs the pool.
fn solid_buffer(
    shm: &wl_shm::WlShm,
    qh: &QueueHandle<App>,
    w: i32,
    h: i32,
    rgba: [u8; 4],
) -> Result<(wl_buffer::WlBuffer, File)> {
    let [r, g, b, a] = rgba;
    let mut file = tempfile::tempfile()?;
    let pixel = [b, g, r, a];
    let row: Vec<u8> = pixel.iter().copied().cycle().take(w as usize * 4).collect();
    for _ in 0..h {
        file.write_all(&row)?;
    }
    file.flush()?;
    file.rewind()?;
    let pool = shm.create_pool(file.as_fd(), w * h * 4, qh, ());
    let buffer = pool.create_buffer(0, w, h, w * 4, wl_shm::Format::Argb8888, qh, ());
    pool.destroy();
    Ok((buffer, file))
}

struct ClientReport {
    toplevel_size: (i32, i32),
    globals: Vec<String>,
}

/// The client: panel (layer-shell, top, anchored top/left/right, exclusive zone) then toplevel.
fn run_client(socket: &Path, ready: Sender<ClientReport>, done: Receiver<()>) -> Result<()> {
    let conn = Connection::from_socket(UnixStream::connect(socket).context("connect")?)?;
    let (globals, mut queue) = registry_queue_init::<App>(&conn)?;
    let qh = queue.handle();
    let compositor: wl_compositor::WlCompositor = globals.bind(&qh, 1..=4, ())?;
    let shm: wl_shm::WlShm = globals.bind(&qh, 1..=1, ())?;
    let wm_base: xdg_wm_base::XdgWmBase = globals.bind(&qh, 1..=1, ())?;
    let layer_shell: zwlr_layer_shell_v1::ZwlrLayerShellV1 = globals.bind(&qh, 1..=1, ())?;
    let interfaces =
        globals.contents().with_list(|list| list.iter().map(|g| g.interface.clone()).collect::<Vec<_>>());
    let mut app = App::default();
    let mut keep = Vec::new();

    let panel = compositor.create_surface(&qh, ());
    let layer = layer_shell.get_layer_surface(
        &panel,
        None,
        zwlr_layer_shell_v1::Layer::Top,
        "nie-test-panel".into(),
        &qh,
        (),
    );
    layer.set_size(0, PANEL_HEIGHT);
    layer.set_anchor(
        zwlr_layer_surface_v1::Anchor::Top
            | zwlr_layer_surface_v1::Anchor::Left
            | zwlr_layer_surface_v1::Anchor::Right,
    );
    layer.set_exclusive_zone(PANEL_HEIGHT as i32);
    panel.commit();
    while app.panel_size.is_none() {
        queue.blocking_dispatch(&mut app)?;
    }
    let (pw, ph) = app.panel_size.context("panel configure")?;
    ensure!((pw, ph) == (OUTPUT.0 as u32, PANEL_HEIGHT), "panel configured {pw}x{ph}");
    let (buffer, file) = solid_buffer(&shm, &qh, pw as i32, ph as i32, PANEL)?;
    panel.attach(Some(&buffer), 0, 0);
    panel.commit();
    keep.push((buffer, file));
    queue.roundtrip(&mut app)?;

    let surface = compositor.create_surface(&qh, ());
    let xdg = wm_base.get_xdg_surface(&surface, &qh, ());
    let toplevel = xdg.get_toplevel(&qh, ());
    toplevel.set_title("nie-test-toplevel".into());
    toplevel.set_app_id("nie.comp.test".into());
    surface.commit();
    while !app.toplevel_configured {
        queue.blocking_dispatch(&mut app)?;
    }
    let (w, h) = app.toplevel_size.filter(|&(w, h)| w > 0 && h > 0).context("toplevel configure size")?;
    let (buffer, file) = solid_buffer(&shm, &qh, w, h, TOPLEVEL)?;
    surface.attach(Some(&buffer), 0, 0);
    surface.commit();
    keep.push((buffer, file));
    queue.roundtrip(&mut app)?;

    ready.send(ClientReport { toplevel_size: (w, h), globals: interfaces }).ok();
    done.recv_timeout(Duration::from_secs(30)).ok();
    drop(keep);
    Ok(())
}

#[test]
fn composites_layer_panel_and_toplevel_into_a_png() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let socket: PathBuf = dir.path().join("nie-test-0");
    let mut headless = Headless::new(&Config {
        socket: socket.to_string_lossy().into_owned(),
        size: OUTPUT,
        ..Config::default()
    })?;

    let (ready_tx, ready_rx) = channel();
    let (done_tx, done_rx) = channel();
    let path = headless.socket_path().to_path_buf();
    let client = thread::spawn(move || run_client(&path, ready_tx, done_rx));

    let deadline = Instant::now() + Duration::from_secs(30);
    let report = loop {
        headless.dispatch(Duration::from_millis(10))?;
        if let Ok(report) = ready_rx.try_recv() {
            break report;
        }
        if client.is_finished() {
            client.join().map_err(|_| anyhow::anyhow!("client panicked"))??;
            bail!("client ended before mapping its surfaces");
        }
        ensure!(Instant::now() < deadline, "client did not map its surfaces in time");
    };
    for _ in 0..5 {
        headless.dispatch(Duration::from_millis(10))?;
    }
    let shot = headless.screenshot()?;
    done_tx.send(()).ok();
    client.join().map_err(|_| anyhow::anyhow!("client panicked"))??;

    // Protocols advertised to clients.
    for interface in [
        "wl_compositor",
        "wl_shm",
        "wl_seat",
        "wl_output",
        "wl_data_device_manager",
        "xdg_wm_base",
        "zwlr_layer_shell_v1",
        "zxdg_output_manager_v1",
        "wp_presentation",
        "zwp_linux_dmabuf_v1",
        "wp_viewporter",
        "zxdg_decoration_manager_v1",
    ] {
        ensure!(report.globals.iter().any(|g| g == interface), "missing global {interface}");
    }

    // The toplevel got the output minus the panel's exclusive zone.
    let zone_height = OUTPUT.1 - PANEL_HEIGHT as i32;
    assert_eq!(report.toplevel_size, (OUTPUT.0, zone_height));

    // The screenshot survives a PNG round trip.
    let png_path = std::env::var_os("NIE_COMP_TEST_PNG")
        .map_or_else(|| dir.path().join("screenshot.png"), PathBuf::from);
    shot.write_png(&png_path)?;
    let decoder = png::Decoder::new(std::io::BufReader::new(File::open(&png_path)?));
    let mut reader = decoder.read_info()?;
    let mut pixels = vec![0; reader.output_buffer_size().context("png size")?];
    let info = reader.next_frame(&mut pixels)?;
    assert_eq!((info.width, info.height), (OUTPUT.0 as u32, OUTPUT.1 as u32));
    assert_eq!(info.color_type, png::ColorType::Rgba);
    let at = |x: u32, y: u32| {
        let i = ((y * info.width + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    };

    let h = PANEL_HEIGHT;
    for x in [0, 1, 400, 798, 799] {
        assert_eq!(at(x, 0), PANEL, "panel top row at x={x}");
        assert_eq!(at(x, h / 2), PANEL, "panel middle at x={x}");
        assert_eq!(at(x, h - 1), PANEL, "panel last row at x={x}");
        assert_eq!(at(x, h), TOPLEVEL, "first toplevel row at x={x}: exclusive zone respected");
        assert_eq!(at(x, 300), TOPLEVEL, "toplevel middle at x={x}");
        assert_eq!(at(x, 599), TOPLEVEL, "toplevel last row at x={x}");
    }
    assert_eq!(shot.pixel(400, 10), PANEL);
    assert_eq!(shot.pixel(400, 300), TOPLEVEL);
    Ok(())
}
