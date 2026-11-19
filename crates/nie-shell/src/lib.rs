//! The contract the aphrody native GPUI shell reads to list, launch and inspect nie.
//!
//! The shell (aphrody-labs/aphrody, `gpui_store`) keeps a catalog of apps (`[[entries]]` with an id,
//! a binary name, platforms and targets) and launches them as processes. nie describes itself in
//! two documents generated from the types below, so the checked-in files cannot drift:
//!
//! - `contract/nie-app.json`: commands, the game source configuration, the inspection endpoint.
//! - `contract/aphrody-store.toml`: an `awesome_gpui_store.toml` override the shell merges into its
//!   catalog (`$XDG_CONFIG_HOME/aphrody/awesome_gpui_store.toml`).
//!
//! Neither document carries game data: nie loads the player's own installation.

#![forbid(unsafe_code)]

use serde::Serialize;

/// Contract schema version, bumped on any breaking change of [`AppContract`].
pub const SCHEMA_VERSION: u32 = 1;

/// Port of the Bevy Remote Protocol (`bevy_remote` default) a running match exposes for inspection.
pub const REMOTE_PORT: u16 = 15702;

/// A runnable entry point of the app.
#[derive(Debug, Clone, Serialize)]
pub struct Command {
    /// Stable identifier the shell stores in its settings.
    pub id: &'static str,
    /// Cargo package that builds the binary.
    pub package: &'static str,
    /// Binary name (`.exe` is added on Windows by the shell).
    pub binary: &'static str,
    /// Arguments passed to the binary.
    pub args: &'static [&'static str],
    /// `window` opens its own GPU window, `stdio` speaks MCP on stdin/stdout, `service` serves HTTP.
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
    /// App identifier in the shell catalog.
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
    /// Bevy Remote Protocol port of a running match.
    pub remote_port: u16,
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
                id: "game",
                package: "nie-game",
                binary: "nie-game",
                args: &[],
                mode: "window",
                headless: false,
                platforms: DESKTOP,
            },
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
            schemes: &["ssh", "sftp", "r2", "s3", "gdrive", "rclone", "https"],
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
        remote_port: REMOTE_PORT,
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

#[derive(Serialize)]
struct StoreCatalog {
    schema_version: u32,
    total_entries: usize,
    entries: Vec<StoreEntry>,
}

/// The fields of the shell's `StoreEntry` that nie sets; the rest take the shell's defaults.
#[derive(Serialize)]
struct StoreEntry {
    id: &'static str,
    name: &'static str,
    tagline: &'static str,
    description: &'static str,
    category: &'static str,
    subcategory: &'static str,
    repo: &'static str,
    homepage: &'static str,
    publisher: &'static str,
    crate_name: &'static str,
    bin_name: &'static str,
    version: &'static str,
    license: &'static str,
    status: &'static str,
    platforms: Vec<&'static str>,
    tags: Vec<&'static str>,
    first_party: bool,
}

/// The `awesome_gpui_store.toml` override that lists nie in the shell catalog.
///
/// # Panics
/// Never: the entry contains only plain data.
#[must_use]
pub fn store_toml() -> String {
    let catalog = StoreCatalog {
        schema_version: 1,
        total_entries: 1,
        entries: vec![StoreEntry {
            id: "nie",
            name: "nie",
            tagline: "Self-hosted Inazuma Eleven engine and studio",
            description: "Bevy-based engine, studio and asset streamer that loads game data from the player's own installation through VFS layers and CRI CPK archives; no game asset is shipped.",
            category: "app",
            subcategory: "Games",
            repo: env!("CARGO_PKG_REPOSITORY"),
            homepage: env!("CARGO_PKG_REPOSITORY"),
            publisher: "Aphrody Labs",
            crate_name: "nie-cli",
            bin_name: "nie",
            version: env!("CARGO_PKG_VERSION"),
            license: "MIT OR Apache-2.0",
            status: "active",
            platforms: vec!["windows", "linux", "macos", "web"],
            tags: vec!["bevy", "wgpu", "webgpu", "rust", "inazuma-eleven"],
            first_party: true,
        }],
    };
    format!(
        "# Generated by `nie-shell store`; do not edit. Drop into $XDG_CONFIG_HOME/aphrody/awesome_gpui_store.toml.\n{}",
        toml::to_string(&catalog).expect("store entry serializes")
    )
}
