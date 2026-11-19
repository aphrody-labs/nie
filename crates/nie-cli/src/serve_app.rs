//! The browser app and the byte stream behind it.
//!
//! - `/app` and `/app/<file>` serve the bundle embedded by `nie-app`: immutable, content-hashed
//!   files, brotli when the client accepts it, and an entry page that is revalidated.
//! - `/v1/vfs/stream` serves byte ranges of the installation's own files (the encrypted index,
//!   the packs, loose files) to a page that decodes them itself. They are the game, not data
//!   derived from it, so the route follows the raw-bytes rule: closed (404) unless
//!   `NIE_RAW_VFS_TOKEN` is configured, then open to that bearer token only.

use std::collections::HashMap;
use std::sync::Arc;

use nie_explore::raw_access::{self, Token};
use nie_formats::vfs::Vfs;
use serde_json::json;

use crate::serve_cmd::HttpResponse;

/// Largest slice one stream answer carries.
const MAX_SLICE: u64 = 128 * 1024 * 1024;

const IMMUTABLE: &str = "public, max-age=31536000, immutable";

/// `/app` (`name` empty) or `/app/<name>`.
pub(crate) fn app(name: Option<&str>, headers: &HashMap<String, String>) -> HttpResponse {
    if !nie_app::is_embedded() {
        return HttpResponse::json(
            404,
            "Not Found",
            &json!({
                "error": "this binary embeds no browser app",
                "reason": "build it with `just app-pack`, then rebuild nie with NIE_APP_DIST pointing at the output",
            }),
        );
    }
    let asset = match name {
        None => nie_app::entry(),
        Some(name) => nie_app::get(name),
    };
    let Some(asset) = asset else {
        return HttpResponse::json(404, "Not Found", &json!({ "error": "no such app file" }));
    };
    let accepts_brotli = headers
        .get("accept-encoding")
        .is_some_and(|v| v.split(',').any(|e| e.trim().starts_with("br")));
    let (body, encoded) = match asset.brotli {
        Some(packed) if accepts_brotli => (packed, true),
        _ => (asset.bytes, false),
    };
    let cache = if asset.immutable { IMMUTABLE } else { "no-cache" };
    let mut response = HttpResponse::bytes(200, "OK", asset.content_type, cache, body.to_vec())
        .with_header("Vary", "Accept-Encoding");
    if encoded {
        response = response.with_header("Content-Encoding", "br");
    }
    response
}

/// True when the client asks for a page (a browser at `/`), not JSON.
pub(crate) fn wants_page(headers: &HashMap<String, String>) -> bool {
    headers
        .get("accept")
        .is_some_and(|v| v.split(',').next().is_some_and(|t| t.trim() == "text/html"))
}

/// The redirect from `/` to the app, for browsers.
pub(crate) fn to_app() -> HttpResponse {
    HttpResponse::bytes(302, "Found", "text/plain; charset=utf-8", "no-store", Vec::new())
        .with_header("Location", "/app/")
}

fn refused() -> HttpResponse {
    HttpResponse::json(404, "Not Found", &json!({ "error": "route not found" }))
}

/// `bytes=a-b`, `bytes=a-` or `bytes=-n` against a file of `total` bytes: `(first, last)`
/// inclusive, `None` when the header is malformed or unsatisfiable.
fn parse_range(header: &str, total: u64) -> Option<(u64, u64)> {
    let spec = header.trim().strip_prefix("bytes=")?;
    if spec.contains(',') {
        return None;
    }
    let (first, last) = spec.split_once('-')?;
    let (first, last) = match (first.trim(), last.trim()) {
        ("", n) => {
            let n: u64 = n.parse().ok()?;
            (total.checked_sub(n)?, total.checked_sub(1)?)
        }
        (a, "") => (a.parse().ok()?, total.checked_sub(1)?),
        (a, b) => (a.parse().ok()?, b.parse::<u64>().ok()?.min(total.checked_sub(1)?)),
    };
    (first <= last && last < total).then_some((first, last))
}

