//! The window of `nie term`: a rio-window event loop, a sugarloaf renderer on wgpu, and the pty
//! child. The terminal state lives in [`TermModel`]; this file only moves events between the
//! window system, the model and the pty, and draws the model's frame.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use nie_explore::term::input::{MouseButton, MouseEvent, MouseKind};
use nie_explore::term::{Image, Mods, SelectKind, TermModel};
use rio_vt::clipboard::{Clipboard, ClipboardType};
use rio_window::application::ApplicationHandler;
use rio_window::dpi::{LogicalSize, PhysicalPosition};
use rio_window::event::{ElementState, Ime, MouseButton as WinButton, MouseScrollDelta, WindowEvent};
use rio_window::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use rio_window::keyboard::{Key, ModifiersState, NamedKey};
use rio_window::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use rio_window::window::{Window, WindowId};
use sugarloaf::components::core::image::Handle;
use sugarloaf::font::FontLibrary;
use sugarloaf::font::fonts::SugarloafFonts;
use sugarloaf::layout::RootStyle;
use sugarloaf::text::DrawOpts;
use sugarloaf::{
    Color, Colorspace, GraphicDataEntry, GraphicOverlay, Sugarloaf, SugarloafBackend, SugarloafRenderer, SugarloafWindow,
    SugarloafWindowSize,
};

use super::pty::{CommandSpec, PtyEvent, PtyHandle, PtySize};
use super::scene::{Canvas, Layout, Look, draw_frame};
use super::{TEXT_DY, keys};

#[derive(Debug)]
enum UserEvent {
    Wake,
}

/// Everything the window needs to start.
pub struct WindowOptions {
    pub cols: u16,
    pub rows: u16,
    pub font_size: f32,
    pub command: CommandSpec,
}

const CLICK_INTERVAL: Duration = Duration::from_millis(400);
const BLINK_INTERVAL: Duration = Duration::from_millis(530);

/// Sugarloaf as a drawing surface for the scene.
struct SugarCanvas<'a> {
    sugarloaf: &'a mut Sugarloaf<'static>,
    scale: f32,
    font_size: f32,
}

impl Canvas for SugarCanvas<'_> {
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, rgba: [f32; 4]) {
        let s = self.scale;
        self.sugarloaf.rect(None, x / s, y / s, w / s, h / s, rgba, 0.0, 0);
    }

    fn text(&mut self, x: f32, y: f32, text: &str, color: [u8; 4], bold: bool, italic: bool) {
        let s = self.scale;
        let opts = DrawOpts {
            font_size: self.font_size,
            color,
            bold,
            italic,
            font_id: None,
        };
        self.sugarloaf.text_mut().draw(x / s, (y + TEXT_DY * s) / s, text, &opts);
    }

    fn image(&mut self, image: &Arc<Image>, rect: [f32; 4], uv: [f32; 4], z_index: i32) {
        self.sugarloaf.push_image_overlay(
            0,
            GraphicOverlay {
                image_id: image.key,
                x: rect[0],
                y: rect[1],
                width: rect[2],
                height: rect[3],
                z_index,
                source_rect: uv,
            },
        );
    }
}

struct State {
    // Drop order matters: the renderer holds the window surface, so it goes before the window.
    sugarloaf: Sugarloaf<'static>,
    window: Window,
    model: TermModel,
    pty: PtyHandle,
    clipboard: Clipboard,
    uploaded: HashMap<u64, Arc<Image>>,
    cell_w: f32,
    cell_h: f32,
    scale: f32,
    font_size: f32,
    title: String,
}

struct App {
    opts: WindowOptions,
    proxy: EventLoopProxy<UserEvent>,
    state: Option<State>,
    mods: ModifiersState,
    cursor: PhysicalPosition<f64>,
    buttons: [bool; 3],
    focused: bool,
    last_click: Option<(Instant, usize, usize, u8)>,
    selecting: bool,
    scroll_remainder: f32,
    cursor_on: bool,
    next_blink: Instant,
    error: Option<anyhow::Error>,
}

