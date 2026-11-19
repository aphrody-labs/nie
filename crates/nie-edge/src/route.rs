//! The routing table of the edge: which upstream answers a path, which paths never leave the
//! host, and which decode-heavy routes carry limits.
//!
//! The table is a list of first path segments. The tests compare it with the route table of
//! `nie-model-serve` (`nie_model_serve::routes::ROUTES`) and with the one of `nie serve`
//! (`crates/nie-cli/src/serve_routes.rs`), so a route added to either server without a decision
//! here fails the build instead of reaching the wrong upstream.

/// Where a request goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// `nie serve`: JSON API, wiki, `/openapi.json`.
    Core,
    /// `nie-model-serve`: decoded media derived from the VFS.
    Media,
    /// Never public: the VFS bridge and the game's untouched bytes. Answered 404 at the edge.
    Blocked,
    /// The edge itself (`/health`).
    Edge,
}

/// First path segments served by `nie-model-serve`.
pub const MEDIA_SEGMENTS: &[&str] = &[
    "tex",
    "tex-info",
    "cfg",
    "typed",
    "audio",
    "audio-info",
    "video",
    "avatar",
    "menu-tree",
    "lip",
    "ui",
    "icons",
    "export",
];

/// First segments that start with one of these belong to `nie-model-serve` too (`model-full`,
/// `model-chr`, ...).
pub const MEDIA_PREFIXES: &[&str] = &["model-"];

/// First path segments that never reach an upstream: `/vfs/*` lists and hashes the game's files,
/// `/raw` and `/depot` return their untouched bytes.
pub const BLOCKED_SEGMENTS: &[&str] = &["vfs", "raw", "depot"];

/// A decode-heavy route family with its own rate and concurrency limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limited {
    /// `/model-*`: model assembly.
    Model,
    /// `/export`: format conversion.
    Export,
    /// `/video`: remux and soundtrack extraction.
    Video,
    /// Wiki search: `/v1/wiki/*` and `/wiki/*` with a `q` parameter.
    WikiSearch,
}

impl Limited {
    /// Every limited family.
    pub const ALL: [Limited; 4] = [
        Limited::Model,
        Limited::Export,
        Limited::Video,
        Limited::WikiSearch,
    ];

    /// Stable name, used in the access log and in the configuration.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Limited::Model => "model",
            Limited::Export => "export",
            Limited::Video => "video",
            Limited::WikiSearch => "wiki_search",
        }
    }

    /// Position in [`Limited::ALL`].
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Limited::Model => 0,
            Limited::Export => 1,
            Limited::Video => 2,
            Limited::WikiSearch => 3,
        }
    }
}

/// The path as the upstream servers will interpret it: percent-decoded, backslashes read as
/// slashes, `.` and empty segments removed. `None` for a path that cannot be classified safely
/// (`..` segments, NUL, invalid UTF-8 after decoding): the edge answers those 400 or 404.
#[must_use]
pub fn normalize(path: &str) -> Option<String> {
    let decoded = percent_decode(path)?;
    let mut out = String::with_capacity(decoded.len() + 1);
    for segment in decoded.split(['/', '\\']) {
        match segment {
            "" | "." => {}
            ".." => return None,
            s if s.contains('\0') => return None,
            s => {
                out.push('/');
                out.push_str(s);
            }
        }
    }
    if out.is_empty() {
        out.push('/');
    }
    Some(out)
}

fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hi = hex(*bytes.get(i + 1)?)?;
            let lo = hex(*bytes.get(i + 2)?)?;
            out.push(hi << 4 | lo);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

const fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// The first path segment without a file extension: `/menu-tree.json` and `/menu-tree/x` both
/// belong to `menu-tree`.
fn first_segment(normalized: &str) -> &str {
    let segment = normalized
        .trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or("");
    segment.split_once('.').map_or(segment, |(stem, _)| stem)
}

/// The target of a request path. A path that cannot be normalized is blocked.
#[must_use]
pub fn classify(path: &str) -> Target {
    let Some(normalized) = normalize(path) else {
        return Target::Blocked;
    };
    let segment = first_segment(&normalized).to_ascii_lowercase();
    if normalized == "/health" {
        Target::Edge
    } else if BLOCKED_SEGMENTS.contains(&segment.as_str()) {
        Target::Blocked
    } else if MEDIA_SEGMENTS.contains(&segment.as_str())
        || MEDIA_PREFIXES.iter().any(|p| segment.starts_with(p))
    {
        Target::Media
    } else {
        Target::Core
    }
}

fn has_query_key(query: Option<&str>, key: &str) -> bool {
    query.is_some_and(|q| {
        q.split('&').any(|pair| {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            k == key && !v.is_empty()
        })
    })
}

