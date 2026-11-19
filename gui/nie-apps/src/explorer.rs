//! The explorer on the `nie_explore::fm` model.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui::{App, AppContext as _, Bounds, SharedString, WindowBounds, WindowOptions, px, size};
use nie_explore::fm::keys::{Key, KeyCode, Mods};
use nie_explore::fm::preview::{self, Body, Context, Preview, PreviewRequest, Style};
use nie_explore::fm::thumbs::ThumbCache;
use nie_explore::fm::{Fm, NodeKind};
use nie_formats::vfs::Vfs;
use nie_ui::explorer::{
    Backend, ColumnData, Entry, EntryKind, Explorer, Frame, KeyInput, PreviewBody, PreviewData,
};
use nie_ui::surface::Rgba8;
use nie_ui::theme::Syntax;

/// How long a preview request waits for the cursor to settle.
const DEBOUNCE: Duration = Duration::from_millis(40);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Wanted {
    path: String,
    hex: bool,
    index: i32,
}

type Done = Arc<Mutex<Vec<(u64, Preview)>>>;

/// The `fm` model, its preview scheduler and the conversion to the toolkit's frame.
pub struct FmBackend {
    fm: Fm,
    vfs: Arc<Vfs>,
    cache: Option<Arc<ThumbCache>>,
    done: Done,
    cancel: Option<Arc<AtomicBool>>,
    next: u64,
    latest: u64,
    wanted: Option<Wanted>,
    shown: Option<Preview>,
    loading: bool,
}

impl FmBackend {
    /// A backend over a mounted game.
    #[must_use]
    pub fn new(vfs: Arc<Vfs>, cache: Option<Arc<ThumbCache>>) -> Self {
        let mut backend = Self {
            fm: Fm::new(Arc::clone(&vfs)),
            vfs,
            cache,
            done: Done::default(),
            cancel: None,
            next: 0,
            latest: 0,
            wanted: None,
            shown: None,
            loading: false,
        };
        backend.sync();
        backend
    }

    /// The model, for hosts that drive it directly (opening a folder first, for instance).
    pub fn fm_mut(&mut self) -> &mut Fm {
        &mut self.fm
    }

    /// Asks for the preview of the entry under the cursor when it changed.
    pub fn sync(&mut self) {
        let view = self.fm.view();
        let wanted = view.target.as_ref().and_then(|t| {
            (t.kind == NodeKind::File).then(|| Wanted {
                path: t.path.clone(),
                hex: self.fm.preview.hex,
                index: self.fm.preview.index,
            })
        });
        if wanted == self.wanted {
            return;
        }
        self.wanted = wanted.clone();
        if let Some(flag) = self.cancel.take() {
            flag.store(true, Ordering::Relaxed);
        }
        match (wanted, view.target) {
            (Some(wanted), Some(target)) => {
                self.loading = true;
                self.next += 1;
                self.latest = self.next;
                let id = self.next;
                let flag = Arc::new(AtomicBool::new(false));
                self.cancel = Some(Arc::clone(&flag));
                let (vfs, cache, done) = (Arc::clone(&self.vfs), self.cache.clone(), Arc::clone(&self.done));
                let request = PreviewRequest { path: wanted.path, size: target.size, hex: wanted.hex, index: wanted.index };
                std::thread::spawn(move || {
                    let started = Instant::now();
                    std::thread::sleep(DEBOUNCE);
                    if flag.load(Ordering::Relaxed) {
                        return;
                    }
                    let poll = || flag.load(Ordering::Relaxed);
                    let ctx = Context { vfs: &vfs, cache: cache.as_deref(), cancelled: &poll };
                    let preview = preview::build(&ctx, &request);
                    let _ = started;
                    if !flag.load(Ordering::Relaxed) {
                        if let Ok(mut done) = done.lock() {
                            done.push((id, preview));
                        }
                    }
                });
            }
            _ => {
                self.loading = false;
                self.shown = None;
            }
        }
    }
}

fn syntax(style: Style) -> Syntax {
    match style {
        Style::Plain => Syntax::Plain,
        Style::Keyword => Syntax::Keyword,
        Style::String => Syntax::String,
        Style::Number => Syntax::Number,
        Style::Constant => Syntax::Constant,
        Style::Comment => Syntax::Comment,
        Style::Function => Syntax::Function,
        Style::Type => Syntax::Type,
        Style::Property => Syntax::Property,
        Style::Operator => Syntax::Operator,
        Style::Punctuation => Syntax::Punctuation,
        Style::Label => Syntax::Label,
        Style::Dim => Syntax::Dim,
        Style::Header => Syntax::Header,
        Style::Accent => Syntax::Accent,
        Style::Error => Syntax::Error,
    }
}

fn picture(image: &preview::Image) -> Rgba8 {
    Rgba8 { width: image.width, height: image.height, rgba: image.rgba.clone() }
}

