//! Previews: what the right-hand column of `nie fm` shows for the entry under the cursor.
//!
//! A preview is plain data ([`Preview`]); drawing it is the job of the renderer. [`build`] is
//! synchronous and polls a cancellation callback between its stages, so an async scheduler can
//! drop a stale request (the cursor moved on) without waiting for a 90 MB film to demux.

mod hex;
mod image;
mod sheet;
mod table;
mod text;
#[cfg(feature = "fm-model")]
mod model;
#[cfg(feature = "fm-video")]
mod video;

use std::time::{Duration, Instant};

use nie_formats::vfs::Vfs;

use super::thumbs::{Cached, Key, ThumbCache};

pub use self::hex::{HEX_BYTES, row as hex_row};

/// Longest side of a decoded preview image, in pixels.
pub const MAX_SIDE: u32 = 1024;

/// A decoded picture, RGBA8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

impl Image {
    /// Whether the buffer matches the declared size.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.rgba.len() == self.width as usize * self.height as usize * 4
    }
}

/// The semantic class of a piece of text; each renderer maps it to its own colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Style {
    /// Unstyled.
    #[default]
    Plain,
    /// A language keyword.
    Keyword,
    /// A string literal.
    String,
    /// A number literal.
    Number,
    /// A constant (`true`, `nil`).
    Constant,
    /// A comment.
    Comment,
    /// A function or an opcode.
    Function,
    /// A type or a constructor.
    Type,
    /// A field name.
    Property,
    /// An operator.
    Operator,
    /// Brackets and separators.
    Punctuation,
    /// A label.
    Label,
    /// Secondary information.
    Dim,
    /// A heading.
    Header,
    /// A value worth looking at.
    Accent,
    /// An error.
    Error,
}

/// A run of text with one style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// The text, without a newline.
    pub text: String,
    /// Its style.
    pub style: Style,
}

/// One line of styled text.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Line(pub Vec<Span>);

impl Line {
    /// A line of one span.
    #[must_use]
    pub fn new(text: impl Into<String>, style: Style) -> Self {
        Self(vec![Span {
            text: text.into(),
            style,
        }])
    }

    /// The text of the line without styles.
    #[must_use]
    pub fn plain(&self) -> String {
        self.0.iter().map(|s| s.text.as_str()).collect()
    }
}

/// A table: one row per record, one column per field.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Table {
    /// A caption such as the record name.
    pub title: String,
    /// Column headings.
    pub columns: Vec<String>,
    /// The rows kept for display.
    pub rows: Vec<Vec<String>>,
    /// The number of rows in the file, `rows.len()` or more.
    pub total_rows: usize,
}

/// Several pictures shown one at a time or as an animation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Frames {
    /// The pictures.
    pub images: Vec<Image>,
    /// One caption per picture.
    pub labels: Vec<String>,
    /// Frames per second when the sequence animates; 0 for a stepped gallery.
    pub fps: u32,
}

/// The content of a preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    /// One picture.
    Image(Image),
    /// A texture atlas: its sprites, a film's frames, a model's turntable.
    Frames(Frames),
    /// Styled text, read-only.
    Text(Vec<Line>),
    /// Records as columns.
    Table(Table),
    /// The first bytes of the file.
    Hex {
        /// The bytes kept for display.
        bytes: Vec<u8>,
        /// The size of the whole file.
        total: usize,
    },
    /// One short sentence: an error, or why nothing can be shown.
    Message(String),
}

/// What the renderer asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewRequest {
    /// Logical VFS path.
    pub path: String,
    /// Size in bytes, from the index.
    pub size: u64,
    /// Force the hex view.
    pub hex: bool,
    /// Which texture or language to show (the `[` and `]` keys).
    pub index: i32,
}

impl PreviewRequest {
    /// A request for the default view.
    #[must_use]
    pub fn new(path: impl Into<String>, size: u64) -> Self {
        Self {
            path: path.into(),
            size,
            hex: false,
            index: 0,
        }
    }
}

/// A finished preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    /// The request path.
    pub path: String,
    /// The kind of previewer that produced it (`image`, `sprites`, `table`, `lua`, `hex`, ...).
    pub kind: &'static str,
    /// Facts about the file, as label and value.
    pub meta: Vec<(String, String)>,
    /// The content.
    pub body: Body,
    /// Time spent building it.
    pub took: Duration,
    /// Whether the picture came from the thumbnail cache.
    pub cached: bool,
}

