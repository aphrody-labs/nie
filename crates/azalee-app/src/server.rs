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
        .route("/wiki/files", get(root_dir))
        .route("/wiki/files/", get(root_dir))
        .route("/wiki/files/{*path}", get(files))
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

async fn dir_page(s: &AppState, path: &str) -> Response {
    match s.db.list_dir(path, 500).await {
        Ok(d) if d.dirs.is_empty() && d.files.is_empty() => {
            (StatusCode::NOT_FOUND, html(&pages::not_found(path, &s.theme), "Not found")).into_response()
        }
        Ok(d) => html(&pages::directory(path, &d, &s.theme), &format!("/{path}")).into_response(),
        Err(e) => fail(&e),
    }
}

fn is_image(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    [".g4tx", ".png", ".webp", ".jpg", ".jpeg"].iter().any(|e| p.ends_with(e))
}

async fn files(State(s): State<Shared>, Path(raw): Path<String>) -> Response {
    let path = nie_formats::vfs_manifest::canonical_path(&raw);
    let entry = match s.db.entry(&path).await {
        Ok(e) => e,
        Err(e) => return fail(&e),
    };
    let Some((pack, size)) = entry else {
        return dir_page(&s, &path).await;
    };
    let cfg = match s.db.file(&path).await {
        Ok(c) => c,
        Err(e) => return fail(&e),
    };
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
        if let Err(e) = res {
            return fail(&e);
        }
    }
    let image = is_image(&path).then(|| format!("{}/static/{}.png", s.cdn_base, nie_sitemap::encode_path(&path)));
    let view = FileView { path: &path, pack: &pack, size, cfg: cfg.as_ref(), tables, image };
    html(&pages::file(&view, &s.theme), &path).into_response()
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
