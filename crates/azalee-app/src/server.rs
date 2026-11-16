//! HTTP routes.

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use nie_formats::vfs::Vfs;
use nie_sitemap::{Config, Sitemap};
use nie_ui_dioxus::{Theme, UiNode, render_html};

use crate::db::{Db, DbError};
use crate::pages::{self, FileView, PREVIEW_ROWS};

/// Shared state.
pub struct AppState {
    /// PostgreSQL import.
    pub db: Db,
    /// Public origin used in sitemap URLs.
    pub base_url: String,
    /// Prefix of image URLs: `/cdn` when this server serves them, or the CDN origin.
    pub cdn_base: String,
    /// Game VFS for assets (optional).
    pub vfs: Option<Mutex<Vfs>>,
    /// Pre-rendered sitemap files.
    pub sitemaps: Vec<(String, String)>,
    /// `robots.txt`.
    pub robots: String,
    /// Colours.
    pub theme: Theme,
}

impl AppState {
    /// Builds the state and the sitemap from the database listing.
    ///
    /// # Errors
    /// Query errors.
    pub async fn new(db: Db, base_url: &str, cdn_base: &str, vfs: Option<Vfs>) -> Result<Self, DbError> {
        let files = db.all_files().await?;
        let map = Sitemap::from_files(files, Config::new(base_url, pages::FILES_PREFIX));
        Ok(Self {
            db,
            base_url: base_url.trim_end_matches('/').to_owned(),
            cdn_base: cdn_base.trim_end_matches('/').to_owned(),
            vfs: vfs.map(Mutex::new),
            sitemaps: map.render(),
            robots: map.robots_txt(),
            theme: Theme::default(),
        })
    }
}

type Shared = Arc<AppState>;

fn html(ui: &UiNode, title: &str) -> Html<String> {
    Html(render_html(ui, title))
}

fn fail(e: &DbError) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("database error: {e}")).into_response()
}

/// Router with every route.
pub fn router(state: Shared) -> Router {
    Router::new()
        .route("/", get(home))
        .route("/series", get(series_page))
        .route("/series/{slug}", get(game_page))
        .route("/zukan/chara", get(chara_list))
        .route("/zukan/chara/{id}", get(chara_page))
        .route("/wiki/files", get(root_dir))
        .route("/wiki/files/", get(root_dir))
        .route("/wiki/files/{*path}", get(files))
        .route("/api/ui/{*path}", get(api_ui))
        .route("/cdn/static/{*path}", get(asset))
        .route("/{name}", get(top_file))
        .with_state(state)
}

async fn home(State(s): State<Shared>) -> Response {
    let (stats, root) = match tokio::try_join!(s.db.stats(), s.db.list_dir("", 0)) {
        Ok(v) => v,
        Err(e) => return fail(&e),
    };
    html(&pages::home(&stats, &root, &s.theme), "Azalee").into_response()
}

async fn root_dir(State(s): State<Shared>) -> Response {
    dir_page(&s, "").await
}

/// A page ready to serialise: its status, UI tree and document title.
pub struct Built {
    /// HTTP status.
    pub status: StatusCode,
    /// The page as a UI tree (rendered to HTML, or sent as JSON for a client-side Dioxus app).
    pub ui: UiNode,
    /// Document title.
    pub title: String,
}

async fn build_dir(s: &AppState, path: &str) -> Result<Built, Response> {
    match s.db.list_dir(path, 500).await {
        Ok(d) if d.dirs.is_empty() && d.files.is_empty() => {
            Ok(Built { status: StatusCode::NOT_FOUND, ui: pages::not_found(path, &s.theme), title: "Not found".into() })
        }
        Ok(d) => Ok(Built { status: StatusCode::OK, ui: pages::directory(path, &d, &s.theme), title: format!("/{path}") }),
        Err(e) => Err(fail(&e)),
    }
}

async fn dir_page(s: &AppState, path: &str) -> Response {
    match build_dir(s, path).await {
        Ok(b) => (b.status, html(&b.ui, &b.title)).into_response(),
        Err(r) => r,
    }
}

fn is_image(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    [".g4tx", ".png", ".webp", ".jpg", ".jpeg"].iter().any(|e| p.ends_with(e))
}

