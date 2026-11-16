//! Page builders: SQL results to [`UiNode`] trees.

use nie_ui_dioxus::{Theme, UiNode, UiStyle, label, link, page, simple_table};

use crate::db::{DirListing, FileRow, Stats};

/// Rows shown per table.
pub const PREVIEW_ROWS: i64 = 40;

/// URL prefix of wiki file pages.
pub const FILES_PREFIX: &str = "/wiki/files";

/// Percent-encodes a path segment.
fn enc(segment: &str) -> String {
    nie_sitemap::encode_segment(segment)
}

/// URL of a file or directory page.
#[must_use]
pub fn page_url(path: &str, dir: bool) -> String {
    let mut u = String::from(FILES_PREFIX);
    for s in path.split('/').filter(|s| !s.is_empty()) {
        u.push('/');
        u.push_str(&enc(s));
    }
    if dir || path.is_empty() {
        u.push('/');
    }
    u
}

fn human(size: i64) -> String {
    let s = size as f64;
    if s >= 1e6 {
        format!("{:.1} MB", s / 1e6)
    } else if s >= 1e3 {
        format!("{:.1} kB", s / 1e3)
    } else {
        format!("{size} B")
    }
}

fn crumbs(path: &str, t: &Theme) -> UiNode {
    let mut parts = vec![link(14.0, t.muted, page_url("", true), "files")];
    let mut acc = String::new();
    for seg in path.split('/').filter(|s| !s.is_empty()) {
        if !acc.is_empty() {
            acc.push('/');
        }
        acc.push_str(seg);
        parts.push(label(14.0, t.muted, "/"));
        parts.push(link(14.0, t.text, page_url(&acc, true), seg));
    }
    UiNode::boxed(UiStyle { axis: nie_ui_dioxus::Axis::Row, gap: 6.0, wrap: true, ..UiStyle::default() }, parts)
}

fn nav(t: &Theme) -> UiNode {
    UiNode::boxed(
        UiStyle { axis: nie_ui_dioxus::Axis::Row, gap: 16.0, ..UiStyle::default() },
        vec![
            label(18.0, t.text, "Azalee"),
            link(14.0, t.muted, "/", "home"),
            link(14.0, t.muted, page_url("", true), "files"),
            link(14.0, t.muted, "/sitemap.xml", "sitemap"),
        ],
    )
}

/// Home page.
#[must_use]
pub fn home(stats: &Stats, root: &DirListing, t: &Theme) -> UiNode {
    let mut kids = vec![nav(t), label(26.0, t.text, "Inazuma Eleven: Victory Road game data")];
    kids.push(label(14.0, t.muted, format!("{} files in the VFS, read from the game install and imported into PostgreSQL.", stats.entries)));
    let mut rows: Vec<Vec<String>> = stats.formats.iter().map(|(f, n)| vec![format!("cfg.bin {f}"), n.to_string()]).collect();
    rows.push(vec!["T2B entries".into(), stats.t2b_entries.to_string()]);
    rows.push(vec!["T2B variables".into(), stats.t2b_vars.to_string()]);
    kids.push(simple_table(&["Dataset".into(), "Count".into()], &rows, 200.0, t));
    kids.push(label(18.0, t.text, "Browse"));
    for d in &root.dirs {
        kids.push(link(16.0, t.text, page_url(d, true), format!("{d}/")));
    }
    page(t, kids)
}

/// Directory page.
#[must_use]
pub fn directory(path: &str, d: &DirListing, t: &Theme) -> UiNode {
    let mut kids = vec![nav(t), crumbs(path, t), label(22.0, t.text, if path.is_empty() { "/" } else { path })];
    for sub in &d.dirs {
        let full = if path.is_empty() { sub.clone() } else { format!("{path}/{sub}") };
        kids.push(link(15.0, t.text, page_url(&full, true), format!("{sub}/")));
    }
    for (name, size) in &d.files {
        let full = if path.is_empty() { name.clone() } else { format!("{path}/{name}") };
        kids.push(UiNode::boxed(
            UiStyle { axis: nie_ui_dioxus::Axis::Row, gap: 12.0, ..UiStyle::default() },
            vec![link(14.0, t.text, page_url(&full, false), name.clone()), label(12.0, t.muted, human(*size))],
        ));
    }
    if d.total_files > i64::try_from(d.files.len()).unwrap_or(i64::MAX) {
        kids.push(label(12.0, t.muted, format!("{} of {} files shown", d.files.len(), d.total_files)));
    }
    page(t, kids)
}

