//! The renderer-independent model of the nie terminal (`nie term`).
//!
//! [`TermModel`] is fed with the bytes a child writes to its pty and exposes what a renderer
//! draws: a grid of [`Cell`]s with resolved colours, the [`Cursor`], the scrollback viewport, the
//! title and the image placements (kitty graphics, sixel and iTerm2) as RGBA with their cell
//! rectangles. The inverse direction, [`input`], encodes keys, mouse events, pastes and focus to
//! the bytes the child expects (xterm sequences, kitty keyboard protocol).
//!
//! The parser and the grid are `rio-vt` (Rio's embeddable terminal core). Nothing here depends on
//! a window system or a GPU API, so the same model backs the rio-window and sugarloaf window of
//! `nie-tui` and a later GPUI view.

pub mod input;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rio_graphics::{ColorType, GraphicData, atlas_image_key, kitty_image_key};
use rio_vt::ansi::CursorShape;
use rio_vt::ansi::graphics::{
    OverlayViewport, UpdateQueues, atlas_overlay_geometry, kitty_overlay_geometry,
};
use rio_vt::config::colors::term::List;
use rio_vt::config::colors::{AnsiColor, ColorRgb, Colors, NamedColor};
use rio_vt::crosswords::grid::Scroll;
use rio_vt::crosswords::pos::{Column, Line, Pos, Side};
use rio_vt::crosswords::square::Wide;
use rio_vt::crosswords::style::StyleFlags;
use rio_vt::crosswords::{Crosswords, CrosswordsSize, Mode};
use rio_vt::event::{EventListener, RioEvent, WindowId, WindowSize};
use rio_vt::performer::handler::Processor;
use rio_vt::selection::{Selection, SelectionType};

pub use input::{
    InputModes, KeyCode, KeyEvent, KeyState, Mods, MouseButton, MouseEvent, MouseKind, MouseMode,
};

/// An sRGB colour.
pub type Rgb = [u8; 3];

/// Lines of scrollback kept by default.
pub const DEFAULT_SCROLLBACK: usize = 10_000;

/// Cell attributes, one bit each.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Attrs(pub u16);

impl Attrs {
    pub const BOLD: u16 = 1;
    pub const ITALIC: u16 = 1 << 1;
    pub const DIM: u16 = 1 << 2;
    pub const UNDERLINE: u16 = 1 << 3;
    pub const DOUBLE_UNDERLINE: u16 = 1 << 4;
    pub const CURLY_UNDERLINE: u16 = 1 << 5;
    pub const DOTTED_UNDERLINE: u16 = 1 << 6;
    pub const DASHED_UNDERLINE: u16 = 1 << 7;
    pub const STRIKEOUT: u16 = 1 << 8;
    pub const BLINK: u16 = 1 << 9;
    /// The cell is part of a hyperlink.
    pub const LINK: u16 = 1 << 10;

    /// Whether every bit of `flag` is set.
    pub fn has(self, flag: u16) -> bool {
        self.0 & flag == flag
    }

    /// Whether the cell has any underline.
    pub fn underlined(self) -> bool {
        self.0
            & (Self::UNDERLINE
                | Self::DOUBLE_UNDERLINE
                | Self::CURLY_UNDERLINE
                | Self::DOTTED_UNDERLINE
                | Self::DASHED_UNDERLINE)
            != 0
    }
}

/// One cell of the viewport. Inverse video and hidden text are already applied to the colours.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    /// The base character (a space for an empty cell).
    pub ch: char,
    /// Zero-width marks that combine with `ch`.
    pub marks: Vec<char>,
    pub fg: Rgb,
    pub bg: Rgb,
    pub attrs: Attrs,
    /// Columns the glyph covers: 1, 2 for the first half of a wide glyph, 0 for its second half.
    pub width: u8,
    /// The cell is inside the selection.
    pub selected: bool,
}

/// The cursor shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorStyle {
    Block,
    Underline,
    Beam,
    Hidden,
}

/// The cursor in viewport coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursor {
    pub col: usize,
    pub row: usize,
    pub style: CursorStyle,
    pub blinking: bool,
    pub color: Rgb,
}