async fn build_file(s: &AppState, raw: &str) -> Result<Built, Response> {
    let path = nie_formats::vfs_manifest::canonical_path(raw);
    let entry = s.db.entry(&path).await.map_err(|e| fail(&e))?;
    let Some((pack, size)) = entry else {
        return build_dir(s, &path).await;
    };
    let cfg = s.db.file(&path).await.map_err(|e| fail(&e))?;
    let mut tables = Vec::new();
    if let Some(c) = &cfg {
        let res: Result<(), DbError> = async {
            if c.format == "t2b" {
                let (h, total, rows) = s.db.t2b_preview(c.id, PREVIEW_ROWS).await?;
                tables.push(("T2B entries".to_owned(), h, rows, total));
            } else if c.format == "rdbn" {
                for (schema, table, list, count) in s.db.rdbn_tables(c.id).await? {
                    let (h, rows) = s.db.rdbn_preview(&schema, &table, PREVIEW_ROWS).await?;
                    tables.push((list, h, rows, i64::from(count)));
                }
            }
            Ok(())
        }
        .await;
        res.map_err(|e| fail(&e))?;
    }
    let image = is_image(&path).then(|| format!("{}/static/{}.png", s.cdn_base, nie_sitemap::encode_path(&path)));
    let view = FileView { path: &path, pack: &pack, size, cfg: cfg.as_ref(), tables, image };
    Ok(Built { status: StatusCode::OK, ui: pages::file(&view, &s.theme), title: path.clone() })
}

async fn files(State(s): State<Shared>, Path(raw): Path<String>) -> Response {
    match build_file(&s, &raw).await {
        Ok(b) => (b.status, html(&b.ui, &b.title)).into_response(),
        Err(r) => r,
    }
}

/// `/api/ui/<path>`: the same page as JSON (`UiNode`), the data a client-side Dioxus app renders
/// with the shared `NodeView` component.
async fn api_ui(State(s): State<Shared>, Path(raw): Path<String>) -> Response {
    match build_file(&s, &raw).await {
        Ok(b) => (b.status, [(header::CONTENT_TYPE, "application/json")], serde_json::to_string(&b.ui).unwrap_or_default()).into_response(),
        Err(r) => r,
    }
}

async fn top_file(State(s): State<Shared>, Path(name): Path<String>) -> Response {
    if name == "robots.txt" {
        return ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], s.robots.clone()).into_response();
    }
    match s.sitemaps.iter().find(|(n, _)| *n == name) {
        Some((_, xml)) => ([(header::CONTENT_TYPE, "application/xml; charset=utf-8")], xml.clone()).into_response(),
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

/// `/cdn/static/<vfs path>.png`: textures decoded to PNG, images served as stored.
async fn asset(State(s): State<Shared>, Path(raw): Path<String>) -> Response {
    let Some(vfs) = &s.vfs else {
        return (StatusCode::NOT_FOUND, "assets are not mounted").into_response();
    };
    let wanted = nie_formats::vfs_manifest::canonical_path(&raw);
    let (path, as_png) = wanted.strip_suffix(".png").filter(|p| p.ends_with(".g4tx")).map_or((wanted.as_str(), false), |p| (p, true));
    let bytes = match vfs.lock().map(|v| v.read(path)) {
        Ok(Ok(b)) => b,
        _ => return (StatusCode::NOT_FOUND, "no such file").into_response(),
    };
    let (ctype, body) = if as_png {
        let base = nie_formats::g4tx_decode::basename_of(path).to_owned();
        match nie_formats::g4tx_decode::decode_best_to_png(&bytes, &base) {
            Some(png) => ("image/png", png),
            None => return (StatusCode::UNPROCESSABLE_ENTITY, "texture not decodable").into_response(),
        }
    } else {
        let l = path.to_ascii_lowercase();
        (if l.ends_with(".webp") { "image/webp" } else if l.ends_with(".jpg") || l.ends_with(".jpeg") { "image/jpeg" } else { "image/png" }, bytes)
    };
    ([(header::CONTENT_TYPE, ctype), (header::CACHE_CONTROL, "public, max-age=31536000, immutable")], body).into_response()
}

/// Value of `key` in a raw query string.
fn query_value<'a>(q: Option<&'a str>, key: &str) -> Option<&'a str> {
    q?.split('&').find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == key).map(|(_, v)| v))
}

