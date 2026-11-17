//! `azalee`: reads `AZALEE_DATABASE_URL`, optional `AZALEE_GAME_SOURCE` (assets), `AZALEE_BIND`
//! (default `0.0.0.0:8080`), `AZALEE_BASE_URL` (sitemap origin) and `AZALEE_CDN_BASE`
//! (default `/cdn`, served here from the VFS; set to the CDN origin in production).

use std::sync::Arc;

use azalee_app::db::Db;
use azalee_app::server::{AppState, router};
use nie_formats::vfs::Vfs;

fn env(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_owned())
}

#[tokio::main]
async fn main() {
    let url = std::env::var("AZALEE_DATABASE_URL").expect("AZALEE_DATABASE_URL");
    let db = Db::connect(&url).await.expect("connect to PostgreSQL");
    let vfs = std::env::var("AZALEE_GAME_SOURCE").ok().map(|src| {
        let mut v = Vfs::new();
        v.init_source(&src).expect("init VFS");
        v.discover_extra_cpks();
        v
    });
    let state = AppState::new(
        db,
        &env("AZALEE_BASE_URL", "http://localhost:8080"),
        &env("AZALEE_CDN_BASE", "/cdn"),
        vfs,
    )
    .await
    .expect("load listing");
    let bind = env("AZALEE_BIND", "0.0.0.0:8080");
    let listener = tokio::net::TcpListener::bind(&bind).await.expect("bind");
    eprintln!(
        "azalee listening on {bind}, {} sitemap files",
        state.sitemaps.len()
    );
    axum::serve(listener, router(Arc::new(state)))
        .await
        .expect("serve");
}