/// The cell under a physical pixel position, clamped to the grid.
fn cell_at(state: &State, position: PhysicalPosition<f64>) -> (usize, usize) {
    let (cols, rows) = state.model.size();
    let col = (position.x as f32 / state.cell_w).floor().max(0.0) as usize;
    let row = (position.y as f32 / state.cell_h).floor().max(0.0) as usize;
    (col.min(cols - 1), row.min(rows - 1))
}

fn pty_size(cols: usize, rows: usize, cell_w: f32, cell_h: f32) -> PtySize {
    PtySize {
        cols: cols as u16,
        rows: rows as u16,
        width: (cols as f32 * cell_w) as u16,
        height: (rows as f32 * cell_h) as u16,
    }
}

impl State {
    fn grid_for(&self, width: u32, height: u32) -> (usize, usize) {
        let cols = (width as f32 / self.cell_w).floor().max(2.0) as usize;
        let rows = (height as f32 / self.cell_h).floor().max(1.0) as usize;
        (cols, rows)
    }

    fn remeasure(&mut self) {
        let (_, m) = self.sugarloaf.compute_cell_metrics(self.font_size, 1.0, self.scale);
        self.cell_w = m.cell_width.max(1) as f32;
        self.cell_h = m.cell_height.max(1) as f32;
    }

    fn apply_size(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.sugarloaf.resize(width, height);
        let (cols, rows) = self.grid_for(width, height);
        self.model.resize(cols, rows, self.cell_w as u32, self.cell_h as u32);
        self.pty.resize(pty_size(cols, rows, self.cell_w, self.cell_h));
        self.window.request_redraw();
    }

    fn upload_images(&mut self, placements: &[nie_explore::term::Placement]) {
        let mut live: HashMap<u64, Arc<Image>> = HashMap::new();
        for p in placements {
            live.insert(p.image.key, p.image.clone());
        }
        for (key, old) in &self.uploaded {
            if live.get(key).is_none_or(|new| !Arc::ptr_eq(new, old)) {
                self.sugarloaf.remove_image(*key);
            }
        }
        for (key, image) in &live {
            let fresh = self.uploaded.get(key).is_none_or(|old| !Arc::ptr_eq(old, image));
            if fresh {
                self.sugarloaf.image_data.insert(
                    *key,
                    GraphicDataEntry {
                        handle: Handle::from_pixels(image.width, image.height, image.rgba.clone()),
                        width: image.width as f32,
                        height: image.height as f32,
                        transmit_time: std::time::Instant::now(),
                    },
                );
            }
        }
        self.uploaded = live;
    }

    fn draw(&mut self, focused: bool, cursor_visible: bool) {
        let frame = self.model.frame();
        self.sugarloaf.clear_image_overlays_for(0);
        self.upload_images(&frame.placements);
        let [r, g, b] = frame.default_bg;
        self.sugarloaf.set_background_color(Some(Color {
            r: f64::from(r) / 255.0,
            g: f64::from(g) / 255.0,
            b: f64::from(b) / 255.0,
            a: 1.0,
        }));
        let layout = Layout {
            cell_w: self.cell_w,
            cell_h: self.cell_h,
            origin_x: 0.0,
            origin_y: 0.0,
        };
        let mut canvas = SugarCanvas {
            sugarloaf: &mut self.sugarloaf,
            scale: self.scale,
            font_size: self.font_size,
        };
        draw_frame(&mut canvas, &frame, &layout, Look { focused, cursor_visible });
        self.sugarloaf.render();
    }

    /// Feeds pending child output to the model and sends back what the terminal answers.
    /// Returns whether the child exited.
    fn pump(&mut self) -> bool {
        let mut exited = false;
        while let Ok(event) = self.pty.events.try_recv() {
            match event {
                PtyEvent::Output(bytes) => self.model.feed(&bytes),
                PtyEvent::Exited(_) => exited = true,
            }
        }
        let replies = self.model.take_replies();
        if !replies.is_empty() {
            self.pty.write(replies);
        }
        if let Some(text) = self.model.take_clipboard() {
            self.clipboard.set(ClipboardType::Clipboard, text);
        }
        let title = self.model.title();
        let title = if title.is_empty() { "nie term" } else { title };
        if title != self.title {
            self.title = title.to_string();
            self.window.set_title(&self.title);
        }
        exited
    }
}