/// `/series?lang=..&theme=..`: the series hub, language from the query or `Accept-Language`,
/// colours from the named token theme.
fn lang_theme(s: &AppState, uri: &axum::http::Uri, headers: &axum::http::HeaderMap) -> (&'static str, Theme) {
    let q = uri.query();
    let accept = headers.get(header::ACCEPT_LANGUAGE).and_then(|v| v.to_str().ok());
    let lang = crate::i18n::negotiate(query_value(q, "lang"), accept);
    let theme = query_value(q, "theme").and_then(Theme::from_tokens).unwrap_or(s.theme);
    (lang, theme)
}

fn localized(status: StatusCode, ui: &UiNode, title: &str, lang: &'static str, theme: &Theme) -> Response {
    let doc = nie_ui_dioxus::render_document(ui, title, lang, theme);
    (status, [(header::CONTENT_LANGUAGE, lang), (header::VARY, "Accept-Language")], Html(doc)).into_response()
}

async fn series_page(State(s): State<Shared>, uri: axum::http::Uri, headers: axum::http::HeaderMap) -> Response {
    let (lang, theme) = lang_theme(&s, &uri, &headers);
    localized(StatusCode::OK, &pages::series(&theme, lang), &crate::i18n::t(lang, "series.title"), lang, &theme)
}

/// `/series/{slug}`: one game of the catalogue.
async fn game_page(State(s): State<Shared>, Path(slug): Path<String>, uri: axum::http::Uri, headers: axum::http::HeaderMap) -> Response {
    let (lang, theme) = lang_theme(&s, &uri, &headers);
    match crate::i18n::game(&slug) {
        Some(g) => localized(StatusCode::OK, &pages::game(&g, &theme, lang), &crate::i18n::t(lang, &format!("game.{slug}")), lang, &theme),
        None => localized(StatusCode::NOT_FOUND, &pages::not_found(&slug, &theme), &crate::i18n::t(lang, "notfound.title"), lang, &theme),
    }
}

/// Game text languages present in the VFS (folders of `text_dir`).
async fn game_langs(s: &AppState) -> Vec<String> {
    let z = crate::i18n::zukan();
    let (prefix, _) = z.text_dir.split_once("{lang}").unwrap_or((&z.text_dir, ""));
    s.db.list_dir(prefix.trim_end_matches('/'), 0).await.map(|d| d.dirs).unwrap_or_default()
}

async fn chara_list(State(s): State<Shared>, uri: axum::http::Uri, headers: axum::http::HeaderMap) -> Response {
    let (lang, theme) = lang_theme(&s, &uri, &headers);
    let z = crate::i18n::zukan();
    let q = uri.query();
    let page_no = query_value(q, "page").and_then(|p| p.parse::<i64>().ok()).unwrap_or(1).max(1);
    let search = query_value(q, "q").map(|v| v.replace('+', " ")).unwrap_or_default();
    match s.db.nouns(&z.names_path(lang), &z.noun_entry, z.full_name_kind, &search, (page_no - 1) * z.page_size, z.page_size).await {
        Ok((total, rows)) => {
            let pages = ((total + z.page_size - 1) / z.page_size).max(1);
            let ui = pages::chara_list(&rows, total, page_no, pages, &search, &theme, lang);
            localized(StatusCode::OK, &ui, &crate::i18n::t(lang, "zukan.chara.title"), lang, &theme)
        }
        Err(e) => fail(&e),
    }
}

async fn chara_page(State(s): State<Shared>, Path(id): Path<i32>, uri: axum::http::Uri, headers: axum::http::HeaderMap) -> Response {
    let (lang, theme) = lang_theme(&s, &uri, &headers);
    let z = crate::i18n::zukan();
    let mut names = Vec::new();
    let mut title = None;
    for gl in game_langs(&s).await {
        match s.db.noun(&z.names_path(&gl), &z.noun_entry, id).await {
            Ok(v) if !v.is_empty() => {
                if gl == lang || title.is_none() {
                    title = v.iter().find(|(k, _)| *k == z.full_name_kind).map(|(_, s)| s.clone()).or(title);
                }
                names.push((gl, v));
            }
            Ok(_) => {}
            Err(e) => return fail(&e),
        }
    }
    match title {
        Some(tt) => {
            let tt = crate::i18n::strip_ruby(&tt);
            localized(StatusCode::OK, &pages::chara(&tt, &names, &theme, lang), &tt, lang, &theme)
        }
        None => localized(StatusCode::NOT_FOUND, &pages::not_found(&id.to_string(), &theme), &crate::i18n::t(lang, "notfound.title"), lang, &theme),
    }
}