/// `/v1/vfs/stream?path=`.
pub(crate) fn stream(
    vfs: &Arc<Vfs>,
    token: Option<&Token>,
    headers: &HashMap<String, String>,
    path: Option<&str>,
) -> HttpResponse {
    if !raw_access::allows(token, headers.get("authorization").map(String::as_str)) {
        return refused();
    }
    let Some(path) = path else {
        return HttpResponse::json(
            400,
            "Bad Request",
            &json!({ "error": "missing path query param" }),
        );
    };
    // The total length comes with the first byte, so ask for it before the range is known.
    let probe = match vfs.raw_range(path, 0, 1) {
        Ok(probe) => probe,
        Err(error) => {
            return HttpResponse::json(
                404,
                "Not Found",
                &json!({ "error": error.to_string(), "path": path }),
            );
        }
    };
    let total = probe.total;
    let wanted = match headers.get("range") {
        None => {
            if total > MAX_SLICE {
                return unsatisfiable(total);
            }
            None
        }
        Some(header) => match parse_range(header, total) {
            Some(range) => Some(range),
            None => return unsatisfiable(total),
        },
    };
    let (first, last) = wanted.unwrap_or((0, total.saturating_sub(1)));
    let length = if total == 0 { 0 } else { last - first + 1 };
    if length > MAX_SLICE {
        return unsatisfiable(total);
    }
    let bytes = if length == 0 {
        Vec::new()
    } else {
        match vfs.raw_range(path, first, usize::try_from(length).unwrap_or(usize::MAX)) {
            Ok(slice) => slice.bytes,
            Err(error) => {
                return HttpResponse::json(
                    500,
                    "Internal Server Error",
                    &json!({ "error": error.to_string(), "path": path }),
                );
            }
        }
    };
    let response = match wanted {
        Some(_) => HttpResponse::bytes(
            206,
            "Partial Content",
            "application/octet-stream",
            "no-store",
            bytes,
        )
        .with_header("Content-Range", format!("bytes {first}-{last}/{total}")),
        None => HttpResponse::bytes(
            200,
            "OK",
            "application/octet-stream",
            "no-store",
            bytes,
        ),
    };
    response.with_header("Accept-Ranges", "bytes")
}