/// Decoded picture data, RGBA8 straight alpha, shared between frames.
#[derive(Debug, PartialEq, Eq)]
pub struct Image {
    /// Stable key: kitty ids map to themselves, sixel and iTerm2 graphics live above `1 << 32`.
    pub key: u64,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// An image drawn over the grid.
#[derive(Clone, Debug)]
pub struct Placement {
    pub image: Arc<Image>,
    /// Cell rectangle of the placement, clipped to the viewport.
    pub col: i32,
    pub row: i32,
    pub cols: u32,
    pub rows: u32,
    /// Pixel rectangle from the top left of the grid, clipped to it: x, y, width, height.
    pub rect: [f32; 4],
    /// Normalised source rectangle `[u0, v0, u1, v1]` inside the image.
    pub uv: [f32; 4],
    /// Stacking order; negative draws under the text.
    pub z_index: i32,
}

/// How a selection grows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectKind {
    Cells,
    Word,
    Line,
    Block,
}

/// A frame of the terminal: everything a renderer needs, copied out of the state machine.
#[derive(Clone, Debug)]
pub struct Frame {
    pub cols: usize,
    pub rows: usize,
    /// `rows` rows of `cols` cells.
    pub cells: Vec<Vec<Cell>>,
    pub cursor: Cursor,
    pub title: String,
    pub default_fg: Rgb,
    pub default_bg: Rgb,
    pub placements: Vec<Placement>,
    pub display_offset: usize,
    pub history: usize,
}

#[derive(Default)]
struct Shared {
    events: Vec<RioEvent>,
}

/// Collects what the state machine reports while it parses; the model drains it after each feed.
#[derive(Clone, Default)]
struct Sink(Arc<Mutex<Shared>>);

impl Sink {
    fn push(&self, event: RioEvent) {
        if let Ok(mut shared) = self.0.lock() {
            shared.events.push(event);
        }
    }

    fn drain(&self) -> Vec<RioEvent> {
        self.0
            .lock()
            .map(|mut shared| std::mem::take(&mut shared.events))
            .unwrap_or_default()
    }
}

impl EventListener for Sink {
    fn send_event(&self, event: RioEvent, _id: WindowId) {
        self.push(event);
    }

    fn send_event_with_high_priority(&self, event: RioEvent, _id: WindowId) {
        self.push(event);
    }
}

/// The terminal state machine and everything around it that is not drawing.
pub struct TermModel {
    term: Crosswords<Sink>,
    parser: Processor,
    sink: Sink,
    palette: List,
    cols: usize,
    rows: usize,
    cell_w: u32,
    cell_h: u32,
    images: HashMap<u64, Arc<Image>>,
    replies: Vec<u8>,
    clipboard: Option<String>,
    bells: u32,
    selection_origin: Option<(Pos, SelectKind)>,
}

fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn arr_rgb(c: [f32; 4]) -> Rgb {
    [to_u8(c[0]), to_u8(c[1]), to_u8(c[2])]
}

impl TermModel {
    /// A terminal of `cols` x `rows` cells with the default scrollback and an 8x16 pixel cell.
    pub fn new(cols: usize, rows: usize) -> Self {
        Self::with_scrollback(cols, rows, DEFAULT_SCROLLBACK)
    }

    /// A terminal with `scrollback` lines of history.
    pub fn with_scrollback(cols: usize, rows: usize, scrollback: usize) -> Self {
        let (cols, rows) = (cols.max(2), rows.max(1));
        let sink = Sink::default();
        let size = CrosswordsSize::new_with_dimensions(cols, rows, cols as u32 * 8, rows as u32 * 16, 8, 16);
        let term = Crosswords::new(
            size,
            CursorShape::Block,
            sink.clone(),
            WindowId::from(0),
            0,
            scrollback,
        );
        Self {
            term,
            parser: Processor::default(),
            sink,
            palette: List::from(&Colors::default()),
            cols,
            rows,
            cell_w: 8,
            cell_h: 16,
            images: HashMap::new(),
            replies: Vec::new(),
            clipboard: None,
            bells: 0,
            selection_origin: None,
        }
    }