/// Everything a previewer needs.
pub struct Context<'a> {
    /// The mounted game.
    pub vfs: &'a Vfs,
    /// The thumbnail cache, when one could be opened.
    pub cache: Option<&'a ThumbCache>,
    /// Returns true once the request is stale.
    pub cancelled: &'a dyn Fn() -> bool,
}

/// The kind of previewer chosen for a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A G4TX texture or atlas.
    Texture,
    /// A DDS, PNG or WebP picture.
    Picture,
    /// A cfg.bin table (RDBN or T2B).
    Table,
    /// An event script (`event_cfg`).
    Event,
    /// Lua 5.2 bytecode.
    LuaBytecode,
    /// Lua source text.
    LuaSource,
    /// Plain text.
    Text,
    /// A USM film.
    Video,
    /// A G4MD model.
    Model,
    /// Anything else.
    Binary,
}

impl Kind {
    /// The short name shown in the status line.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Texture => "texture",
            Self::Picture => "image",
            Self::Table => "table",
            Self::Event => "event",
            Self::LuaBytecode => "lua",
            Self::LuaSource => "lua",
            Self::Text => "text",
            Self::Video => "video",
            Self::Model => "model",
            Self::Binary => "hex",
        }
    }
}

/// Chooses the previewer from the magic bytes first and the name second. `data` is the whole file.
#[must_use]
pub fn classify(path: &str, data: &[u8]) -> Kind {
    // The T2B check compares the string table with the whole length, so it needs the whole file.
    let head = data;
    use nie_formats::FileFormat;
    let lower = path.to_ascii_lowercase();
    if head.starts_with(b"\x1bLua") {
        return Kind::LuaBytecode;
    }
    match nie_formats::detect(head) {
        FileFormat::G4tx => return Kind::Texture,
        FileFormat::Dds | FileFormat::Png | FileFormat::Webp => return Kind::Picture,
        FileFormat::Usm => return Kind::Video,
        FileFormat::G4md => return Kind::Model,
        _ => {}
    }
    let named_table = lower.ends_with(".cfg.bin") || lower.ends_with(".cfg");
    if named_table && (nie_formats::cfgbin::is_rdbn(head) || nie_formats::cfgbin::is_t2b(head)) {
        return if lower.contains("/event_cfg/") {
            Kind::Event
        } else {
            Kind::Table
        };
    }
    if lower.ends_with(".usm") {
        return Kind::Video;
    }
    if lower.ends_with(".g4md") {
        return Kind::Model;
    }
    if lower.ends_with(".lua") && text::is_text(head) {
        return Kind::LuaSource;
    }
    if text::is_text(head) {
        return Kind::Text;
    }
    Kind::Binary
}

