//! The explorer over HTTP: the producer of [`nie_ui::explorer::Frame`] that talks to `nie serve`.
//!
//! It is the data side of the browser build (gpui_web has a `fetch` HTTP client) and works the same
//! on native builds that install an HTTP client. Listings come from `GET /v1/vfs/ls?path=` and
//! pictures from `GET /tex/<path>.png`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use futures::AsyncReadExt as _;
use gpui::{ForegroundExecutor, SharedString};
use http_client::{AsyncBody, HttpClient};
use nie_ui::explorer::{Backend, ColumnData, Entry, EntryKind, Frame, KeyInput, PreviewBody, PreviewData};
use nie_ui::surface::Rgba8;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    name: String,
    path: String,
    kind: EntryKind,
    detail: String,
}

#[derive(Debug, Clone, Default)]
struct Listing {
    dir: String,
    rows: Vec<Row>,
    total_files: usize,
}

enum Msg {
    Health(String),
    Listing(Listing),
    Failed(String, String),
    Picture(String, Rgba8),
    NoPicture(String),
}

/// Read-only explorer state driven by `nie serve`.
pub struct HttpBackend {
    client: Arc<dyn HttpClient>,
    executor: ForegroundExecutor,
    base: String,
    cwd: String,
    listings: HashMap<String, Listing>,
    cursors: HashMap<String, usize>,
    cursor: usize,
    height: usize,
    inbox: Rc<RefCell<Vec<Msg>>>,
    requested: Vec<String>,
    preview: Option<(String, PreviewBody)>,
    status: String,
    /// `936 CPK, 255628 entries`, from `/health`.
    summary: String,
    quit: bool,
}

fn parent_of(dir: &str) -> Option<String> {
    if dir.is_empty() {
        return None;
    }
    Some(dir.rsplit_once('/').map_or(String::new(), |(p, _)| p.to_owned()))
}

fn human(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 { format!("{bytes} B") } else { format!("{v:.1} {}", UNITS[u]) }
}

fn parse_listing(dir: &str, json: &Value) -> Listing {
    let mut rows = Vec::new();
    for d in json["dirs"].as_array().into_iter().flatten() {
        let name = d["name"].as_str().unwrap_or_default().to_owned();
        let path = if dir.is_empty() { name.clone() } else { format!("{dir}/{name}") };
        rows.push(Row { detail: format!("{} files", d["files"].as_u64().unwrap_or(0)), name, path, kind: EntryKind::Dir });
    }
    for f in json["files"].as_array().into_iter().flatten() {
        let name = f["name"].as_str().unwrap_or_default().to_owned();
        let path = f["path"].as_str().map_or_else(
            || if dir.is_empty() { name.clone() } else { format!("{dir}/{name}") },
            str::to_owned,
        );
        rows.push(Row { detail: f["size"].as_u64().map(human).unwrap_or_default(), name, path, kind: EntryKind::File });
    }
    Listing { dir: dir.to_owned(), rows, total_files: json["fileTotal"].as_u64().unwrap_or(0) as usize }
}

fn decode_png(bytes: &[u8]) -> Option<Rgba8> {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf.as_chunks::<3>().0.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf.as_chunks::<2>().0.iter().flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    Some(Rgba8 { width: info.width, height: info.height, rgba })
}