/// The limited family of a request, if any.
#[must_use]
pub fn limited(path: &str, query: Option<&str>) -> Option<Limited> {
    let normalized = normalize(path)?;
    let segment = first_segment(&normalized).to_ascii_lowercase();
    if segment.starts_with("model-") {
        Some(Limited::Model)
    } else if segment == "export" {
        Some(Limited::Export)
    } else if segment == "video" {
        Some(Limited::Video)
    } else if (normalized.starts_with("/v1/wiki") || segment == "wiki") && has_query_key(query, "q")
    {
        Some(Limited::WikiSearch)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nie_model_serve::routes::{ROUTES, sample};

    fn sample_path(pattern: &str) -> String {
        pattern
            .replace("{root}", "map")
            .replace(['{', '}'], "")
            .replace("lang", "fr")
            .replace("key", "x")
    }

    #[test]
    fn model_serve_routes_reach_media() {
        for def in ROUTES {
            let paths = std::iter::once(sample(def)).chain(def.alias.map(str::to_owned));
            for path in paths {
                let want = match path.as_str() {
                    "/health" => Target::Edge,
                    "/openapi.json" => Target::Core,
                    p if p.starts_with("/vfs/") || p.starts_with("/raw/") => Target::Blocked,
                    _ => Target::Media,
                };
                assert_eq!(classify(&path), want, "route {} ({path})", def.pattern);
            }
        }
    }

    #[test]
    fn every_media_prefix_is_a_model_serve_route() {
        let firsts: Vec<String> = ROUTES
            .iter()
            .flat_map(|d| std::iter::once(d.pattern).chain(d.alias))
            .map(|p| first_segment(&normalize(&sample_path(p)).unwrap()).to_owned())
            .collect();
        for seg in MEDIA_SEGMENTS {
            assert!(firsts.iter().any(|f| f == seg), "no route under /{seg}");
        }
        for prefix in MEDIA_PREFIXES {
            assert!(
                firsts.iter().any(|f| f.starts_with(prefix)),
                "no route under /{prefix}*"
            );
        }
        for seg in ["vfs", "raw"] {
            assert!(
                firsts.iter().any(|f| f == seg),
                "blocked /{seg} is not a model-serve route anymore"
            );
        }
    }

    #[test]
    fn nie_serve_routes_stay_on_core() {
        let source = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../nie-cli/src/serve_routes.rs"
        ))
        .expect("nie-cli serve_routes.rs");
        let table = source
            .split("const ROUTES")
            .nth(1)
            .and_then(|rest| rest.split("\n];").next())
            .expect("ROUTES table");
        let patterns: Vec<&str> = table
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with("\"/") && l.ends_with("\","))
            .map(|l| l.trim_end_matches(',').trim_matches('"'))
            .collect();
        assert!(patterns.len() > 15, "parsed {} patterns", patterns.len());
        for pattern in patterns {
            let path = sample_path(pattern);
            let want = if path == "/health" {
                Target::Edge
            } else {
                Target::Core
            };
            assert_eq!(classify(&path), want, "nie serve route {pattern} ({path})");
        }
    }

    #[test]
    fn private_paths_cannot_be_smuggled() {
        for path in [
            "/raw/data/x",
            "/RAW/data/x",
            "/%72aw/data/x",
            "//raw/x",
            "/./vfs/ls",
            "/tex/../raw/x",
            "/a/%2e%2e/raw",
            "/vfs%2Fls",
            "/\\vfs/ls",
            "/depot/1/2",
            "/tex/%00",
            "/%zz",
        ] {
            assert_eq!(classify(path), Target::Blocked, "{path}");
        }
        assert_eq!(classify("/vfs"), Target::Blocked);
        assert_eq!(classify("/export/data/x.cfg.bin"), Target::Media);
        assert_eq!(classify("/v1/vfs/find"), Target::Core);
        assert_eq!(classify("/"), Target::Core);
        assert_eq!(classify("/health"), Target::Edge);
        assert_eq!(classify("/health/"), Target::Edge);
    }

    #[test]
    fn limits_apply_to_decode_heavy_routes() {
        assert_eq!(limited("/model-full/x.glb", None), Some(Limited::Model));
        assert_eq!(limited("/model-tree/map/a", None), Some(Limited::Model));
        assert_eq!(limited("/export/data/x", None), Some(Limited::Export));
        assert_eq!(limited("/video/a.usm", None), Some(Limited::Video));
        assert_eq!(
            limited("/v1/wiki/characters", Some("q=mark&limit=5")),
            Some(Limited::WikiSearch)
        );
        assert_eq!(limited("/v1/wiki/characters", Some("limit=5")), None);
        assert_eq!(limited("/v1/wiki/characters", Some("q=")), None);
        assert_eq!(
            limited("/wiki/fr/skills", Some("q=a")),
            Some(Limited::WikiSearch)
        );
        assert_eq!(limited("/tex/a.png", None), None);
    }
}