/// Builds the preview of one file.
///
/// Never fails: a file that cannot be decoded gets a [`Body::Message`] or the hex view, and the
/// message says which decoder refused it.
#[must_use]
pub fn build(ctx: &Context<'_>, request: &PreviewRequest) -> Preview {
    let started = Instant::now();
    let finish = |kind: &'static str, meta: Vec<(String, String)>, body: Body, cached: bool| Preview {
        path: request.path.clone(),
        kind,
        meta,
        body,
        took: started.elapsed(),
        cached,
    };
    let mut meta = vec![
        ("path".to_owned(), request.path.clone()),
        ("size".to_owned(), human_size(request.size)),
    ];
    let mut cpk = String::new();
    if let Some(entry) = ctx.vfs.find_any(&request.path)
        && !entry.cpk.is_empty()
    {
        entry.cpk.clone_into(&mut cpk);
        meta.push(("cpk".to_owned(), cpk.clone()));
    }
    let key = Key::new(&request.path, &cpk, request.size, request.index);
    if !request.hex
        && let Some(cache) = ctx.cache
        && let Some(hit) = cache.get(&key)
    {
        let kind = if hit.kind == "texture" { "texture" } else { "image" };
        meta.extend(hit.meta);
        meta.push(("cache".to_owned(), "hit".to_owned()));
        return finish(kind, meta, Body::Image(hit.image), true);
    }
    if (ctx.cancelled)() {
        return finish("cancelled", meta, Body::Message("cancelled".into()), false);
    }

    let data = match ctx.vfs.read(&request.path) {
        Ok(data) => data,
        Err(error) => {
            return finish(
                "error",
                meta,
                Body::Message(format!("cannot read the file: {error}")),
                false,
            );
        }
    };
    if (ctx.cancelled)() {
        return finish("cancelled", meta, Body::Message("cancelled".into()), false);
    }
    let kind = if request.hex {
        Kind::Binary
    } else {
        classify(&request.path, &data)
    };
    let (body, extra, cached) = match kind {
        Kind::Texture => image::texture(ctx, request, &data),
        Kind::Picture => image::picture(&data),
        Kind::Table => table::table(ctx, request, &data),
        Kind::Event => text::event(&data),
        Kind::LuaBytecode => text::lua_bytecode(&data),
        Kind::LuaSource => text::lua_source(&data),
        Kind::Text => text::plain(&data),
        Kind::Video => video_preview(ctx, request, &data),
        Kind::Model => model_preview(ctx, request, &data),
        Kind::Binary => hex::view(&data),
    };
    if !request.hex
        && let Body::Image(image) = &body
        && let Some(cache) = ctx.cache
    {
        let stored = Cached {
            kind: kind.name().to_owned(),
            meta: extra.clone(),
            image: image.clone(),
        };
        if cache.put(&key, &stored).is_ok() {
            meta.push(("cache".to_owned(), "stored".to_owned()));
        }
    }
    meta.extend(extra);
    finish(kind.name(), meta, body, cached)
}

type Built = (Body, Vec<(String, String)>, bool);

#[cfg(feature = "fm-video")]
fn video_preview(ctx: &Context<'_>, request: &PreviewRequest, data: &[u8]) -> Built {
    video::film(ctx, request, data)
}

#[cfg(not(feature = "fm-video"))]
fn video_preview(_: &Context<'_>, request: &PreviewRequest, data: &[u8]) -> Built {
    sheet::film_card(&request.path, data, "the build has no H.264 decoder (feature fm-video)")
}

#[cfg(feature = "fm-model")]
fn model_preview(ctx: &Context<'_>, request: &PreviewRequest, data: &[u8]) -> Built {
    model::turntable(ctx, request, data)
}

#[cfg(not(feature = "fm-model"))]
fn model_preview(_: &Context<'_>, _: &PreviewRequest, data: &[u8]) -> Built {
    hex::message_with_hex(data, "the build has no model renderer (feature fm-model)")
}