    /// Feeds bytes read from the pty.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
        self.pump();
    }

    /// Resizes the grid and tells the state machine the pixel size of a cell, which sixel and
    /// kitty images need to size themselves in cells.
    pub fn resize(&mut self, cols: usize, rows: usize, cell_w: u32, cell_h: u32) {
        let (cols, rows) = (cols.max(2), rows.max(1));
        let (cell_w, cell_h) = (cell_w.max(1), cell_h.max(1));
        self.cols = cols;
        self.rows = rows;
        self.cell_w = cell_w;
        self.cell_h = cell_h;
        self.term.resize(CrosswordsSize::new_with_dimensions(
            cols,
            rows,
            cols as u32 * cell_w,
            rows as u32 * cell_h,
            cell_w,
            cell_h,
        ));
        self.selection_origin = None;
        self.pump();
    }

    /// Columns and rows.
    pub fn size(&self) -> (usize, usize) {
        (self.cols, self.rows)
    }

    /// Pixel size of one cell as last given to [`Self::resize`].
    pub fn cell_size(&self) -> (u32, u32) {
        (self.cell_w, self.cell_h)
    }

    /// The window title the child set (empty when it set none).
    pub fn title(&self) -> &str {
        &self.term.title
    }

    /// Default foreground and background.
    pub fn default_colors(&self) -> (Rgb, Rgb) {
        (self.color_index(NamedColor::Foreground as usize), self.color_index(NamedColor::Background as usize))
    }

    /// Bytes the terminal answers to the child (device attributes, cursor reports, colour and
    /// size queries, kitty graphics replies). The caller writes them to the pty.
    pub fn take_replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.replies)
    }

    /// Text the child stored in the clipboard through OSC 52.
    pub fn take_clipboard(&mut self) -> Option<String> {
        self.clipboard.take()
    }

    /// Bells rung since the last call.
    pub fn take_bells(&mut self) -> u32 {
        std::mem::take(&mut self.bells)
    }

    /// The modes that decide how input is encoded.
    pub fn input_modes(&self) -> InputModes {
        let mode = self.term.mode();
        let mouse = if mode.contains(Mode::MOUSE_MOTION) {
            MouseMode::Motion
        } else if mode.contains(Mode::MOUSE_DRAG) {
            MouseMode::Drag
        } else if mode.contains(Mode::MOUSE_REPORT_CLICK) {
            MouseMode::Click
        } else if mode.contains(Mode::MOUSE_REPORT_X10) {
            MouseMode::X10
        } else {
            MouseMode::Off
        };
        InputModes {
            app_cursor: mode.contains(Mode::APP_CURSOR),
            kitty_flags: self.term.keyboard_mode().bits(),
            mouse,
            sgr_mouse: mode.contains(Mode::SGR_MOUSE),
            utf8_mouse: mode.contains(Mode::UTF8_MOUSE),
            bracketed_paste: mode.contains(Mode::BRACKETED_PASTE),
            focus_events: mode.contains(Mode::FOCUS_IN_OUT),
            alt_screen: mode.contains(Mode::ALT_SCREEN),
            alternate_scroll: mode.contains(Mode::ALTERNATE_SCROLL),
            newline_mode: mode.contains(Mode::LINE_FEED_NEW_LINE),
        }
    }

    /// Bytes for a key event.
    pub fn encode_key(&self, ev: &KeyEvent) -> Vec<u8> {
        input::encode_key(&self.input_modes(), ev)
    }

    /// Bytes for a mouse event, `None` when the child does not track the mouse.
    pub fn encode_mouse(&self, ev: &MouseEvent) -> Option<Vec<u8>> {
        input::encode_mouse(&self.input_modes(), ev)
    }

    /// Bytes for a paste.
    pub fn encode_paste(&self, text: &str) -> Vec<u8> {
        input::encode_paste(&self.input_modes(), text)
    }

    /// Bytes for a focus change, when the child wants them.
    pub fn encode_focus(&self, focused: bool) -> Option<Vec<u8>> {
        input::encode_focus(&self.input_modes(), focused)
    }

    /// Bytes for `lines` of wheel on the alternate screen when the child has not claimed the mouse.
    pub fn encode_alt_scroll(&self, lines: i32) -> Option<Vec<u8>> {
        input::encode_alt_scroll(&self.input_modes(), lines)
    }

    /// Scrolls the viewport through the history, positive lines go up.
    pub fn scroll_lines(&mut self, lines: i32) {
        self.term.scroll_display(Scroll::Delta(lines));
    }

    /// Scrolls one page up (`true`) or down.
    pub fn scroll_page(&mut self, up: bool) {
        self.term.scroll_display(if up { Scroll::PageUp } else { Scroll::PageDown });
    }

    /// Back to the live screen.
    pub fn scroll_to_bottom(&mut self) {
        self.term.scroll_display(Scroll::Bottom);
    }

    /// Lines the viewport is scrolled up from the live screen.
    pub fn display_offset(&self) -> usize {
        self.term.display_offset()
    }

    /// Lines of history above the live screen.
    pub fn history_size(&self) -> usize {
        self.term.history_size()
    }

    /// Starts a selection at a viewport cell.
    pub fn select_begin(&mut self, col: usize, row: usize, kind: SelectKind) {
        let origin = self.viewport_pos(col, row);
        let ty = match kind {
            SelectKind::Cells => SelectionType::Simple,
            SelectKind::Word => SelectionType::Semantic,
            SelectKind::Line => SelectionType::Lines,
            SelectKind::Block => SelectionType::Block,
        };
        self.term.selection = Some(Selection::new(ty, origin, Side::Left));
        self.selection_origin = Some((origin, kind));
    }

    /// Extends the selection to a viewport cell.
    pub fn select_extend(&mut self, col: usize, row: usize) {
        let Some((origin, kind)) = self.selection_origin else {
            return;
        };
        let point = self.viewport_pos(col, row);
        let ty = match kind {
            SelectKind::Cells => SelectionType::Simple,
            SelectKind::Word => SelectionType::Semantic,
            SelectKind::Line => SelectionType::Lines,
            SelectKind::Block => SelectionType::Block,
        };
        let forward = point >= origin;
        let mut selection = Selection::new(ty, origin, if forward { Side::Left } else { Side::Right });
        selection.update(point, if forward { Side::Right } else { Side::Left });
        self.term.selection = Some(selection);
    }

    /// Drops the selection.
    pub fn select_clear(&mut self) {
        self.term.selection = None;
        self.selection_origin = None;
    }

    /// The selected text.
    pub fn selection_text(&self) -> Option<String> {
        self.term.selection_to_string().filter(|s| !s.is_empty())
    }

    fn viewport_pos(&self, col: usize, row: usize) -> Pos {
        let row = row.min(self.rows - 1);
        let col = col.min(self.cols - 1);
        Pos::new(Line(row as i32 - self.term.display_offset() as i32), Column(col))
    }

    fn color_index(&self, idx: usize) -> Rgb {
        match self.term.colors()[idx] {
            Some(c) => arr_rgb(c),
            None => arr_rgb(self.palette[idx]),
        }
    }

    fn resolve(&self, color: AnsiColor) -> Rgb {
        match color {
            AnsiColor::Named(n) => self.color_index(n as usize),
            AnsiColor::Indexed(i) => self.color_index(usize::from(i)),
            AnsiColor::Spec(ColorRgb { r, g, b }) => [r, g, b],
        }
    }

    fn pump(&mut self) {
        for event in self.sink.drain() {
            match event {
                RioEvent::PtyWrite(_, text) => self.replies.extend_from_slice(text.as_bytes()),
                RioEvent::TextAreaSizeRequest(_, format) => {
                    let text = format(WindowSize {
                        rows: self.rows as u16,
                        cols: self.cols as u16,
                        width: (self.cols as u32 * self.cell_w).min(u32::from(u16::MAX)) as u16,
                        height: (self.rows as u32 * self.cell_h).min(u32::from(u16::MAX)) as u16,
                    });
                    self.replies.extend_from_slice(text.as_bytes());
                }
                RioEvent::ColorRequest(_, idx, format) => {
                    let [r, g, b] = self.color_index(idx.min(rio_vt::config::colors::term::COUNT - 1));
                    self.replies.extend_from_slice(format(ColorRgb { r, g, b }).as_bytes());
                }
                RioEvent::UpdateGraphics { queues, .. } => self.apply_graphics(queues),
                RioEvent::ClipboardStore(_, text) => self.clipboard = Some(text),
                RioEvent::Bell(_) => self.bells += 1,
                _ => {}
            }
        }
    }

    fn apply_graphics(&mut self, queues: UpdateQueues) {
        for data in queues.pending {
            let key = atlas_image_key(data.id.get());
            if let Some(image) = to_image(key, data) {
                self.images.insert(key, Arc::new(image));
            }
        }
        for key in queues.remove_queue {
            self.images.remove(&key);
        }
        for (id, data) in queues.pending_images {
            let key = kitty_image_key(id);
            if let Some(image) = to_image(key, data) {
                self.images.insert(key, Arc::new(image));
            }
        }
    }

    /// The images currently drawn over the grid, in the viewport, lowest `z_index` first.
    pub fn placements(&self) -> Vec<Placement> {
        let graphics = &self.term.graphics;
        if graphics.kitty_placements.is_empty() && graphics.atlas_placements.is_empty() {
            return Vec::new();
        }
        let (cw, ch) = (self.cell_w as f32, self.cell_h as f32);
        let viewport = OverlayViewport {
            cell_width: cw,
            cell_height: ch,
            origin_x: 0.0,
            origin_y: 0.0,
            history_size: self.term.grid.lines_evicted() as i64 + self.term.history_size() as i64,
            display_offset: self.term.display_offset() as i64,
            screen_lines: self.rows as i64,
        };
        let mut out = Vec::new();
        let mut push = |image: &Arc<Image>, g: [f32; 4], uv: [f32; 4], z_index: i32| {
            // Clip to the grid like text is clipped.
            let (gx1, gy1) = (self.cols as f32 * cw, self.rows as f32 * ch);
            let (x0, y0) = (g[0].max(0.0), g[1].max(0.0));
            let (x1, y1) = ((g[0] + g[2]).min(gx1), (g[1] + g[3]).min(gy1));
            if x1 <= x0 || y1 <= y0 || g[2] <= 0.0 || g[3] <= 0.0 {
                return;
            }
            let (fx0, fx1) = ((x0 - g[0]) / g[2], (x1 - g[0]) / g[2]);
            let (fy0, fy1) = ((y0 - g[1]) / g[3], (y1 - g[1]) / g[3]);
            let uv = [
                uv[0] + (uv[2] - uv[0]) * fx0,
                uv[1] + (uv[3] - uv[1]) * fy0,
                uv[0] + (uv[2] - uv[0]) * fx1,
                uv[1] + (uv[3] - uv[1]) * fy1,
            ];
            let col = (x0 / cw).floor() as i32;
            let row = (y0 / ch).floor() as i32;
            out.push(Placement {
                image: image.clone(),
                col,
                row,
                cols: ((x1 / cw).ceil() as i32 - col) as u32,
                rows: ((y1 / ch).ceil() as i32 - row) as u32,
                rect: [x0, y0, x1 - x0, y1 - y0],
                uv,
                z_index,
            });
        };
        for p in &graphics.atlas_placements {
            let Some(image) = self.images.get(&p.image_key) else {
                continue;
            };
            if let Some(g) = atlas_overlay_geometry(p, &viewport) {
                push(image, [g.x, g.y, g.width, g.height], g.source_rect, -1);
            }
        }
        for p in graphics.kitty_placements.values() {
            let Some(image) = self.images.get(&kitty_image_key(p.image_id)) else {
                continue;
            };
            if let Some(g) = kitty_overlay_geometry(p, image.width as usize, image.height as usize, &viewport) {
                push(image, [g.x, g.y, g.width, g.height], g.source_rect, p.z_index);
            }
        }
        out.sort_by_key(|p| p.z_index);
        out
    }

    /// The cursor in viewport coordinates.
    pub fn cursor(&self) -> Cursor {
        let state = self.term.cursor();
        let row = state.pos.row.0.clamp(0, self.rows as i32 - 1);
        Cursor {
            col: state.pos.col.0.min(self.cols - 1),
            row: row as usize,
            style: match state.content {
                CursorShape::Block => CursorStyle::Block,
                CursorShape::Underline => CursorStyle::Underline,
                CursorShape::Beam => CursorStyle::Beam,
                CursorShape::Hidden => CursorStyle::Hidden,
            },
            blinking: self.term.blinking_cursor,
            color: self.color_index(NamedColor::Cursor as usize),
        }
    }

    /// Copies the viewport out of the state machine.
    pub fn frame(&self) -> Frame {
        let grid = &self.term.grid;
        let offset = self.term.display_offset() as i32;
        let (default_fg, default_bg) = self.default_colors();
        let selection = self.term.selection.as_ref().and_then(|s| s.to_range(&self.term));
        let mut cells = Vec::with_capacity(self.rows);
        for r in 0..self.rows {
            let line = Line(r as i32 - offset);
            let row = &grid[line];
            let mut out = Vec::with_capacity(self.cols);
            for c in 0..self.cols {
                let pos = Pos::new(line, Column(c));
                let sq = row[Column(c)];
                let style = grid.style_of(&sq);
                let mut fg = self.resolve(style.fg);
                let mut bg = self.resolve(style.bg);
                let flags = style.flags;
                if flags.contains(StyleFlags::INVERSE) {
                    std::mem::swap(&mut fg, &mut bg);
                }
                if flags.contains(StyleFlags::DIM) {
                    fg = fg.map(|v| (f32::from(v) * 0.66) as u8);
                }
                let mut attrs = 0u16;
                let mut set = |on: bool, bit: u16| {
                    if on {
                        attrs |= bit;
                    }
                };
                set(flags.contains(StyleFlags::BOLD), Attrs::BOLD);
                set(flags.contains(StyleFlags::ITALIC), Attrs::ITALIC);
                set(flags.contains(StyleFlags::DIM), Attrs::DIM);
                set(flags.contains(StyleFlags::UNDERLINE), Attrs::UNDERLINE);
                set(flags.contains(StyleFlags::DOUBLE_UNDERLINE), Attrs::DOUBLE_UNDERLINE);
                set(flags.contains(StyleFlags::UNDERCURL), Attrs::CURLY_UNDERLINE);
                set(flags.contains(StyleFlags::DOTTED_UNDERLINE), Attrs::DOTTED_UNDERLINE);
                set(flags.contains(StyleFlags::DASHED_UNDERLINE), Attrs::DASHED_UNDERLINE);
                set(flags.contains(StyleFlags::STRIKEOUT), Attrs::STRIKEOUT);
                set(flags.intersects(StyleFlags::ALL_BLINK), Attrs::BLINK);
                set(sq.has_hyperlink(), Attrs::LINK);
                let width = match sq.wide() {
                    Wide::Narrow => 1,
                    Wide::Wide => 2,
                    Wide::Spacer | Wide::LeadingSpacer => 0,
                };
                let (mut ch, mut marks) = (sq.c(), Vec::new());
                if sq.has_grapheme() {
                    let mut text = grid.cell_text(pos);
                    ch = text.next().unwrap_or(' ');
                    marks = text.collect();
                }
                if flags.contains(StyleFlags::HIDDEN) || ch == '\0' {
                    ch = ' ';
                    marks.clear();
                }
                let selected = selection.is_some_and(|s| s.contains(pos));
                out.push(Cell {
                    ch,
                    marks,
                    fg,
                    bg,
                    attrs: Attrs(attrs),
                    width,
                    selected,
                });
            }
            cells.push(out);
        }
        Frame {
            cols: self.cols,
            rows: self.rows,
            cells,
            cursor: self.cursor(),
            title: self.term.title.clone(),
            default_fg,
            default_bg,
            placements: self.placements(),
            display_offset: self.term.display_offset(),
            history: self.term.history_size(),
        }
    }

    /// The text of one viewport row, trailing blanks trimmed. Wide glyph halves count once.
    pub fn row_text(&self, row: usize) -> String {
        let line = Line(row as i32 - self.term.display_offset() as i32);
        let mut s = String::new();
        for c in 0..self.cols {
            let pos = Pos::new(line, Column(c));
            if matches!(self.term.grid[pos].wide(), Wide::Spacer | Wide::LeadingSpacer) {
                continue;
            }
            let mut any = false;
            for ch in self.term.grid.cell_text(pos) {
                s.push(if ch == '\0' { ' ' } else { ch });
                any = true;
            }
            if !any {
                s.push(' ');
            }
        }
        s.truncate(s.trim_end().len());
        s
    }
}

