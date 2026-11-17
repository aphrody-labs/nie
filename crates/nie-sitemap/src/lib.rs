//! Sitemap generated from the VFS listing: one page URL per logical file and per directory.
//!
//! Output follows sitemaps.org: at most 50,000 URLs per file, a `sitemapindex` when more are
//! needed. The listing comes from a live [`nie_formats::vfs::Vfs`] or from an NDJSON inventory
//! (`vfs_inventory` / `Manifest::to_ndjson` lines with a `path` field). Output is deterministic:
//! entries are sorted and deduplicated.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use nie_formats::vfs_manifest::canonical_path;

/// sitemaps.org limit per file.
pub const MAX_URLS: usize = 50_000;

/// One logical VFS file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VfsFile {
    /// Path inside the VFS (any separator; canonicalised).
    pub path: String,
    /// Extracted size in bytes (0 when unknown).
    pub size: u64,
}

/// Generation settings.
#[derive(Debug, Clone)]
pub struct Config {
    /// Origin without trailing slash, e.g. `https://rosegriffon.fr`.
    pub base_url: String,
    /// Route prefix the pages live under, e.g. `/wiki/files`.
    pub route_prefix: String,
    /// Optional `lastmod` (W3C date, e.g. the manifest build date).
    pub lastmod: Option<String>,
    /// URLs per sitemap file, clamped to [`MAX_URLS`].
    pub max_urls: usize,
}

impl Config {
    /// Settings with the 50,000 URL limit.
    #[must_use]
    pub fn new(base_url: &str, route_prefix: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            route_prefix: format!("/{}", route_prefix.trim_matches('/'))
                .trim_end_matches('/')
                .to_owned(),
            lastmod: None,
            max_urls: MAX_URLS,
        }
    }
}

/// Kind of a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PageKind {
    /// Directory listing page (URL ends with `/`).
    Dir,
    /// File page.
    File,
}

/// One page of the site.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Page {
    /// Canonical VFS path ("" for the root directory).
    pub path: String,
    /// Page kind.
    pub kind: PageKind,
}

/// The computed route set.
#[derive(Debug, Clone)]
pub struct Sitemap {
    config: Config,
    pages: Vec<Page>,
}

/// Percent-encodes one path segment (RFC 3986 unreserved characters kept).
#[must_use]
pub fn encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for b in segment.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// Percent-encodes a whole `/`-separated path, segment by segment.
#[must_use]
pub fn encode_path(path: &str) -> String {
    path.split('/')
        .map(encode_segment)
        .collect::<Vec<_>>()
        .join("/")
}

/// Escapes text for XML element content.
#[must_use]
pub fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

impl Sitemap {
    /// Builds the page set: every file, every ancestor directory and the root.
    #[must_use]
    pub fn from_files<I: IntoIterator<Item = VfsFile>>(files: I, mut config: Config) -> Self {
        config.max_urls = config.max_urls.clamp(1, MAX_URLS);
        let mut pages: BTreeSet<Page> = BTreeSet::new();
        pages.insert(Page {
            path: String::new(),
            kind: PageKind::Dir,
        });
        for f in files {
            let path = canonical_path(&f.path);
            if path.is_empty() {
                continue;
            }
            let mut end = 0;
            while let Some(i) = path[end..].find('/') {
                end += i;
                pages.insert(Page {
                    path: path[..end].to_owned(),
                    kind: PageKind::Dir,
                });
                end += 1;
            }
            pages.insert(Page {
                path,
                kind: PageKind::File,
            });
        }
        Self {
            config,
            pages: pages.into_iter().collect(),
        }
    }