impl HttpBackend {
    /// A backend over the `nie serve` at `base` (`http://host:8790`, or empty for the page's own
    /// origin in the browser).
    pub fn new(client: Arc<dyn HttpClient>, executor: ForegroundExecutor, base: impl Into<String>) -> Self {
        let mut backend = Self {
            client,
            executor,
            base: base.into().trim_end_matches('/').to_owned(),
            cwd: String::new(),
            listings: HashMap::new(),
            cursors: HashMap::new(),
            cursor: 0,
            height: 24,
            inbox: Rc::default(),
            requested: Vec::new(),
            preview: None,
            status: "Loading the game listing...".to_owned(),
            summary: String::new(),
            quit: false,
        };
        backend.request_listing(String::new());
        backend.fetch(format!("{}/health", backend.base), |r| {
            let json = r.ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok());
            Msg::Health(json.map_or_else(String::new, |j| {
                format!("{} CPK, {} entries", j["cpks"].as_u64().unwrap_or(0), j["entries"].as_u64().unwrap_or(0))
            }))
        });
        backend
    }

    fn fetch(&self, url: String, then: impl FnOnce(anyhow::Result<Vec<u8>>) -> Msg + 'static) {
        let client = Arc::clone(&self.client);
        let inbox = Rc::clone(&self.inbox);
        self.executor
            .spawn(async move {
                let result = async {
                    let mut response = client.get(&url, AsyncBody::empty(), true).await?;
                    anyhow::ensure!(response.status().is_success(), "{url}: HTTP {}", response.status());
                    let mut bytes = Vec::new();
                    response.body_mut().read_to_end(&mut bytes).await?;
                    Ok(bytes)
                }
                .await;
                inbox.borrow_mut().push(then(result));
            })
            .detach();
    }

    fn request_listing(&mut self, dir: String) {
        if self.listings.contains_key(&dir) || self.requested.contains(&dir) {
            return;
        }
        self.requested.push(dir.clone());
        let url = format!("{}/v1/vfs/ls?path={}&limit=2000", self.base, dir);
        self.fetch(url, move |r| match r.and_then(|b| Ok(serde_json::from_slice::<Value>(&b)?)) {
            Ok(json) => Msg::Listing(parse_listing(&dir, &json)),
            Err(e) => Msg::Failed(dir, e.to_string()),
        });
    }

    fn rows(&self) -> &[Row] {
        self.listings.get(&self.cwd).map_or(&[], |l| l.rows.as_slice())
    }

    fn selected(&self) -> Option<&Row> {
        self.rows().get(self.cursor)
    }

    fn remember(&mut self) {
        self.cursors.insert(self.cwd.clone(), self.cursor);
        self.preview = None;
        if let Some(row) = self.selected().cloned() {
            match row.kind {
                EntryKind::Dir => self.request_listing(row.path),
                EntryKind::File => {
                    let url = format!("{}/tex/{}.png", self.base, row.path);
                    let path = row.path.clone();
                    self.fetch(url, move |r| match r.ok().and_then(|b| decode_png(&b)) {
                        Some(img) => Msg::Picture(path, img),
                        None => Msg::NoPicture(path),
                    });
                }
            }
        }
    }

    fn go(&mut self, dir: String) {
        self.cwd = dir.clone();
        self.cursor = self.cursors.get(&dir).copied().unwrap_or(0);
        self.request_listing(dir.clone());
        if let Some(p) = parent_of(&dir) {
            self.request_listing(p);
        }
        self.remember();
    }

    fn column(&self, dir: &str, cursor: Option<usize>, window: bool) -> ColumnData {
        let Some(listing) = self.listings.get(dir) else {
            return ColumnData { title: SharedString::from(dir.to_owned()), ..ColumnData::default() };
        };
        let (start, end) = if window {
            let start = self.cursor.saturating_sub(self.height.saturating_sub(1)).min(self.cursor.saturating_sub(self.height / 2));
            (start, (start + self.height).min(listing.rows.len()))
        } else {
            (0, listing.rows.len().min(self.height))
        };
        ColumnData {
            title: SharedString::from(if dir.is_empty() { "/".to_owned() } else { dir.to_owned() }),
            rows: listing.rows[start..end]
                .iter()
                .map(|r| Entry {
                    name: SharedString::from(r.name.clone()),
                    kind: r.kind,
                    detail: SharedString::from(r.detail.clone()),
                    marked: false,
                })
                .collect(),
            cursor: cursor.and_then(|c| c.checked_sub(start)).filter(|c| start + c < end),
            total: listing.rows.len().max(listing.total_files),
        }
    }
}

impl Backend for HttpBackend {
    fn set_height(&mut self, rows: usize) {
        self.height = rows.max(1);
    }

