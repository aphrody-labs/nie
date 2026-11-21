//! The contract a native app shell reads to list, launch and inspect nie.
//!
//! A shell keeps a catalog of apps (an id, a binary name, platforms and targets) and launches them
//! as processes. nie describes itself in one document generated from the types below, so the
//! checked-in file cannot drift:
//!
//! - `contract/nie-app.json`: commands, the game source configuration and build targets.
//!
//! The document carries no game data: nie loads the player's own installation.

#![forbid(unsafe_code)]

use serde::Serialize;

/// Contract schema version, bumped on any breaking change of [`AppContract`].
pub const SCHEMA_VERSION: u32 = 2;

/// A runnable entry point of the app.
#[derive(Debug, Clone, Serialize)]
pub struct Command {
    /// Stable identifier of the entry point.
    pub id: &'static str,
    /// Cargo package that builds the binary.
    pub package: &'static str,
    /// Binary name (`.exe` is added on Windows).
    pub binary: &'static str,
    /// Arguments passed to the binary.
    pub args: &'static [&'static str],
    /// `stdio` speaks MCP on stdin/stdout, `service` serves HTTP.
    pub mode: &'static str,
    /// Whether it runs without a display.
    pub headless: bool,
    /// Operating systems it is built for.
    pub platforms: &'static [&'static str],
}

/// How the game installation is found and streamed.
#[derive(Debug, Clone, Serialize)]
pub struct GameSource {
    /// Asset source name in Bevy paths: `game://data/...`.
    pub asset_source: &'static str,
    /// Environment variable naming a TOML config for `nie-assets`.
    pub config_env: &'static str,
    /// Environment variable that sets the source directly.
    pub source_env: &'static str,
    /// Accepted source URI schemes; a bare path is a local installation.
    pub schemes: &'static [&'static str],
    /// Environment variables of the read-through cache.
    pub cache_env: &'static [&'static str],
    /// Game files are never bundled, copied into the repository or uploaded.
    pub ships_game_assets: bool,
}

/// A build target.
#[derive(Debug, Clone, Serialize)]
pub struct Target {
    /// Rust target triple, or `native-*` for the host triples of the CI matrix.
    pub triple: &'static str,
    /// `native` or `web`.
    pub kind: &'static str,
    /// Graphics API.
    pub graphics: &'static str,
}

/// The full contract.
#[derive(Debug, Clone, Serialize)]
pub struct AppContract {
    /// Contract schema version.
    pub schema: u32,
    /// App identifier.
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// App version (the workspace version).
    pub version: &'static str,
    /// Source repository.
    pub repository: &'static str,
    /// Runnable entry points.
    pub commands: Vec<Command>,
    /// Game source configuration.
    pub game_source: GameSource,
    /// Build targets.
    pub targets: Vec<Target>,
}

const DESKTOP: &[&str] = &["windows", "linux", "macos"];

/// The contract of this build.
#[must_use]
pub fn contract() -> AppContract {
    AppContract {
        schema: SCHEMA_VERSION,
        id: "nie",
        name: "nie",
        version: env!("CARGO_PKG_VERSION"),
        repository: env!("CARGO_PKG_REPOSITORY"),
        commands: vec![
            Command {
                id: "serve",
                package: "nie-cli",
                binary: "nie",
                args: &["serve"],
                mode: "service",
                headless: true,
                platforms: DESKTOP,
            },
            Command {
                id: "mcp",
                package: "nie-cli",
                binary: "nie-mcp",
                args: &[],
                mode: "stdio",
                headless: true,
                platforms: DESKTOP,
            },
        ],
        game_source: GameSource {
            asset_source: "game",
            config_env: "NIE_ASSETS_CONFIG",
            source_env: "NIE_GAME_SOURCE",
            schemes: &["ssh", "sftp", "r2", "s3", "gdrive", "rclone", "http", "https"],
            cache_env: &["NIE_ASSETS_CACHE_DIR", "NIE_ASSETS_CACHE_MAX_MIB"],
            ships_game_assets: false,
        },
        targets: vec![
            Target {
                triple: "native-windows",
                kind: "native",
                graphics: "wgpu (DX12, Vulkan)",
            },
            Target {
                triple: "native-linux",
                kind: "native",
                graphics: "wgpu (Vulkan)",
            },
            Target {
                triple: "native-macos",
                kind: "native",
                graphics: "wgpu (Metal)",
            },
            Target {
                triple: "wasm32-unknown-unknown",
                kind: "web",
                graphics: "WebGPU",
            },
        ],
    }
}

/// The contract as pretty JSON, newline-terminated.
///
/// # Panics
/// Never: the contract contains only plain data.
#[must_use]
pub fn contract_json() -> String {
    let mut json = serde_json::to_string_pretty(&contract()).expect("contract serializes");
    json.push('\n');
    json
}