fn unsatisfiable(total: u64) -> HttpResponse {
    HttpResponse::json(
        416,
        "Range Not Satisfiable",
        &json!({ "error": "range not satisfiable", "size": total }),
    )
    .with_header("Content-Range", format!("bytes */{total}"))
    .with_header("Accept-Ranges", "bytes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_follow_rfc_9110() {
        assert_eq!(parse_range("bytes=0-0", 10), Some((0, 0)));
        assert_eq!(parse_range("bytes=2-5", 10), Some((2, 5)));
        assert_eq!(parse_range("bytes=2-", 10), Some((2, 9)));
        assert_eq!(parse_range("bytes=-3", 10), Some((7, 9)));
        assert_eq!(parse_range("bytes=8-99", 10), Some((8, 9)));
        assert_eq!(parse_range("bytes=10-", 10), None);
        assert_eq!(parse_range("bytes=5-2", 10), None);
        assert_eq!(parse_range("bytes=0-1,4-5", 10), None);
        assert_eq!(parse_range("items=0-1", 10), None);
        assert_eq!(parse_range("bytes=0-0", 0), None);
    }

    #[test]
    fn a_browser_asks_for_a_page_and_a_probe_does_not() {
        let browser: HashMap<String, String> = [(
            "accept".to_owned(),
            "text/html,application/xhtml+xml,*/*;q=0.8".to_owned(),
        )]
        .into();
        assert!(wants_page(&browser));
        assert!(!wants_page(&HashMap::new()));
        let curl: HashMap<String, String> = [("accept".to_owned(), "*/*".to_owned())].into();
        assert!(!wants_page(&curl));
    }

    const TOKEN: &str = "0123456789abcdef0123456789abcdef-token";

    fn installation() -> (tempfile::TempDir, Arc<Vfs>) {
        let dir = tempfile::tempdir().unwrap();
        let files = nie_formats::synthetic::sample_files(24);
        for (path, bytes) in nie_formats::synthetic::installation(&files, 3).unwrap() {
            let target = dir.path().join(path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, bytes).unwrap();
        }
        let mut vfs = Vfs::new();
        vfs.init(dir.path().join("data")).unwrap();
        (dir, Arc::new(vfs))
    }

    fn bearer(token: &str) -> HashMap<String, String> {
        [("authorization".to_owned(), format!("Bearer {token}"))].into()
    }

    fn header<'a>(response: &'a HttpResponse, name: &str) -> Option<&'a str> {
        response
            .headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    #[test]
    fn the_stream_is_closed_without_the_token() {
        let (_dir, vfs) = installation();
        let token = Token::new(TOKEN);
        let path = Some("cpk_list.cfg.bin");
        // No token configured: closed for everyone, even with a bearer.
        assert_eq!(stream(&vfs, None, &bearer(TOKEN), path).status, 404);
        // A token configured: a missing or wrong bearer is refused the same way.
        assert_eq!(stream(&vfs, token.as_ref(), &HashMap::new(), path).status, 404);
        assert_eq!(stream(&vfs, token.as_ref(), &bearer("x".repeat(40).as_str()), path).status, 404);
        assert_eq!(stream(&vfs, token.as_ref(), &bearer(TOKEN), path).status, 200);
    }

    #[test]
    fn ranges_of_a_pack_are_the_bytes_of_the_file() {
        let (dir, vfs) = installation();
        let token = Token::new(TOKEN);
        let pack = std::fs::read_dir(dir.path().join("data/packs"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        let name = pack.file_name().to_string_lossy().into_owned();
        let on_disk = std::fs::read(pack.path()).unwrap();
        let path = format!("packs/{name}");
        let ask = |range: &str| {
            let mut headers = bearer(TOKEN);
            headers.insert("range".to_owned(), range.to_owned());
            stream(&vfs, token.as_ref(), &headers, Some(&path))
        };

        let head = ask("bytes=0-15");
        assert_eq!(head.status, 206);
        assert_eq!(head.body, &on_disk[..16]);
        assert_eq!(
            header(&head, "Content-Range").unwrap(),
            format!("bytes 0-15/{}", on_disk.len())
        );
        assert_eq!(header(&head, "Accept-Ranges"), Some("bytes"));

        let tail = ask("bytes=-32");
        assert_eq!(tail.body, &on_disk[on_disk.len() - 32..]);

        let past = ask(&format!("bytes={}-", on_disk.len()));
        assert_eq!(past.status, 416);
        assert_eq!(
            header(&past, "Content-Range").unwrap(),
            format!("bytes */{}", on_disk.len())
        );
    }

    #[test]
    fn only_the_index_the_packs_and_declared_loose_files_are_reachable() {
        let (_dir, vfs) = installation();
        let token = Token::new(TOKEN);
        let get = |path: &str| stream(&vfs, token.as_ref(), &bearer(TOKEN), Some(path)).status;
        assert_eq!(get("cpk_list.cfg.bin"), 200);
        assert_eq!(get("loose/clip_010.usm"), 200);
        for refused in [
            "../Cargo.toml",
            "packs/../cpk_list.cfg.bin",
            "/etc/passwd",
            "packs/sub/x.cpk",
            "packs/readme.txt",
            "common/menu/file_000.bin",
            "",
        ] {
            assert_eq!(get(refused), 404, "{refused}");
        }
    }

    #[test]
    fn a_binary_without_the_bundle_says_so() {
        if !nie_app::is_embedded() {
            let response = app(None, &HashMap::new());
            assert_eq!(response.status, 404);
            assert!(String::from_utf8_lossy(&response.body).contains("NIE_APP_DIST"));
        }
    }
}