fn to_image(key: u64, data: GraphicData) -> Option<Image> {
    let (w, h) = (data.width, data.height);
    if w == 0 || h == 0 {
        return None;
    }
    let rgba = match data.color_type {
        ColorType::Rgba => data.pixels,
        ColorType::Rgb => {
            let mut out = Vec::with_capacity(w * h * 4);
            for px in data.pixels.chunks_exact(3) {
                out.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
            out
        }
    };
    if rgba.len() < w * h * 4 {
        return None;
    }
    Some(Image {
        key,
        width: w as u32,
        height: h as u32,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(cols: usize, rows: usize) -> TermModel {
        let mut m = TermModel::new(cols, rows);
        m.resize(cols, rows, 8, 16);
        m
    }

    #[test]
    fn sgr_colours_and_attributes_reach_the_cells() {
        let mut m = model(20, 4);
        m.feed(b"\x1b[31mred\x1b[0m \x1b[1;4mbold\x1b[0m \x1b[38;2;10;20;30mrgb\x1b[0m \x1b[7mrev");
        let f = m.frame();
        let red = f.cells[0][0].clone();
        assert_eq!(red.ch, 'r');
        assert_ne!(red.fg, f.default_fg, "palette red differs from the default foreground");
        let plain = &f.cells[0][3];
        assert_eq!(plain.fg, f.default_fg);
        assert_eq!(plain.bg, f.default_bg);
        let bold = &f.cells[0][4];
        assert_eq!(bold.ch, 'b');
        assert!(bold.attrs.has(Attrs::BOLD) && bold.attrs.has(Attrs::UNDERLINE));
        let rgb = &f.cells[0][9];
        assert_eq!((rgb.ch, rgb.fg), ('r', [10, 20, 30]));
        let rev = &f.cells[0][13];
        assert_eq!((rev.ch, rev.fg, rev.bg), ('r', f.default_bg, f.default_fg), "inverse swaps");
        assert_eq!(m.row_text(0), "red bold rgb rev");
    }

    #[test]
    fn indexed_256_colours_come_from_the_cube() {
        let mut m = model(10, 2);
        m.feed(b"\x1b[48;5;196mx");
        let f = m.frame();
        assert_eq!(f.cells[0][0].bg, [255, 0, 0], "index 196 is the cube red");
    }

    #[test]
    fn cursor_movement_and_erase() {
        let mut m = model(10, 5);
        m.feed(b"ab\x1b[2;3Hc\x1b[1A\x1b[2Cd");
        let c = m.cursor();
        assert_eq!((c.row, c.col), (0, 6), "CUU then CUF from (1,3) writes d at column 5");
        assert_eq!(m.row_text(0), "ab   d");
        assert_eq!(m.row_text(1), "  c");
        m.feed(b"\x1b[2J\x1b[H");
        assert_eq!(m.row_text(0), "");
        assert_eq!((m.cursor().row, m.cursor().col), (0, 0));
        m.feed(b"\x1b[?25l");
        assert_eq!(m.cursor().style, CursorStyle::Hidden);
    }

    #[test]
    fn wide_glyphs_take_two_columns() {
        let mut m = model(10, 2);
        m.feed("a\u{4e2d}b".as_bytes());
        let f = m.frame();
        assert_eq!((f.cells[0][1].ch, f.cells[0][1].width), ('\u{4e2d}', 2));
        assert_eq!(f.cells[0][2].width, 0);
        assert_eq!(f.cells[0][3].ch, 'b');
        assert_eq!(m.row_text(0), "a\u{4e2d}b");
    }

    #[test]
    fn scrollback_viewport() {
        let mut m = model(10, 3);
        for i in 0..10 {
            m.feed(format!("line{i}\r\n").as_bytes());
        }
        assert_eq!(m.display_offset(), 0);
        assert!(m.history_size() >= 8, "history {}", m.history_size());
        assert_eq!(m.row_text(0), "line8");
        m.scroll_lines(3);
        assert_eq!(m.display_offset(), 3);
        assert_eq!(m.row_text(0), "line5");
        assert_eq!(m.frame().display_offset, 3);
        m.scroll_to_bottom();
        assert_eq!(m.row_text(0), "line8");
    }

    #[test]
    fn title_replies_and_clipboard() {
        let mut m = model(20, 3);
        m.feed(b"\x1b]0;nie fm\x07");
        assert_eq!(m.title(), "nie fm");
        m.feed(b"\x1b[5n");
        assert_eq!(m.take_replies(), b"\x1b[0n", "device status report");
        m.feed(b"\x1b[16t\x1b[14t");
        assert_eq!(m.take_replies(), b"\x1b[6;16;8t\x1b[4;48;160t", "cell size and text area size in pixels");
        m.feed(b"\x1b]52;c;aGVsbG8=\x07");
        assert_eq!(m.take_clipboard().as_deref(), Some("hello"));
        m.feed(b"\x07");
        assert_eq!(m.take_bells(), 1);
    }

    #[test]
    fn modes_drive_input_encoding() {
        let mut m = model(20, 3);
        assert_eq!(m.encode_key(&KeyEvent::press(KeyCode::Up, Mods::NONE)), b"\x1b[A");
        m.feed(b"\x1b[?1h");
        assert_eq!(m.encode_key(&KeyEvent::press(KeyCode::Up, Mods::NONE)), b"\x1bOA");
        assert!(m.encode_mouse(&MouseEvent {
            kind: MouseKind::Press(MouseButton::Left),
            col: 0,
            row: 0,
            mods: Mods::NONE
        })
        .is_none());
        m.feed(b"\x1b[?1000h\x1b[?1006h");
        let press = MouseEvent {
            kind: MouseKind::Press(MouseButton::Left),
            col: 1,
            row: 2,
            mods: Mods::NONE,
        };
        assert_eq!(m.encode_mouse(&press).unwrap(), b"\x1b[<0;2;3M");
        m.feed(b"\x1b[?2004h");
        assert_eq!(m.encode_paste("x"), b"\x1b[200~x\x1b[201~");
        // kitty keyboard protocol: push flags 1 (disambiguate).
        m.feed(b"\x1b[>1u");
        assert_eq!(m.input_modes().kitty_flags, 1);
        assert_eq!(m.encode_key(&KeyEvent::press(KeyCode::Escape, Mods::NONE)), b"\x1b[27u");
    }

    #[test]
    fn selection_copies_text() {
        let mut m = model(20, 3);
        m.feed(b"hello world");
        m.select_begin(0, 0, SelectKind::Cells);
        m.select_extend(4, 0);
        assert_eq!(m.selection_text().as_deref(), Some("hello"));
        assert!(m.frame().cells[0][2].selected);
        assert!(!m.frame().cells[0][6].selected);
        m.select_extend(0, 0);
        assert_eq!(m.selection_text().as_deref(), Some("h"));
        m.select_clear();
        assert!(m.selection_text().is_none());
    }

    #[test]
    fn kitty_graphics_transmit_makes_a_placement() {
        let mut m = model(20, 10);
        // 2x2 raw RGBA (f=32), transmit and display at the cursor.
        m.feed(b"ab\x1b_Gf=32,s=2,v=2,a=T;/wAA//8AAP//AAD//wAA/w==\x1b\\");
        let placements = m.placements();
        assert_eq!(placements.len(), 1, "one image placement");
        let p = &placements[0];
        assert_eq!((p.image.width, p.image.height), (2, 2));
        assert_eq!(p.image.rgba.len(), 16);
        assert_eq!((p.col, p.row), (2, 0), "placed at the cursor, after `ab`");
        assert_eq!(p.rect, [16.0, 0.0, 2.0, 2.0]);
        assert_eq!(p.uv, [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(m.frame().placements.len(), 1);
    }

    #[test]
    fn sixel_makes_a_placement() {
        let mut m = model(20, 10);
        // 4x12 red sixel: two bands of four columns, colour register 0 red.
        m.feed(b"\x1bPq\"1;1;4;12#0;2;100;0;0#0~~~~-#0~~~~\x1b\\");
        let placements = m.placements();
        assert_eq!(placements.len(), 1, "one sixel placement");
        let p = &placements[0];
        assert_eq!((p.image.width, p.image.height), (4, 12));
        assert!(p.image.key >= 1 << 32, "sixel graphics live above the kitty id space");
        assert_eq!(&p.image.rgba[..4], &[255, 0, 0, 255]);
        assert_eq!(p.rect[0], 0.0);
        assert_eq!(p.rect[2], 4.0);
    }

    #[test]
    fn resize_keeps_the_grid_consistent() {
        let mut m = model(10, 3);
        m.feed(b"abc");
        m.resize(30, 8, 9, 18);
        assert_eq!(m.size(), (30, 8));
        assert_eq!(m.cell_size(), (9, 18));
        assert_eq!(m.frame().cells.len(), 8);
        assert_eq!(m.frame().cells[0].len(), 30);
        assert_eq!(m.row_text(0), "abc");
    }
}