    fn key(&mut self, key: &KeyInput) {
        let last = self.rows().len().saturating_sub(1);
        let before = (self.cwd.clone(), self.cursor);
        match (key.key.as_str(), key.key_char.as_deref()) {
            ("down", _) | (_, Some("j")) => self.cursor = (self.cursor + 1).min(last),
            ("up", _) | (_, Some("k")) => self.cursor = self.cursor.saturating_sub(1),
            ("pagedown", _) => self.cursor = (self.cursor + self.height).min(last),
            ("pageup", _) => self.cursor = self.cursor.saturating_sub(self.height),
            ("home", _) | (_, Some("g")) => self.cursor = 0,
            ("end", _) | (_, Some("G")) => self.cursor = last,
            ("right" | "enter", _) | (_, Some("l")) => {
                if let Some(row) = self.selected().cloned()
                    && row.kind == EntryKind::Dir
                {
                    self.go(row.path);
                    return;
                }
            }
            ("left" | "backspace", _) | (_, Some("h")) => {
                if let Some(parent) = parent_of(&self.cwd) {
                    let child = self.cwd.clone();
                    self.go(parent);
                    if let Some(i) = self.rows().iter().position(|r| r.path == child) {
                        self.cursor = i;
                        self.remember();
                    }
                    return;
                }
            }
            (_, Some("q")) => self.quit = true,
            _ => {}
        }
        if before != (self.cwd.clone(), self.cursor) {
            self.remember();
        }
    }

    fn poll(&mut self) -> bool {
        let msgs: Vec<Msg> = self.inbox.borrow_mut().drain(..).collect();
        let changed = !msgs.is_empty();
        for msg in msgs {
            match msg {
                Msg::Health(summary) => self.summary = summary,
                Msg::Listing(l) => {
                    let dir = l.dir.clone();
                    self.listings.insert(dir.clone(), l);
                    self.status = String::new();
                    if dir == self.cwd && self.preview.is_none() {
                        self.remember();
                    }
                }
                Msg::Failed(dir, e) => self.status = format!("{dir}: {e}"),
                Msg::Picture(path, img) => {
                    if self.selected().is_some_and(|r| r.path == path) {
                        self.preview = Some((path, PreviewBody::Image(img)));
                    }
                }
                Msg::NoPicture(path) => {
                    if self.selected().is_some_and(|r| r.path == path) {
                        self.preview = Some((path, PreviewBody::Message("No picture for this file; use nie fm for other previews.".into())));
                    }
                }
            }
        }
        changed
    }

    fn frame(&self) -> Frame {
        let parent_dir = parent_of(&self.cwd).unwrap_or_default();
        let parent_cursor = self.listings.get(&parent_dir).and_then(|l| l.rows.iter().position(|r| r.path == self.cwd));
        let preview = self.selected().map(|row| match (&self.preview, row.kind) {
            (Some((p, body)), _) if *p == row.path => PreviewData {
                path: SharedString::from(row.path.clone()),
                kind: SharedString::from("file"),
                meta: vec![("size".into(), row.detail.clone())],
                body: body.clone(),
            },
            (_, EntryKind::Dir) => PreviewData {
                path: SharedString::from(row.path.clone()),
                kind: SharedString::from("folder"),
                meta: vec![("files".into(), row.detail.clone())],
                body: PreviewBody::Text(
                    self.listings
                        .get(&row.path)
                        .map(|l| {
                            l.rows
                                .iter()
                                .take(self.height)
                                .map(|r| vec![(
                                    if r.kind == EntryKind::Dir { nie_ui::Syntax::Type } else { nie_ui::Syntax::Plain },
                                    if r.kind == EntryKind::Dir { format!("{}/", r.name) } else { r.name.clone() },
                                )])
                                .collect()
                        })
                        .unwrap_or_default(),
                ),
            },
            _ => PreviewData {
                path: SharedString::from(row.path.clone()),
                kind: SharedString::from("file"),
                meta: vec![("size".into(), row.detail.clone())],
                body: PreviewBody::Message("Loading the preview...".into()),
            },
        });
        Frame {
            cwd: SharedString::from(if self.cwd.is_empty() { "/".to_owned() } else { self.cwd.clone() }),
            parent: self.column(&parent_dir, parent_cursor, false),
            current: self.column(&self.cwd, Some(self.cursor), true),
            preview,
            status: SharedString::from(if self.status.is_empty() { self.summary.clone() } else { self.status.clone() }),
        }
    }

    fn should_quit(&self) -> bool {
        self.quit
    }
}