impl App {
    fn create(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let font_size = self.opts.font_size;
        let (fonts, _missing) = FontLibrary::new(SugarloafFonts {
            size: font_size,
            ..SugarloafFonts::default()
        });
        // The window opens at the size of cols x rows cells at scale 1; the real scale is applied
        // once the window exists.
        let (_, m) = sugarloaf::layout::compute_cell_metrics(&fonts, font_size, 1.0, 1.0);
        let (cell_w, cell_h) = (m.cell_width.max(1) as f64, m.cell_height.max(1) as f64);
        let attrs = Window::default_attributes()
            .with_title("nie term")
            .with_inner_size(LogicalSize::new(
                cell_w * f64::from(self.opts.cols),
                cell_h * f64::from(self.opts.rows),
            ));
        let window = event_loop.create_window(attrs).map_err(|e| anyhow!("create window: {e}"))?;
        window.set_ime_allowed(true);
        let scale = window.scale_factor() as f32;
        let size = window.inner_size();
        let handle = window.window_handle().map_err(|e| anyhow!("window handle: {e}"))?.as_raw();
        let display = window.display_handle().map_err(|e| anyhow!("display handle: {e}"))?.as_raw();
        let renderer = SugarloafRenderer {
            backend: SugarloafBackend::Wgpu(sugarloaf::wgpu::Backends::PRIMARY),
            font_features: None,
            colorspace: Colorspace::Srgb,
            prefer_alpha_capable_adapter: false,
        };
        let sugarloaf = Sugarloaf::new(
            SugarloafWindow {
                handle,
                display,
                size: SugarloafWindowSize {
                    width: size.width as f32,
                    height: size.height as f32,
                },
                scale,
            },
            renderer,
            &fonts,
            RootStyle {
                scale_factor: scale,
                font_size,
                line_height: 1.0,
            },
        )
        .map_err(|e| anyhow!("sugarloaf: {e:?}"))?;

        let clipboard = std::panic::catch_unwind(Clipboard::default).unwrap_or_else(|_| Clipboard::new_nop());
        let proxy = self.proxy.clone();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            let _ = proxy.send_event(UserEvent::Wake);
        });
        let mut state = State {
            sugarloaf,
            window,
            model: TermModel::new(usize::from(self.opts.cols), usize::from(self.opts.rows)),
            pty: PtyHandle::spawn(&self.opts.command, PtySize { cols: self.opts.cols, rows: self.opts.rows, width: 0, height: 0 }, wake)?,
            clipboard,
            uploaded: HashMap::new(),
            cell_w: 8.0,
            cell_h: 16.0,
            scale,
            font_size,
            title: "nie term".into(),
        };
        state.remeasure();
        state.apply_size(size.width, size.height);
        state.window.request_redraw();
        self.state = Some(state);
        Ok(())
    }

    fn mods(&self) -> Mods {
        keys::mods(self.mods)
    }

    fn reset_blink(&mut self) {
        self.cursor_on = true;
        self.next_blink = Instant::now() + BLINK_INTERVAL;
    }

    fn paste(&mut self, ty: ClipboardType) {
        let Some(state) = self.state.as_mut() else { return };
        let text = state.clipboard.get(ty);
        if !text.is_empty() {
            let bytes = state.model.encode_paste(&text);
            state.pty.write(bytes);
        }
    }

    fn copy_selection(&mut self, ty: ClipboardType) {
        let Some(state) = self.state.as_mut() else { return };
        if let Some(text) = state.model.selection_text() {
            state.clipboard.set(ty, text);
        }
    }

    fn mouse_press(&mut self, button: WinButton, pressed: bool) {
        let mods = self.mods();
        let Some(state) = self.state.as_mut() else { return };
        let (col, row) = cell_at(state, self.cursor);
        let model_button = match button {
            WinButton::Left => MouseButton::Left,
            WinButton::Middle => MouseButton::Middle,
            WinButton::Right => MouseButton::Right,
            _ => return,
        };
        let idx = match model_button {
            MouseButton::Left => 0,
            MouseButton::Middle => 1,
            MouseButton::Right => 2,
        };
        self.buttons[idx] = pressed;
        // Shift hands the mouse back to the terminal for selection, as in xterm.
        if !mods.shift {
            let kind = if pressed {
                MouseKind::Press(model_button)
            } else {
                MouseKind::Release(model_button)
            };
            if let Some(bytes) = state.model.encode_mouse(&MouseEvent { kind, col, row, mods }) {
                state.pty.write(bytes);
                return;
            }
        }
        match (model_button, pressed) {
            (MouseButton::Left, true) => {
                let now = Instant::now();
                let count = match self.last_click {
                    Some((t, c, r, n)) if now.duration_since(t) < CLICK_INTERVAL && (c, r) == (col, row) => (n % 3) + 1,
                    _ => 1,
                };
                self.last_click = Some((now, col, row, count));
                let kind = match count {
                    1 => SelectKind::Cells,
                    2 => SelectKind::Word,
                    _ => SelectKind::Line,
                };
                if mods.alt && count == 1 {
                    state.model.select_begin(col, row, SelectKind::Block);
                } else {
                    state.model.select_begin(col, row, kind);
                }
                state.model.select_extend(col, row);
                self.selecting = true;
                state.window.request_redraw();
            }
            (MouseButton::Left, false) => {
                self.selecting = false;
                self.copy_selection(ClipboardType::Selection);
            }
            (MouseButton::Middle, true) => self.paste(ClipboardType::Selection),
            _ => {}
        }
    }

    fn wheel(&mut self, delta: MouseScrollDelta) {
        let mods = self.mods();
        let Some(state) = self.state.as_mut() else { return };
        let lines = match delta {
            MouseScrollDelta::LineDelta(_, y) => y * 3.0,
            MouseScrollDelta::PixelDelta(p) => p.y as f32 / state.cell_h,
        };
        self.scroll_remainder += lines;
        let whole = self.scroll_remainder.trunc() as i32;
        if whole == 0 {
            return;
        }
        self.scroll_remainder -= whole as f32;
        let (col, row) = cell_at(state, self.cursor);
        if !mods.shift
            && let Some(bytes) = state.model.encode_mouse(&MouseEvent {
                kind: MouseKind::Wheel { lines: whole },
                col,
                row,
                mods,
            })
        {
            for _ in 0..whole.unsigned_abs() {
                state.pty.write(bytes.clone());
            }
            return;
        }
        if let Some(bytes) = state.model.encode_alt_scroll(whole) {
            state.pty.write(bytes);
        } else {
            state.model.scroll_lines(whole);
            state.window.request_redraw();
        }
    }

    fn key(&mut self, event: &rio_window::event::KeyEvent) {
        let mods = self.mods();
        let Some(state) = self.state.as_mut() else { return };
        let pressed = event.state == ElementState::Pressed;
        // Window shortcuts: copy, paste and history paging never reach the child.
        if pressed {
            if let Key::Named(named) = &event.logical_key {
                if mods.shift && !mods.ctrl && matches!(named, NamedKey::PageUp | NamedKey::PageDown) {
                    state.model.scroll_page(matches!(named, NamedKey::PageUp));
                    state.window.request_redraw();
                    return;
                }
                if mods.shift && !mods.ctrl && *named == NamedKey::Insert {
                    self.paste(ClipboardType::Clipboard);
                    return;
                }
            }
            if mods.ctrl
                && mods.shift
                && let Key::Character(c) = &event.logical_key
            {
                match c.to_lowercase().as_str() {
                    "c" => {
                        self.copy_selection(ClipboardType::Clipboard);
                        return;
                    }
                    "v" => {
                        self.paste(ClipboardType::Clipboard);
                        return;
                    }
                    _ => {}
                }
            }
        }
        let Some(state) = self.state.as_mut() else { return };
        let Some(ev) = keys::key_event(event, mods) else { return };
        let bytes = state.model.encode_key(&ev);
        if bytes.is_empty() {
            return;
        }
        if pressed && state.model.display_offset() > 0 {
            state.model.scroll_to_bottom();
        }
        state.model.select_clear();
        state.pty.write(bytes);
        state.window.request_redraw();
        self.reset_blink();
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            if let Err(err) = self.create(event_loop) {
                self.error = Some(err);
                event_loop.exit();
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, _event: UserEvent) {
        let Some(state) = self.state.as_mut() else { return };
        if state.pump() {
            event_loop.exit();
            return;
        }
        state.window.request_redraw();
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let blinking = self.state.as_ref().is_some_and(|s| s.model.cursor().blinking) && self.focused;
        if blinking {
            let now = Instant::now();
            if now >= self.next_blink {
                self.cursor_on = !self.cursor_on;
                self.next_blink = now + BLINK_INTERVAL;
                if let Some(state) = &self.state {
                    state.window.request_redraw();
                }
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_blink));
        } else {
            self.cursor_on = true;
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                let (focused, on) = (self.focused, self.cursor_on);
                if let Some(state) = self.state.as_mut() {
                    state.window.pre_present_notify();
                    state.draw(focused, on);
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(state) = self.state.as_mut() {
                    state.apply_size(size.width, size.height);
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Some(state) = self.state.as_mut() {
                    state.scale = scale_factor as f32;
                    state.sugarloaf.rescale(state.scale);
                    state.remeasure();
                    let size = state.window.inner_size();
                    state.apply_size(size.width, size.height);
                }
            }
            WindowEvent::Focused(focused) => {
                self.focused = focused;
                self.reset_blink();
                if let Some(state) = self.state.as_mut() {
                    if let Some(bytes) = state.model.encode_focus(focused) {
                        state.pty.write(bytes);
                    }
                    state.window.request_redraw();
                }
            }
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::KeyboardInput { event, is_synthetic, .. } => {
                if !is_synthetic {
                    self.key(&event);
                }
            }
            WindowEvent::Ime(Ime::Commit(text)) => {
                if let Some(state) = self.state.as_mut() {
                    state.pty.write(text.into_bytes());
                    state.window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = position;
                let mods = self.mods();
                let selecting = self.selecting;
                let held = if self.buttons[0] {
                    Some(MouseButton::Left)
                } else if self.buttons[1] {
                    Some(MouseButton::Middle)
                } else if self.buttons[2] {
                    Some(MouseButton::Right)
                } else {
                    None
                };
                if let Some(state) = self.state.as_mut() {
                    let (col, row) = cell_at(state, position);
                    if !mods.shift
                        && let Some(bytes) = state.model.encode_mouse(&MouseEvent {
                            kind: MouseKind::Move(held),
                            col,
                            row,
                            mods,
                        })
                    {
                        state.pty.write(bytes);
                    } else if selecting {
                        state.model.select_extend(col, row);
                        state.window.request_redraw();
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => self.mouse_press(button, state == ElementState::Pressed),
            WindowEvent::MouseWheel { delta, .. } => self.wheel(delta),
            _ => {}
        }
    }
}

/// Opens the window and runs until it closes or the child exits.
pub fn run(opts: WindowOptions) -> Result<()> {
    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .map_err(|e| anyhow!("event loop: {e}"))?;
    let mut app = App {
        opts,
        proxy: event_loop.create_proxy(),
        state: None,
        mods: ModifiersState::default(),
        cursor: PhysicalPosition::new(0.0, 0.0),
        buttons: [false; 3],
        focused: true,
        last_click: None,
        selecting: false,
        scroll_remainder: 0.0,
        cursor_on: true,
        next_blink: Instant::now() + BLINK_INTERVAL,
        error: None,
    };
    event_loop.run_app(&mut app).map_err(|e| anyhow!("event loop: {e}"))?;
    match app.error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}