/// Not-found page.
#[must_use]
pub fn not_found(path: &str, t: &Theme) -> UiNode {
    page(t, vec![nav(t), label(22.0, t.text, "Not found"), label(14.0, t.muted, path.to_owned())])
}

/// Body of a file page, assembled by the server from the queries it ran.
pub struct FileView<'a> {
    /// VFS path.
    pub path: &'a str,
    /// Pack file name.
    pub pack: &'a str,
    /// Size in bytes.
    pub size: i64,
    /// Decoded cfg.bin row, if the file is one.
    pub cfg: Option<&'a FileRow>,
    /// Preview tables: `(title, headers, rows, total rows)`.
    pub tables: Vec<(String, Vec<String>, Vec<Vec<String>>, i64)>,
    /// Image URL when the file is a texture or image.
    pub image: Option<String>,
}

/// File page.
#[must_use]
pub fn file(v: &FileView<'_>, t: &Theme) -> UiNode {
    let mut kids = vec![nav(t), crumbs(v.path, t), label(22.0, t.text, v.path.rsplit('/').next().unwrap_or(v.path))];
    let mut meta = vec![
        vec!["Path".into(), v.path.into()],
        vec!["Pack".into(), v.pack.into()],
        vec!["Size".into(), format!("{} ({} bytes)", human(v.size), v.size)],
    ];
    if let Some(c) = v.cfg {
        meta.push(vec!["SHA-256".into(), c.sha256.clone()]);
        meta.push(vec!["Format".into(), c.format.clone()]);
        if let Some(e) = &c.error {
            meta.push(vec!["Decode error".into(), e.clone()]);
        }
    }
    kids.push(simple_table(&["Field".into(), "Value".into()], &meta, 520.0, t));
    if let Some(src) = &v.image {
        kids.push(UiNode::Image { src: src.clone(), alt: v.path.to_owned(), width: 384.0, height: 384.0 });
    }
    for (title, headers, rows, total) in &v.tables {
        kids.push(label(18.0, t.text, title.clone()));
        kids.push(simple_table(headers, rows, 130.0, t));
        kids.push(label(12.0, t.muted, format!("{} of {} rows shown", rows.len(), total)));
    }
    page(t, kids)
}

/// Series navigation, localised.
fn series_nav(t: &Theme, lang: &str) -> UiNode {
    use crate::i18n::t as tr;
    let q = format!("?lang={lang}");
    UiNode::boxed(
        UiStyle { axis: nie_ui_dioxus::Axis::Row, gap: 16.0, wrap: true, ..UiStyle::default() },
        vec![
            label(18.0, t.text, "Azalee"),
            link(14.0, t.muted, format!("/{q}"), tr(lang, "nav.home")),
            link(14.0, t.muted, format!("/series{q}"), tr(lang, "nav.series")),
            link(14.0, t.muted, page_url("", true), tr(lang, "nav.files")),
            link(14.0, t.muted, "/sitemap.xml", tr(lang, "nav.sitemap")),
        ],
    )
}

/// `/series`: every game of the series from `data/series.json`, titles from the locale files.
#[must_use]
pub fn series(t: &Theme, lang: &str) -> UiNode {
    use crate::i18n::{self, t as tr};
    let mut kids = vec![series_nav(t, lang), label(28.0, t.text, tr(lang, "series.title")), label(15.0, t.muted, tr(lang, "series.lead"))];
    for g in i18n::series() {
        let status = tr(lang, if g.nie { "series.nie.yes" } else { "series.nie.no" });
        kids.push(UiNode::boxed(
            UiStyle { axis: nie_ui_dioxus::Axis::Column, gap: 4.0, ..UiStyle::default() },
            vec![
                label(18.0, t.text, tr(lang, &format!("game.{}", g.slug))),
                UiNode::boxed(
                    UiStyle { axis: nie_ui_dioxus::Axis::Row, gap: 12.0, wrap: true, ..UiStyle::default() },
                    vec![label(13.0, t.muted, status), link(13.0, t.muted, g.official, tr(lang, "series.official"))],
                ),
            ],
        ));
    }
    page(t, kids)
}