/// `1.5 MiB` style sizes.
#[must_use]
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_by_magic_then_name() {
        assert_eq!(classify("data/x.bin", b"\x1bLuaS\0"), Kind::LuaBytecode);
        assert_eq!(classify("data/x.g4tx", b"G4TX\0\0\0\0"), Kind::Texture);
        assert_eq!(classify("data/x.txt", b"hello world\n"), Kind::Text);
        assert_eq!(classify("data/x.bin", &[0, 1, 2, 3, 255, 254]), Kind::Binary);
        assert_eq!(classify("data/x.lua", b"print(1)\n"), Kind::LuaSource);
    }

    use crate::fm::fixture;

    fn built(game: &fixture::Fixture, cache: Option<&ThumbCache>, path: &str, index: i32) -> Preview {
        let never = || false;
        let ctx = Context {
            vfs: &game.vfs,
            cache,
            cancelled: &never,
        };
        let size = game.vfs.find_any(path).map_or(0, |e| e.size);
        let mut request = PreviewRequest::new(path, size);
        request.index = index;
        build(&ctx, &request)
    }

    fn text_of(preview: &Preview) -> String {
        match &preview.body {
            Body::Text(lines) => lines.iter().map(Line::plain).collect::<Vec<_>>().join("\n"),
            other => panic!("{path}: expected text, got {other:?}", path = preview.path),
        }
    }

    #[test]
    fn an_atlas_previews_as_its_sprites() {
        let game = fixture::game();
        let preview = built(&game, None, "data/dx11/menu/icons.g4tx", 0);
        assert_eq!(preview.kind, "texture");
        let Body::Frames(frames) = &preview.body else {
            panic!("frames, got {:?}", preview.body)
        };
        assert_eq!(frames.images.len(), 2);
        assert!(frames.labels[0].starts_with("icon_a"));
        assert!(frames.images.iter().all(Image::is_valid));
        assert_eq!((frames.images[0].width, frames.images[0].height), (16, 16));
    }

    #[test]
    fn textures_step_with_the_item_index_and_hit_the_cache() {
        let game = fixture::game();
        let dir = tempfile::tempdir().unwrap();
        let cache = ThumbCache::open_in(dir.path().join("cache"), 1 << 24, Some(game.dir.path())).unwrap();
        let path = "data/dx11/menu/plain.g4tx";
        let first = built(&game, Some(&cache), path, 0);
        let Body::Image(a) = &first.body else { panic!("image") };
        assert_eq!((a.width, a.height), (16, 16));
        assert!(!first.cached);
        let second = built(&game, Some(&cache), path, 1);
        let Body::Image(b) = &second.body else { panic!("image") };
        assert_eq!((b.width, b.height), (32, 32));
        let again = built(&game, Some(&cache), path, 0);
        assert!(again.cached, "the second view of the same texture comes from the cache");
        assert_eq!(again.body, first.body);
    }

    #[test]
    fn a_localized_table_switches_language() {
        let game = fixture::game();
        let en = built(&game, None, "data/common/text/en/words.cfg.bin", 0);
        let Body::Table(table) = &en.body else { panic!("table, got {:?}", en.body) };
        assert_eq!(table.title, "WORD_INFO");
        assert_eq!(table.rows[0], ["1", "hello"]);
        let fr = built(&game, None, "data/common/text/en/words.cfg.bin", 1);
        let Body::Table(table) = &fr.body else { panic!("table, got {:?}", fr.body) };
        assert_eq!(table.rows[0], ["1", "bonjour"]);
        assert!(fr.meta.iter().any(|(k, v)| k == "language" && v.starts_with("fr")));
    }

    #[test]
    fn records_of_a_table_step_with_the_item_index() {
        let game = fixture::game();
        let first = built(&game, None, "data/common/menu/ui_main.cfg.bin", 0);
        assert_eq!(first.kind, "table");
        let Body::Table(table) = &first.body else { panic!("table") };
        assert_eq!(table.total_rows, 2);
        assert_eq!(table.rows[1], ["20", "quit"]);
    }

    #[test]
    fn scripts_and_text_are_highlighted_or_plain() {
        let game = fixture::game();
        let lua = built(&game, None, "data/common/script/intro.lua.bin", 0);
        assert_eq!(lua.kind, "lua");
        let text = text_of(&lua);
        assert!(text.contains("LOADK(0, 0)"), "{text}");
        assert!(text.contains("\"hello\""), "{text}");
        let Body::Text(lines) = &lua.body else { unreachable!() };
        assert!(lines.iter().flat_map(|l| &l.0).any(|s| s.style == Style::Function));

        let event = built(&game, None, "data/common/event_cfg/evt/ev_demo.cfg.bin", 0);
        assert_eq!(event.kind, "event");
        assert!(text_of(&event).contains("cmd(0, \"Cut\""), "{}", text_of(&event));

        let plain = built(&game, None, "data/common/misc/readme.txt", 0);
        assert_eq!(plain.kind, "text");
        assert_eq!(text_of(&plain), "Synthetic game.\nSecond line.");
    }

    #[test]
    fn unknown_binary_falls_back_to_hex() {
        let game = fixture::game();
        let blob = built(&game, None, "data/common/misc/blob.bin", 0);
        assert_eq!(blob.kind, "hex");
        let Body::Hex { bytes, total } = &blob.body else { panic!("hex") };
        assert_eq!((bytes.len(), *total), (8, 8));
    }

    #[test]
    fn a_cancelled_request_stops_before_reading() {
        let game = fixture::game();
        let stale = || true;
        let ctx = Context {
            vfs: &game.vfs,
            cache: None,
            cancelled: &stale,
        };
        let preview = build(&ctx, &PreviewRequest::new("data/dx11/menu/icons.g4tx", 1));
        assert_eq!(preview.kind, "cancelled");
    }

    #[test]
    fn sizes_are_human_readable() {
        assert_eq!(human_size(12), "12 B");
        assert_eq!(human_size(1536), "1.5 KiB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MiB");
    }
}