    /// Reads an NDJSON inventory (one JSON object per line with `path`, optional `size`).
    ///
    /// # Errors
    /// Returns the 1-based line number of the first line that is not a JSON object with a string
    /// `path`.
    pub fn from_ndjson(text: &str, config: Config) -> Result<Self, usize> {
        let mut files = Vec::new();
        for (n, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let v: serde_json::Value = serde_json::from_str(line).map_err(|_| n + 1)?;
            let path = v.get("path").and_then(|p| p.as_str()).ok_or(n + 1)?;
            files.push(VfsFile {
                path: path.to_owned(),
                size: v
                    .get("size")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0),
            });
        }
        Ok(Self::from_files(files, config))
    }

    /// Sorted pages.
    #[must_use]
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    /// Number of file pages.
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.pages
            .iter()
            .filter(|p| p.kind == PageKind::File)
            .count()
    }

    /// Absolute URL of a page.
    #[must_use]
    pub fn url(&self, page: &Page) -> String {
        let mut u = format!("{}{}", self.config.base_url, self.config.route_prefix);
        for seg in page.path.split('/').filter(|s| !s.is_empty()) {
            u.push('/');
            u.push_str(&encode_segment(seg));
        }
        if page.kind == PageKind::Dir {
            u.push('/');
        }
        u
    }

    /// Number of sitemap files needed.
    #[must_use]
    pub fn chunk_count(&self) -> usize {
        self.pages.len().div_ceil(self.config.max_urls)
    }

    /// Renders `(file name, xml)` pairs. One chunk gives only `sitemap.xml`; several give
    /// `sitemap.xml` (the index) and `sitemap-1.xml` ... `sitemap-N.xml`.
    #[must_use]
    pub fn render(&self) -> Vec<(String, String)> {
        let lastmod = self
            .config
            .lastmod
            .as_ref()
            .map(|d| format!("<lastmod>{}</lastmod>", xml_escape(d)))
            .unwrap_or_default();
        let chunks: Vec<String> = self
            .pages
            .chunks(self.config.max_urls)
            .map(|chunk| {
                let mut x = String::from(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
                );
                for p in chunk {
                    let _ = writeln!(x, "<url><loc>{}</loc>{lastmod}</url>", xml_escape(&self.url(p)));
                }
                x.push_str("</urlset>\n");
                x
            })
            .collect();
        if chunks.len() <= 1 {
            return vec![(
                "sitemap.xml".into(),
                chunks.into_iter().next().unwrap_or_default(),
            )];
        }
        let mut index = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<sitemapindex xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
        );
        let mut out = Vec::with_capacity(chunks.len() + 1);
        for (i, xml) in chunks.into_iter().enumerate() {
            let name = format!("sitemap-{}.xml", i + 1);
            let _ = writeln!(
                index,
                "<sitemap><loc>{}/{name}</loc>{lastmod}</sitemap>",
                xml_escape(&self.config.base_url)
            );
            out.push((name, xml));
        }
        index.push_str("</sitemapindex>\n");
        out.insert(0, ("sitemap.xml".into(), index));
        out
    }

    /// `robots.txt` pointing at the index.
    #[must_use]
    pub fn robots_txt(&self) -> String {
        format!(
            "User-agent: *\nAllow: /\nSitemap: {}/sitemap.xml\n",
            self.config.base_url
        )
    }

    /// Route tree as JSON for the UI router: `{name, kind, size?, children?}`.
    #[must_use]
    pub fn tree_json(&self, files: &[VfsFile]) -> serde_json::Value {
        let mut root = serde_json::json!({"name": "", "kind": "dir", "children": {}});
        for f in files {
            let path = canonical_path(&f.path);
            let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            let mut node = &mut root;
            for (i, part) in parts.iter().enumerate() {
                let last = i + 1 == parts.len();
                let children = node["children"]
                    .as_object_mut()
                    .expect("dir has children map");
                node = children.entry((*part).to_owned()).or_insert_with(|| {
                    if last {
                        serde_json::json!({"name": part, "kind": "file", "size": f.size})
                    } else {
                        serde_json::json!({"name": part, "kind": "dir", "children": {}})
                    }
                });
            }
        }
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(p: &str) -> VfsFile {
        VfsFile {
            path: p.into(),
            size: 1,
        }
    }

    fn cfg() -> Config {
        Config::new("https://rosegriffon.fr/", "/wiki/files/")
    }

    #[test]
    fn dirs_and_files() {
        let s = Sitemap::from_files(
            [
                f("data/a/b.cfg.bin"),
                f("data\\a/c.png"),
                f("data/a/b.cfg.bin"),
            ],
            cfg(),
        );
        let urls: Vec<_> = s.pages().iter().map(|p| s.url(p)).collect();
        assert_eq!(
            urls,
            [
                "https://rosegriffon.fr/wiki/files/",
                "https://rosegriffon.fr/wiki/files/data/",
                "https://rosegriffon.fr/wiki/files/data/a/",
                "https://rosegriffon.fr/wiki/files/data/a/b.cfg.bin",
                "https://rosegriffon.fr/wiki/files/data/a/c.png",
            ]
        );
        assert_eq!(s.file_count(), 2);
    }

    #[test]
    fn encoding_and_escaping() {
        let s = Sitemap::from_files([f("d/a b&c.png")], cfg());
        let xml = &s.render()[0].1;
        assert!(xml.contains("d/a%20b%26c.png</loc>"));
        assert_eq!(xml_escape("a&b<c"), "a&amp;b&lt;c");
    }

    #[test]
    fn splits_into_index() {
        let mut c = cfg();
        c.max_urls = 3;
        c.lastmod = Some("2026-11-16".into());
        let files: Vec<_> = (0..10).map(|i| f(&format!("d/f{i}"))).collect();
        let s = Sitemap::from_files(files, c);
        assert_eq!(s.pages().len(), 12);
        let out = s.render();
        assert_eq!(out.len(), 5);
        assert_eq!(out[0].0, "sitemap.xml");
        assert!(out[0].1.contains("<sitemapindex"));
        assert!(out[0].1.contains("https://rosegriffon.fr/sitemap-4.xml"));
        assert!(
            out[1..]
                .iter()
                .all(|(_, x)| x.matches("<url>").count() <= 3)
        );
        assert_eq!(
            out[1..]
                .iter()
                .map(|(_, x)| x.matches("<url>").count())
                .sum::<usize>(),
            12
        );
    }

    #[test]
    fn ndjson_and_tree() {
        let text = "{\"path\":\"data/x/y.bin\",\"size\":9}\n\n{\"path\":\"data/z.bin\"}\n";
        let s = Sitemap::from_ndjson(text, cfg()).unwrap();
        assert_eq!(s.file_count(), 2);
        let tree = s.tree_json(&[VfsFile {
            path: "data/x/y.bin".into(),
            size: 9,
        }]);
        assert_eq!(
            tree["children"]["data"]["children"]["x"]["children"]["y.bin"]["size"],
            9
        );
        assert_eq!(Sitemap::from_ndjson("{}\n", cfg()).unwrap_err(), 1);
    }

    #[test]
    fn robots() {
        assert!(
            Sitemap::from_files([], cfg())
                .robots_txt()
                .ends_with("Sitemap: https://rosegriffon.fr/sitemap.xml\n")
        );
    }
}