fn column(col: &nie_explore::fm::model::Column) -> ColumnData {
    ColumnData {
        title: SharedString::from(col.title.clone()),
        rows: col
            .rows
            .iter()
            .map(|r| Entry {
                name: SharedString::from(r.name.clone()),
                kind: match r.kind {
                    NodeKind::Dir => EntryKind::Dir,
                    NodeKind::File => EntryKind::File,
                },
                detail: SharedString::from(match r.kind {
                    NodeKind::Dir => format!("{} files", r.files),
                    NodeKind::File => preview::human_size(r.size),
                }),
                marked: r.marked,
            })
            .collect(),
        cursor: col.cursor,
        total: col.total,
    }
}

fn convert(preview: &Preview, frame_index: i32) -> PreviewData {
    let body = match &preview.body {
        Body::Image(image) => PreviewBody::Image(picture(image)),
        Body::Frames(frames) => {
            let ix = usize::try_from(frame_index).unwrap_or(0).min(frames.images.len().saturating_sub(1));
            frames.images.get(ix).map_or(PreviewBody::Empty, |i| PreviewBody::Image(picture(i)))
        }
        Body::Text(lines) => PreviewBody::Text(
            lines.iter().map(|l| l.0.iter().map(|s| (syntax(s.style), s.text.clone())).collect()).collect(),
        ),
        Body::Table(t) => PreviewBody::Table { columns: t.columns.clone(), rows: t.rows.clone(), total: t.total_rows },
        Body::Hex { bytes, .. } => PreviewBody::Hex(
            bytes.chunks(16).take(512).enumerate().map(|(i, c)| preview::hex_row(i * 16, c)).collect(),
        ),
        Body::Message(m) => PreviewBody::Message(m.clone()),
    };
    PreviewData { path: SharedString::from(preview.path.clone()), kind: SharedString::from(preview.kind), meta: preview.meta.clone(), body }
}

fn key_of(input: &KeyInput) -> Option<Key> {
    let mods = Mods { ctrl: input.ctrl, alt: input.alt, shift: input.shift };
    let code = match input.key.as_str() {
        "enter" => KeyCode::Enter,
        "escape" => KeyCode::Esc,
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "tab" => KeyCode::Tab,
        "space" => KeyCode::Space,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        f if f.len() > 1 && f.starts_with('f') && f[1..].parse::<u8>().is_ok() => KeyCode::F(f[1..].parse().ok()?),
        _ => {
            let text = input.key_char.as_deref().unwrap_or(&input.key);
            let mut chars = text.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            let c = if input.shift && input.key_char.is_none() { c.to_ascii_uppercase() } else { c };
            return Some(Key { code: KeyCode::Char(c), mods: Mods { shift: false, ..mods } });
        }
    };
    Some(Key { code, mods })
}

impl Backend for FmBackend {
    fn set_height(&mut self, rows: usize) {
        self.fm.set_height(rows);
    }

    fn key(&mut self, input: &KeyInput) {
        if let Some(key) = key_of(input) {
            self.fm.handle_key(key);
            self.sync();
        }
    }

    fn poll(&mut self) -> bool {
        let mut changed = self.fm.poll();
        let finished: Vec<_> = self.done.lock().map(|mut d| d.drain(..).collect()).unwrap_or_default();
        for (id, preview) in finished {
            if id == self.latest {
                self.shown = Some(preview);
                self.loading = false;
                changed = true;
            }
        }
        changed
    }

    fn frame(&self) -> Frame {
        let view = self.fm.view();
        let preview = match (&self.shown, &view.target) {
            (Some(p), Some(t)) if p.path == t.path => Some(convert(p, self.fm.preview.frame)),
            (_, Some(t)) if t.kind == NodeKind::Dir => Some(PreviewData {
                path: SharedString::from(t.path.clone()),
                kind: SharedString::from("folder"),
                meta: vec![("files".to_owned(), t.files.to_string())],
                body: PreviewBody::Text(
                    t.children
                        .iter()
                        .map(|(name, kind)| {
                            vec![(
                                if *kind == NodeKind::Dir { Syntax::Type } else { Syntax::Plain },
                                if *kind == NodeKind::Dir { format!("{name}/") } else { name.clone() },
                            )]
                        })
                        .collect(),
                ),
            }),
            (_, Some(t)) => Some(PreviewData {
                path: SharedString::from(t.path.clone()),
                kind: SharedString::from("loading"),
                meta: Vec::new(),
                body: PreviewBody::Message(if self.loading { "Loading the preview...".into() } else { String::new() }),
            }),
            _ => None,
        };
        Frame {
            cwd: SharedString::from(view.cwd.clone()),
            parent: column(&view.parent),
            current: column(&view.current),
            preview,
            status: SharedString::from(view.status.clone().unwrap_or_else(|| format!("{} files", view.files))),
        }
    }

    fn should_quit(&self) -> bool {
        self.fm.should_quit()
    }
}

/// Opens the explorer window on a GPUI app.
pub fn open(cx: &mut App, backend: FmBackend) {
    let bounds = Bounds::centered(None, size(px(1280.0), px(760.0)), cx);
    let window = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(gpui::TitlebarOptions { title: Some("nie explorer".into()), ..Default::default() }),
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(|cx| Explorer::new(backend, cx));
            let focus = view.read(cx).focus_handle().clone();
            window.focus(&focus, cx);
            view
        },
    );
    if let Err(error) = window {
        eprintln!("nie-explorer: cannot open a window: {error:#}");
        cx.quit();
    }
}
