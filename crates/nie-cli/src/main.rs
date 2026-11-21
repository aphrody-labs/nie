#![cfg(any(feature = "cloud", feature = "host"))]

//! nie: command line of the engine, the VFS tools, the wiki, mods, saves, serving and the nie.exe reverse-engineering and recompilation toolchain.
#![forbid(unsafe_code)]
#![allow(clippy::pedantic)]

mod output;
mod plugins;
#[cfg(feature = "host")]
pub use output::CapturedCommand;

// The CLI is also an in-process library for the native MCP server. These
// macros preserve normal terminal output while allowing one command running
// on its dedicated CLI thread to capture its complete response without ever
// redirecting the process-wide stdout used by MCP JSON-RPC.
macro_rules! println {
    () => {{ $crate::output::write_stdout(format_args!(""), true) }};
    ($($arg:tt)*) => {{ $crate::output::write_stdout(format_args!($($arg)*), true) }};
}
#[cfg(feature = "host")]
macro_rules! eprint {
    ($($arg:tt)*) => {{ $crate::output::write_stderr(format_args!($($arg)*), false) }};
}
macro_rules! eprintln {
    () => {{ $crate::output::write_stderr(format_args!(""), true) }};
    ($($arg:tt)*) => {{ $crate::output::write_stderr(format_args!($($arg)*), true) }};
}

#[cfg(feature = "host")]
mod atlas_cmd;
#[cfg(feature = "host")]
mod avatar_cmd;
mod decode_cmd;
mod deps_audit;
mod deps_cmd;
#[cfg(feature = "host")]
mod exe_lint;
#[cfg(feature = "host")]
mod fm_cmd;
#[cfg(feature = "term")]
mod term_cmd;
#[cfg(feature = "host")]
mod icons_cmd;
#[cfg(feature = "host")]
mod img_cmd;
#[cfg(feature = "host")]
mod lua_audit_cmd;
#[cfg(feature = "host")]
mod lua_cmd;
#[cfg(feature = "host")]
mod lua_run_cmd;
#[cfg(feature = "host")]
mod mcp;
#[cfg(feature = "host")]
mod mem_lua;
#[cfg(feature = "host")]
mod menu_predecode;
#[cfg(feature = "host")]
mod mod_cmd;
#[cfg(feature = "host")]
mod mode_index;
#[cfg(feature = "host")]
mod ocgen_cmd;
#[cfg(feature = "host")]
mod render_cmd;
#[cfg(feature = "host")]
mod search_cmd;
#[cfg(feature = "host")]
mod seed_ui;
mod serve_app;
mod serve_cmd;
mod serve_routes;
mod serve_wiki;
mod steam_turbo;
#[cfg(feature = "host")]
mod strings_cmd;
#[cfg(feature = "host")]
pub mod vfs_bundle;
#[cfg(feature = "host")]
mod vfs_lint;
#[cfg(feature = "host")]
mod video_cmd;
#[cfg(feature = "host")]
mod vn_cmd;
mod wiki_cmd;
mod wiki_verify;

#[cfg(feature = "host")]
use std::io::Write;
use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
#[cfg(feature = "host")]
use sha2::{Digest, Sha256};

/// Image base de nie.exe (PE32+ Level-5).
#[cfg(feature = "host")]
const NIE_IMAGE_BASE: i64 = 0x1_4000_0000;

/// Parse a decimal or hexadecimal (`0x...`) address.
fn parse_addr(s: &str) -> Result<i64, String> {
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(hex, 16)
            .map(|v| v as i64)
            .map_err(|e| e.to_string()),
        None => s.parse::<i64>().map_err(|e| e.to_string()),
    }
}

#[derive(Parser)]
#[command(
    name = "nie",
    version,
    about = "nie: engine, VFS, mod and reverse-engineering toolchain for Inazuma Eleven: Victory Road"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// HTTP server: remote VFS, formats, game-data queries, /health and /ready probes.
    #[command(name = "serve")]
    Serve(serve_cmd::ServeArgs),
    /// Characters, techniques and teams of the game, computed from the VFS (`nie-wiki-vfs`).
    Wiki(wiki_cmd::WikiArgs),
    /// Scaffold a nie application (`--template cli|serve|lib|ui|wasm`): `apps/<name>` or `--path`.
    New(nie_scaffold::Args),
    /// Generate the C, C++, Rust, Python, C#, Go, Bun and Zig bindings of a C ABI contract.
    Bindgen(nie_bindgen::Args),
    /// Dependency decisions for upstream sources (GitHub and crates.io read live).
    Deps(deps_cmd::DepsArgs),
    /// Terminal explorer of the game VFS: three columns, fuzzy search, previews of textures, sprite sheets, films, tables, scripts and models.
    #[cfg(feature = "host")]
    Fm(fm_cmd::FmArgs),
    /// GPU terminal window (rio): hosts `nie fm` with native kitty graphics, or any command after `--`; `--screenshot <png>` renders one frame offscreen.
    #[cfg(feature = "term")]
    Term(term_cmd::TermArgs),
    /// List the plugins compiled into this build and the commands each owns.
    Plugins {
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Start the native Model Context Protocol server on stdio.
    #[cfg(feature = "host")]
    Mcp,
    /// Probe non-destructively the native `nie.exe` or Ghidra Computer Use surface.
    #[cfg(feature = "host")]
    #[command(name = "computer-use")]
    ComputerUse {
        surface: String,
        #[arg(long)]
        executable: Option<String>,
        #[arg(long, default_value = "http://127.0.0.1:8080/mcp")]
        ghidra_url: String,
    },
    /// Mod workflow: init, add, get/set, status, validate, install, uninstall.
    #[cfg(feature = "host")]
    Mod {
        #[command(subcommand)]
        op: mod_cmd::ModOp,
    },
    /// IEVR Ultimate Team launcher operations: team encryption, .utmod containers, save parking.
    #[cfg(feature = "host")]
    Launcher {
        #[command(subcommand)]
        op: LauncherOp,
    },
    /// LEVEL-5 archive operations (dump, pack, merge), implemented natively in Rust.
    #[cfg(feature = "host")]
    Viola {
        #[command(subcommand)]
        op: ViolaOp,
    },
    /// Print the format of a file or directory tree without writing anything.
    Format {
        /// File or directory to inspect.
        src: PathBuf,
    },
    /// Decode a file or directory tree to JSON (data) or PNG (textures), with the same dispatch table as the FFI.
    Decode {
        /// Source file or directory.
        src: PathBuf,
        /// Output file, or directory when `src` is one (default: next to the source).
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Print nothing on success.
        #[arg(short, long)]
        quiet: bool,
    },
    /// Regenerate the labeled `*.cfg.bin.json` (`{entries}`/`{lists}`) next to every `*.cfg.bin` of a tree.
    /// This is the shape read by the typed `nie-data` parsers, not the raw structure of `nie decode`; skips up-to-date files unless `--force`.
    #[cfg(feature = "host")]
    RefreshTypedJson {
        /// Directory to walk (recursive).
        dir: PathBuf,
        /// Regenerate even `.json` files newer than their `.cfg.bin`.
        #[arg(long)]
        force: bool,
        /// Print nothing on success.
        #[arg(short, long)]
        quiet: bool,
    },
    /// Fetch the game from Steam, including EAC, EOS and Steamworks.
    ///
    /// The signed third-party components that launch nie.exe come from Steam; this is the only route to a startable install (see `nie info`).
    Steam {
        #[command(subcommand)]
        op: SteamOp,
    },
    /// Describe this game install: binary, VFS, coverage.
    ///
    /// Adds the binary sha256 and the VFS volume to what the reference tool reports.
    Info {
        /// Game root; default: NIE_GAME_SOURCE, NIE_GAME_DIR or NIE_GAME_PATH, then the saved source, then the current directory or the executable's ancestors.
        #[arg(long)]
        game_dir: Option<PathBuf>,
        /// Print JSON instead of `key  value`.
        #[arg(long)]
        json: bool,
    },
    /// List the language tags present and their localized VFS resources.
    ///
    /// Languages come from `common/text/<locale>` only; textures, fonts and Criware files are counted when their path carries the same segment.
    Locales {
        /// Game root; default: NIE_GAME_SOURCE, NIE_GAME_DIR or NIE_GAME_PATH, then the saved source, then the current directory or the executable's ancestors.
        #[arg(long)]
        game_dir: Option<PathBuf>,
        /// VFS paths kept per category and language (0 = no sample).
        #[arg(long, default_value_t = 3)]
        samples: usize,
        /// Print JSON instead of a readable summary.
        #[arg(long)]
        json: bool,
        /// Export a unified multilingual lexicon from the VFS to this file.
        #[arg(long)]
        export: Option<PathBuf>,
        /// Comma-separated languages to export (`all` for every IEVR language).
        #[arg(long, value_delimiter = ',')]
        langs: Vec<String>,
        /// Output directory for the `<lang>.json` files (`0x...` hashes plus seeded UI keys).
        #[arg(long)]
        locales_out: Option<PathBuf>,
        /// Existing directory holding `<lang>.json` files to preserve and merge.
        #[arg(long)]
        seed_locales: Option<PathBuf>,
        /// Comma-separated TEXT_FILES categories to export (default: every useful category).
        #[arg(long, value_delimiter = ',')]
        categories: Vec<String>,
    },
    /// Convert a game asset to an interchange format.
    ///
    /// `decode` emits the canonical form; `convert` picks the output format. The source is a disk file or a VFS path (the VFS is queried when the disk path does not exist).
    Convert {
        /// Disk file or VFS path (`data/dx11/.../x.g4tx`).
        src: String,
        /// Output format: png, webp, gif, jpg, bmp, tga, tiff, qoi; for an icon atlas also css (sheet + image), svg (standalone) or json (region manifest).
        #[arg(long, default_value = "png")]
        to: String,
        /// Output file (default: the source with the format extension).
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Game root; default: NIE_GAME_SOURCE, NIE_GAME_DIR or NIE_GAME_PATH, then the saved source, then the current directory or the executable's ancestors.
        #[arg(long)]
        game_dir: Option<PathBuf>,
        /// Compare the produced bytes with a reference (local file or `https://` URL).
        #[arg(long)]
        reference: Option<String>,
        /// For `--to css`: apply the atlas as a mask tinted by `currentColor` instead of a background image, so icons follow the theme color.
        #[arg(long)]
        masque: bool,
        /// Write every texture of the `.g4tx`, not only the one named like the file; `-o` then names a directory.
        #[arg(long)]
        toutes: bool,
    },
    /// Render a GLB to PNG captures or a turntable GIF for reproducible visual QA.
    #[cfg(feature = "host")]
    Render {
        #[command(subcommand)]
        op: RenderOp,
    },
    /// Static analysis of a Lua source (file or tree), without running it.
    ///
    /// Targets decompiled scripts (`data/lua_scripts/decompiled/`): declared functions, calls, strings and decompiler syntax errors. `.lua.bin` bytecode is run through `nie_lua`, not read here.
    #[cfg(feature = "host")]
    Lua {
        /// `.lua` file or directory to walk recursively.
        src: PathBuf,
        /// List the declared functions.
        #[arg(long)]
        functions: bool,
        /// List call targets, aggregated by descending frequency.
        #[arg(long)]
        calls: bool,
        /// List the retained string literals.
        #[arg(long)]
        strings: bool,
        /// List the strings passed to `CRC32`, with their hash.
        #[arg(long)]
        crc32: bool,
        /// Maximum detail lines per section and file (0 = unlimited).
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Run a raw `.lua.bin` in the Lua 5.2 VM, resolving `INCLUDE` from the VFS.
    #[cfg(feature = "host")]
    #[command(name = "lua-run")]
    LuaRun {
        /// Disk path or logical VFS path (e.g. `data/common/script/lua/menu/main_menu...`).
        script: String,
        /// Game root; default: NIE_GAME_SOURCE, NIE_GAME_DIR or NIE_GAME_PATH, then the saved source, then the current directory or the executable's ancestors.
        #[arg(long)]
        game_dir: Option<PathBuf>,
        /// Instruction limit (0 = unlimited; use only for terminating scripts).
        #[arg(long, default_value_t = 20_000_000)]
        instruction_limit: u32,
        /// Also install the engine menu command stubs.
        #[arg(long)]
        menu_host: bool,
        /// Add the Lua 5.2 disassembly of the chunk to the JSON output.
        #[arg(long)]
        disassemble: bool,
    },
    /// Run every raw Lua chunk of the VFS in batch and measure the engine host gaps.
    #[cfg(feature = "host")]
    #[command(name = "lua-audit")]
    LuaAudit {
        /// Game root; default: NIE_GAME_SOURCE, NIE_GAME_DIR or NIE_GAME_PATH, then the saved source, then the current directory or the executable's ancestors.
        #[arg(long)]
        game_dir: Option<PathBuf>,
        /// Keep only paths starting with this VFS prefix.
        #[arg(long)]
        prefix: Option<String>,
        /// Per-script instruction limit (0 = unlimited).
        #[arg(long, default_value_t = 200_000)]
        instruction_limit: u32,
        /// Also install the menu command host.
        #[arg(long)]
        menu_host: bool,
    },
    /// Import the merged knowledge (Ghidra index nie-index.json) into the knowledge base.
    #[cfg(feature = "host")]
    Seed {
        /// Target sqlite database.
        #[arg(long, default_value = "var/nie.sqlite")]
        db: PathBuf,
        /// Path of research/nie-index.json.
        #[arg(long)]
        json: PathBuf,
        /// nie.exe binary (to compute sha256 and size); optional.
        #[arg(long)]
        exe: Option<PathBuf>,
    },
    /// Ingest into `hash_name` the UI names read from the VFS (screens, layers, groups, commands, objects, menu components; textures with `--textures`).
    ///
    /// Makes the CRC-32 values met in `cfg.bin`, `objbin` and the binary reversible.
    #[cfg(feature = "host")]
    SeedUi {
        /// Target sqlite database.
        #[arg(long, default_value = "var/nie.sqlite")]
        db: PathBuf,
        /// Game root; default: NIE_GAME_SOURCE, NIE_GAME_DIR or NIE_GAME_PATH, then the saved source, then the current directory or the executable's ancestors.
        #[arg(long)]
        game_dir: Option<PathBuf>,
        /// Also scan the menu `.g4tx` files (texture and region names); slow.
        #[arg(long)]
        textures: bool,
    },
    /// Extract the binary strings into `str` and anchor the functions referencing them (`func_str_ref`, `xref` kind=`str`).
    ///
    /// Strings are read from the data sections (ASCII and UTF-16LE), function bounds come from `.pdata`, and a reference is kept only when its target equals the start of a string exactly.
    #[cfg(feature = "host")]
    Strings {
        /// Binary to read; must be the indexed one (resolved by sha256).
        #[arg(long)]
        exe: PathBuf,
        /// Target sqlite database.
        #[arg(long, default_value = "var/nie.sqlite")]
        db: PathBuf,
        /// Minimum length kept, in characters.
        #[arg(long, default_value_t = 4)]
        min_len: usize,
        /// Section to scan, repeatable (default: `.rdata`, `.data`, `.rodata`).
        #[arg(long = "section")]
        sections: Vec<String>,
        /// Force the binary id instead of resolving it by sha256.
        #[arg(long)]
        binary_id: Option<i64>,
        /// Write only `str`: no disassembly, no anchoring.
        #[arg(long)]
        no_xrefs: bool,
        /// Compute everything and write nothing.
        #[arg(long)]
        dry_run: bool,
        /// Print the first `n` strings found, for a visual check.
        #[arg(long, default_value_t = 0)]
        sample: usize,
    },
    /// Find files by path on disk (`ignore` engine, the one of ripgrep and fd).
    ///
    /// Complements `nie vfs find`, which only sees inside the CPKs.
    #[cfg(feature = "host")]
    Find {
        /// Substring searched in the path (empty = list everything).
        #[arg(default_value = "")]
        pattern: String,
        /// Walk root.
        #[arg(long, short = 'C', default_value = ".")]
        dir: PathBuf,
        /// Repeatable glob pattern (`--glob '**/*.rs'`).
        #[arg(long, short = 'g')]
        glob: Vec<String>,
        /// Repeatable extension (`--ext rs --ext toml`).
        #[arg(long, short = 'e')]
        ext: Vec<String>,
        /// `f` = files only, `d` = directories only.
        #[arg(long, short = 't')]
        r#type: Option<String>,
        /// Include hidden files.
        #[arg(long, short = 'H')]
        hidden: bool,
        /// Ignore `.gitignore` rules.
        #[arg(long, short = 'I')]
        no_ignore: bool,
        /// Maximum depth.
        #[arg(long)]
        depth: Option<usize>,
        /// Maximum number of results (0 = unlimited).
        #[arg(long, short = 'n', default_value_t = 0)]
        limit: usize,
        /// Case-sensitive search.
        #[arg(long, short = 's')]
        case_sensitive: bool,
        /// Print only the result count.
        #[arg(long, short = 'c')]
        count: bool,
    },
    /// Search a regular expression in file contents (ripgrep engine).
    #[cfg(feature = "host")]
    Grep {
        /// Regular expression.
        pattern: String,
        /// Walk root.
        #[arg(long, short = 'C', default_value = ".")]
        dir: PathBuf,
        /// Glob pattern restricting visited files, repeatable.
        #[arg(long, short = 'g')]
        glob: Vec<String>,
        /// Extension restricting visited files, repeatable.
        #[arg(long, short = 'e')]
        ext: Vec<String>,
        /// Case-insensitive.
        #[arg(long, short = 'i')]
        ignore_case: bool,
        /// Include hidden files.
        #[arg(long, short = 'H')]
        hidden: bool,
        /// Ignore `.gitignore` rules.
        #[arg(long, short = 'I')]
        no_ignore: bool,
        /// Maximum number of lines printed (0 = unlimited).
        #[arg(long, short = 'n', default_value_t = 0)]
        limit: usize,
        /// Print only the paths of files containing a match.
        #[arg(long, short = 'l')]
        files_with_matches: bool,
    },
    /// Image editing (info, resize, crop, convert, overlay).
    #[cfg(feature = "host")]
    Img {
        #[command(subcommand)]
        op: ImgOp,
    },
    /// Catalogue of game modes: screens, layers, objects, assets and scripts per mode.
    #[cfg(feature = "host")]
    Mode {
        #[command(subcommand)]
        op: ModeOp,
    },
    /// Game icons: index (name to atlas + rectangle) and PNG decoding.
    #[cfg(feature = "host")]
    Icons {
        #[command(subcommand)]
        op: icons_cmd::IconsCmd,
        /// Game root; default: NIE_GAME_SOURCE, NIE_GAME_DIR or NIE_GAME_PATH, then the saved source, then the current directory or the executable's ancestors.
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// Avatar editor (`chara_edit`): catalogue, parts, preset recipes, resolved export.
    #[cfg(feature = "host")]
    Avatar {
        #[command(subcommand)]
        op: avatar_cmd::AvatarCmd,
        /// Game root; default: NIE_GAME_SOURCE, NIE_GAME_DIR or NIE_GAME_PATH, then the saved source, then the current directory or the executable's ancestors.
        #[arg(long)]
        game_dir: Option<PathBuf>,
        /// Knowledge base, to resolve icon names.
        #[arg(long, default_value = "var/nie.sqlite")]
        db: PathBuf,
    },
    /// Generate an original 3D character: morphology, chara_edit, measured colors.
    #[cfg(feature = "host")]
    #[command(name = "oc", visible_alias = "ocgen")]
    Ocgen {
        #[command(subcommand)]
        op: ocgen_cmd::OcgenCmd,
    },
    /// Single index of every RE surface: files, crates, docs, knowledge base, forge, binaries, tools, metrics.
    #[cfg(feature = "host")]
    Atlas {
        #[command(subcommand)]
        op: atlas_cmd::AtlasCmd,
    },
    /// Print the coverage (classified functions) of the indexed binary.
    #[cfg(feature = "host")]
    Coverage {
        #[arg(long, default_value = "var/nie.sqlite")]
        db: PathBuf,
    },
    /// BFS frontier operations on Redis.
    #[cfg(feature = "host")]
    Queue {
        #[command(subcommand)]
        op: QueueOp,
        #[arg(long, env = "NIE_REDIS", default_value = "redis://127.0.0.1/")]
        redis: String,
        #[arg(long, default_value = "nie")]
        tag: String,
    },
    /// Propagate labels over the call graph (auto-ML, string anchoring, label spreading).
    #[cfg(feature = "host")]
    Propagate {
        #[arg(long, default_value = "var/nie.sqlite")]
        db: PathBuf,
        #[arg(long, default_value_t = 16)]
        rounds: usize,
    },
    /// Extract the MSVC RTTI classes from nie.exe and ingest them into the database.
    #[cfg(feature = "host")]
    Rtti {
        /// Target sqlite database.
        #[arg(long, default_value = "var/nie.sqlite")]
        db: PathBuf,
        /// Path to nie.exe.
        #[arg(long)]
        exe: PathBuf,
    },
    /// PE/ELF triage via nie-re-core: sections plus imports/exports ingested into the database.
    #[cfg(feature = "host")]
    Index {
        /// Target sqlite database.
        #[arg(long, default_value = "var/nie.sqlite")]
        db: PathBuf,
        /// Binary to index.
        #[arg(long)]
        exe: PathBuf,
    },
    /// Recover the missing call edges by iced-x86 disassembly of `.text`.
    #[cfg(feature = "host")]
    Disasm {
        /// Target sqlite database.
        #[arg(long, default_value = "var/nie.sqlite")]
        db: PathBuf,
        /// Binary to disassemble (nie.exe).
        #[arg(long)]
        exe: PathBuf,
    },
    /// Discover the authoritative functions from `.pdata` and measure the Ghidra misalignment.
    #[cfg(feature = "host")]
    Pdata {
        /// Target sqlite database.
        #[arg(long, default_value = "var/nie.sqlite")]
        db: PathBuf,
        /// PE x64 binary (nie.exe).
        #[arg(long)]
        exe: PathBuf,
    },
    /// Rebuild the map on `.pdata` (true starts), re-anchor Ghidra, disassemble and propagate.
    #[cfg(feature = "host")]
    Rebuild {
        /// Target sqlite database.
        #[arg(long, default_value = "var/nie.sqlite")]
        db: PathBuf,
        /// PE x64 binary.
        #[arg(long)]
        exe: PathBuf,
        #[arg(long, default_value_t = 16)]
        rounds: usize,
    },
    /// Recover the `.text` leaf functions invisible to `.pdata`.
    #[cfg(feature = "host")]
    Recover {
        /// Target sqlite database.
        #[arg(long, default_value = "var/nie.sqlite")]
        db: PathBuf,
        /// PE x64 binary.
        #[arg(long)]
        exe: PathBuf,
        /// Measure without writing to the database.
        #[arg(long)]
        dry_run: bool,
        /// CSV exported from Ghidra (function names, FID matches, demangled imports), ingested before the other passes.
        #[arg(long)]
        ghidra_csv: Option<PathBuf>,
    },
    /// IEVR save operations (Lives format): decrypt, read, edit.
    Save {
        #[command(subcommand)]
        op: SaveOp,
    },
    /// Build the CRC32-to-path manifest for all .g4md/.g4mg files of the CPKs, used to resolve the ModelIdCrc of inagle_uniforms to real paths.
    #[cfg(feature = "host")]
    UniformMap {
        /// Game root; default: NIE_GAME_SOURCE, NIE_GAME_DIR or NIE_GAME_PATH, then the saved source, then the current directory or the executable's ancestors.
        #[arg(long)]
        game_dir: Option<PathBuf>,
        /// Output NDJSON manifest path (crc32 to path).
        #[arg(long, default_value = "var/model-crc-manifest.ndjson")]
        out: PathBuf,
    },
    /// Scan the .g4tx files of the game CPKs and write an NDJSON manifest of their headers.
    #[cfg(feature = "host")]
    Textures {
        /// Game root; default: NIE_GAME_SOURCE, NIE_GAME_DIR or NIE_GAME_PATH, then the saved source, then the current directory or the executable's ancestors.
        #[arg(long)]
        game_dir: Option<PathBuf>,
        /// Hard limit: maximum number of .g4tx files to process (default 500).
        #[arg(long, default_value_t = 500)]
        limit: usize,
        /// Output NDJSON manifest path.
        #[arg(long, default_value = "var/g4tx-manifest.ndjson")]
        manifest: PathBuf,
        /// Also push to Redis db3 (iev:tex:*).
        #[arg(long)]
        redis: bool,
        /// Redis URL (db3).
        #[arg(long, default_value = "redis://127.0.0.1/3")]
        redis_url: String,
    },
    /// Pre-decode the menu sprites (g4tx to DDS to RGBA to PNG) into the on-disk dump: reads the Redis db3 index (iev:file:index) to locate the g4tx in the CPKs, decodes each with nie-formats and image_dds (BC1/BC3/BC7), and writes `<game_dir>/data/dx11/menu/.../<name>.png` (idempotent, skips non-empty outputs).
    #[cfg(feature = "host")]
    MenuPredecode {
        /// Game root; default: NIE_GAME_SOURCE, NIE_GAME_DIR or NIE_GAME_PATH, then the saved source, then the current directory or the executable's ancestors.
        #[arg(long)]
        game_dir: Option<PathBuf>,
        /// Redis db3 URL (iev:file:index).
        #[arg(long, default_value = "redis://127.0.0.1/3")]
        redis_url: String,
    },
    /// Live RE: read, scan and dump the memory of a running nie.exe.
    ///
    /// The game must be running; attaches to an existing process, never launches or stops it.
    /// The backend depends on the platform: process_vm_readv on Linux/Wine, OpenProcess/ReadProcessMemory on native Windows.
    #[cfg(feature = "host")]
    Mem {
        #[command(subcommand)]
        op: MemOp,
    },
    /// Explorer of the VFS (CPK): list, search, preview and extract files, with structured per-format decoding (G4MT/G4SK/G4MD/G4TX/...) and character/technique search by name, ID or internal code.
    Vfs {
        #[command(subcommand)]
        op: VfsOp,
    },
    /// Checks of the game binary nie.exe against the typed Rust code.
    #[cfg(feature = "host")]
    Exe {
        #[command(subcommand)]
        op: ExeOp,
    },

    /// Feed the `nie-vn-engine` visual novel with the real game assets (voices, textures, music); produces a local catalogue, never versioned.
    #[cfg(feature = "host")]
    Vn {
        #[command(subcommand)]
        op: vn_cmd::VnCmd,
        /// Game root; default: NIE_GAME_SOURCE, NIE_GAME_DIR or NIE_GAME_PATH, then the saved source, then the current directory or the executable's ancestors.
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },

    /// USM/Sofdec2 cutscenes: inventory, metadata, MP4 remux without re-encoding, catalogue.
    #[cfg(feature = "host")]
    Video {
        #[command(subcommand)]
        op: video_cmd::VideoCmd,
        /// Game root; default: NIE_GAME_SOURCE, NIE_GAME_DIR or NIE_GAME_PATH, then the saved source, then the current directory or the executable's ancestors.
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },

    /// The real nie.exe already launched by Steam: describe its process (PID, module base, ptrace).
    ///
    /// `nie` does not launch the game: Steam starts it, `nie live attach` describes it and `nie mem` reads it.
    #[cfg(feature = "host")]
    Live {
        #[command(subcommand)]
        op: LiveOp,
    },
}

/// Subcommands of `nie mode` (game mode catalogue).
#[cfg(feature = "host")]
#[derive(Subcommand)]
enum ModeOp {
    /// (Re)build the catalogue in the database: one mode, its screens and its assets.
    Index {
        #[arg(long, default_value = "var/nie.sqlite")]
        db: PathBuf,
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// Check that every `*_menu_setting` screen is attached to the curated modes.
    Coverage {
        /// Game root; default: NIE_GAME_SOURCE, NIE_GAME_DIR or NIE_GAME_PATH, then the saved source, then the current directory or the executable's ancestors.
        #[arg(long)]
        game_dir: Option<PathBuf>,
        /// Output file; `-` or absent = stdout.
        #[arg(long, short = 'o')]
        out: Option<PathBuf>,
        /// Fail if a screen is covered by several modes or if stems are duplicated.
        #[arg(long)]
        strict: bool,
    },
    /// Export the catalogue as JSON (for the wiki).
    Export {
        #[arg(long, default_value = "var/nie.sqlite")]
        db: PathBuf,
        /// Output file; `-` or absent = stdout.
        #[arg(long, short = 'o')]
        out: Option<PathBuf>,
    },
    /// Export the **content** of a mode's files: layers per screen, parsed menu objects, regions of each texture and localized messages.
    ///
    /// `export` gives the inventory (which files); this gives what they contain.
    Contenu {
        /// Mode id, as named in the VFS (`victory_road`, `chronicle_mode`, ...).
        slug: String,
        /// Output file; `-` or absent = stdout.
        #[arg(long, short = 'o')]
        out: Option<PathBuf>,
        #[arg(long)]
        game_dir: Option<PathBuf>,
        /// Game binary used to resolve message keys: defaults to root/nie.exe,
        /// then root/data/nie.exe. Without it the messages section is empty.
        #[arg(long)]
        exe: Option<PathBuf>,
    },
}

/// Subcommands of `nie render` (visual checks of assembled GLBs).
#[cfg(feature = "host")]
#[derive(Subcommand)]
enum RenderOp {
    /// Render a fixed view of a GLB to PNG (lossless).
    GlbPng {
        /// Local GLB model to check.
        glb: PathBuf,
        /// Output PNG.
        #[arg(long, short = 'o')]
        out: PathBuf,
        /// Output width in pixels.
        #[arg(long, default_value_t = 1024)]
        width: u32,
        /// Output height in pixels.
        #[arg(long, default_value_t = 1024)]
        height: u32,
        /// Azimuth angle in radians (0 = the renderer's reference face).
        #[arg(long, default_value_t = 0.6)]
        angle: f32,
    },
    /// Render a full rotation of a GLB to a GIF to inspect silhouette, UVs and textures.
    GlbGif {
        /// Local GLB model to check.
        glb: PathBuf,
        /// Output GIF.
        #[arg(long, short = 'o')]
        out: PathBuf,
        /// Width of each frame in pixels.
        #[arg(long, default_value_t = 720)]
        width: u32,
        /// Height of each frame in pixels.
        #[arg(long, default_value_t = 720)]
        height: u32,
        /// Frames of the full rotation; eight evenly spaced views are enough to reveal seam defects, more is for fine inspection.
        #[arg(long, default_value_t = 24)]
        frames: u32,
        /// GIF frames per second.
        #[arg(long, default_value_t = 12)]
        fps: u32,
    },
}

/// Subcommands of `nie img` (image editing through the `image` library).
#[cfg(feature = "host")]
#[derive(Subcommand)]
enum ImgOp {
    /// Dimensions, format and color space, without rewriting anything.
    Info { src: PathBuf },
    /// Resize. With a single dimension, the other follows the ratio.
    Resize {
        src: PathBuf,
        #[arg(long, short = 'o')]
        out: PathBuf,
        #[arg(long, short = 'w')]
        width: Option<u32>,
        #[arg(long, short = 'H')]
        height: Option<u32>,
        /// nearest | triangle | catmullrom | gaussian | lanczos3
        #[arg(long, default_value = "lanczos3")]
        filter: String,
        /// Force the exact dimensions (distorts instead of fitting in the box).
        #[arg(long)]
        exact: bool,
    },
    /// Recolor an image: TSV shift and/or luminance ramp, alpha preserved.
    ///
    /// Same filter as `nie mod recolor` (`nie_formats::recolor`), to tune the parameters on a PNG before applying them to a game `.g4tx`.
    Recolor {
        src: PathBuf,
        #[arg(long, short = 'o')]
        out: PathBuf,
        /// Hue shift in degrees (cyclic).
        #[arg(long, default_value_t = 0.0, allow_negative_numbers = true)]
        teinte: f32,
        /// Saturation factor (`1` = unchanged, `0` = gray).
        #[arg(long, default_value_t = 1.0)]
        saturation: f32,
        /// Brightness factor (`1` = unchanged).
        #[arg(long, default_value_t = 1.0)]
        valeur: f32,
        /// Luminance ramp `#RRGGBB[@position]`, comma-separated.
        #[arg(long)]
        rampe: Option<String>,
        /// Ramp blend strength, from `0` (original) to `1` (pure ramp).
        #[arg(long, default_value_t = 1.0)]
        force: f32,
        /// Alpha below which a pixel is left untouched.
        #[arg(long, default_value_t = 1)]
        alpha_min: u8,
    },
    /// Crop a region.
    Crop {
        src: PathBuf,
        #[arg(long, short = 'o')]
        out: PathBuf,
        #[arg(long, default_value_t = 0)]
        x: u32,
        #[arg(long, default_value_t = 0)]
        y: u32,
        #[arg(long, short = 'w')]
        w: u32,
        #[arg(long, short = 'H')]
        h: u32,
    },
    /// Re-encode to the format deduced from the output extension.
    Convert {
        src: PathBuf,
        #[arg(long, short = 'o')]
        out: PathBuf,
    },
    /// Overlay one image on another (alpha respected) to recompose a layered visual.
    Composite {
        base: PathBuf,
        overlay: PathBuf,
        #[arg(long, short = 'o')]
        out: PathBuf,
        #[arg(long, default_value_t = 0)]
        x: i64,
        #[arg(long, default_value_t = 0)]
        y: i64,
    },
    /// Assemble images into a sprite sheet plus its rectangle manifest.
    ///
    /// No image is resized: cells take the size of the largest and smaller ones are centered. The manifest carries the rectangle of the image, not of the cell, which is what a consumer wants to cut out.
    Planche {
        /// Source images, in the order they are laid out.
        srcs: Vec<PathBuf>,
        #[arg(long, short = 'o')]
        out: PathBuf,
        /// Write the JSON manifest to this path. Without it, the sheet is only an image.
        #[arg(long)]
        manifeste: Option<PathBuf>,
        /// Number of columns. `0` = everything on one row.
        #[arg(long, default_value_t = 0)]
        colonnes: u32,
        #[arg(long, default_value_t = 16)]
        marge: u32,
        #[arg(long, default_value_t = 16)]
        gouttiere: u32,
        /// Background color, `RRGGBB` or `RRGGBBAA` (default: transparent).
        #[arg(long, default_value = "00000000")]
        fond: String,
    },
    /// Compare a render with a reference capture: identity, ΔE2000, per-region SSIM, map.
    Diff {
        /// Image produced by the repository.
        rendu: PathBuf,
        /// Capture of the real game.
        reference: PathBuf,
        /// Regions: `[{"nom":…, "rect":[x,y,w,h], "kind":"dynamique"|"nommee"}]`.
        #[arg(long)]
        roi: Option<PathBuf>,
        /// Output directory (report, map, difference). Without it, only the summary is printed.
        #[arg(long, short = 'o')]
        out: Option<PathBuf>,
        /// Scale the reference down to half its dimensions, in linear light.
        #[arg(long)]
        downscale_ref: bool,
        /// Amplification of the difference image.
        #[arg(long, default_value_t = 4)]
        amplification: u8,
    },
}

/// Subcommands of `nie mem` (runtime RE through nie-trace).
#[cfg(feature = "host")]
#[derive(Subcommand)]
enum MemOp {
    /// List the memory ranges (filtered by --module unless --all).
    Maps {
        /// Process id (0 = detect nie.exe).
        #[arg(long, short = 'p', default_value_t = 0)]
        pid: i32,
        /// Module whose ranges are used (default nie.exe).
        #[arg(long, short = 'm', default_value = "nie.exe")]
        module: String,
        /// Every readable range instead of one module.
        #[arg(long)]
        all: bool,
    },
    /// Print the load address (base) of a module.
    Base {
        /// Process id (0 = detect nie.exe).
        #[arg(long, short = 'p', default_value_t = 0)]
        pid: i32,
        /// Module whose ranges are used (default nie.exe).
        #[arg(long, short = 'm', default_value = "nie.exe")]
        module: String,
    },
    /// Read bytes at an address (hex dump, or raw file with -o).
    Read {
        /// Address `0x…` or module-relative `nie.exe+0xF600CA`.
        addr: String,
        #[arg(long, short = 'n', default_value_t = 256)]
        len: usize,
        /// Process id (0 = detect nie.exe).
        #[arg(long, short = 'p', default_value_t = 0)]
        pid: i32,
        /// Write the raw bytes here (otherwise hex dump on stdout).
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },
    /// Write bytes or a value to live memory (YOLO read & write mode).
    Write {
        /// Address `0x…` or module-relative `nie.exe+0xF600CA`.
        addr: String,
        /// Data: hex (`90 90 90`, `0x9090`), string (`str:text`), or integer.
        data: String,
        /// Process id (0 = detect nie.exe).
        #[arg(long, short = 'p', default_value_t = 0)]
        pid: i32,
    },
    /// Dump the readable ranges (module or --all) to a directory.
    Dump {
        /// Process id (0 = detect nie.exe).
        #[arg(long, short = 'p', default_value_t = 0)]
        pid: i32,
        /// Module whose ranges are used (default nie.exe).
        #[arg(long, short = 'm', default_value = "nie.exe")]
        module: String,
        /// Every readable range instead of one module.
        #[arg(long)]
        all: bool,
        #[arg(long, short = 'o', default_value = "./memdump")]
        output: PathBuf,
    },
    /// Search a pattern: hex `48 8B 0D`, text `str:Closing`, or UTF-16LE `wstr:Title`.
    Scan {
        /// Hex pattern (`DE AD BE EF`), `str:…` (UTF-8) or `wstr:…` (UTF-16LE, Windows strings).
        pattern: String,
        /// Process id (0 = detect nie.exe).
        #[arg(long, short = 'p', default_value_t = 0)]
        pid: i32,
        /// Module whose ranges are used (default nie.exe).
        #[arg(long, short = 'm', default_value = "nie.exe")]
        module: String,
        /// Every readable range instead of one module.
        #[arg(long)]
        all: bool,
        #[arg(long, short = 'l', default_value_t = 20)]
        limit: usize,
    },
    /// Read a Lua table field from the live game (`listRowNum`, `pageNum`...).
    ///
    /// The Lua chunk declares the key without ever assigning it: only the process memory carries the value, set by the engine when the screen is instantiated.
    LuaField {
        /// Field name, as it appears in the script (e.g. `listRowNum`).
        name: String,
        /// Process id (0 = detect nie.exe).
        #[arg(long, short = 'p', default_value_t = 0)]
        pid: i32,
        /// Maximum number of interned `TString` objects to consider.
        #[arg(long, default_value_t = 8)]
        strings: usize,
        /// Maximum number of table entries to read per `TString`.
        #[arg(long, default_value_t = 24)]
        nodes: usize,
        /// Also print the neighboring `Node`s, up to this radius (0 = field only).
        #[arg(long, short = 'r', default_value_t = 8)]
        radius: i64,
        /// Print only the entries whose value is a scalar (drops noise).
        #[arg(long)]
        numeric: bool,
    },
    /// Read the `colorPresetID → color` table of the avatar editor palettes.
    ///
    /// These values exist neither in the catalogue nor in the binary: only the live game carries them. The expected ids come from the resolved catalogue (`nie avatar export`).
    Palettes {
        /// Resolved catalogue JSON file, from which the expected ids are read.
        #[arg(long, default_value = "var/avatar-resolved.json")]
        catalogue: PathBuf,
        /// Process id (0 = detect nie.exe).
        #[arg(long, short = 'p', default_value_t = 0)]
        pid: i32,
        /// Start of the range to scan.
        #[arg(long, default_value = "0x10400000")]
        addr: String,
        /// Length of the range to scan.
        #[arg(long, short = 'n', default_value_t = 262_144)]
        len: usize,
        /// Write the result as JSON here instead of standard output.
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
        /// Merge the read colors into the catalogue, under the `couleursRgb` key.
        #[arg(long)]
        fusionner: bool,
    },
    /// Show the catalogue of AOB signatures and offsets derived from the memory dump (nie-trace::catalog).
    Catalog {
        /// Filter by category (player, match, shop, spirit, passive).
        #[arg(long)]
        category: Option<String>,
        /// JSON format instead of a text table.
        #[arg(long)]
        json: bool,
    },
    /// Apply a live modding recipe (.txt) to the running process (nie-trace::recette).
    Recette {
        /// Recipe file (.txt).
        file: PathBuf,
        /// Process id (0 = detect nie.exe).
        #[arg(long, short = 'p', default_value_t = 0)]
        pid: i32,
        /// Module whose ranges are used (default nie.exe).
        #[arg(long, short = 'm', default_value = "nie.exe")]
        module: String,
        /// Really apply the memory writes (otherwise a safe dry run).
        #[arg(long)]
        force: bool,
    },
}

/// Subcommands of `nie exe`.
#[cfg(feature = "host")]
#[derive(Subcommand)]
enum ExeOp {
    /// Which classes and functions of nie.exe have a typed Rust equivalent (reads the `nie index` database).
    Lint(exe_lint::ExeOptions),
}

/// Subcommands of `nie vfs` (CPK / VFS explorer).
///
/// `--game-dir` is optional everywhere and resolved through [`nie_formats::vfs::resolve_game_dir`].
#[derive(Subcommand)]
enum VfsOp {
    /// Locate/register a local or remote source without mounting the VFS.
    Locate {
        #[arg(long)]
        game_dir: Option<String>,
        /// One-time recursive indexing for an installation at an arbitrary location.
        #[arg(long, conflicts_with = "game_dir")]
        scan_root: Option<PathBuf>,
        #[arg(long)]
        json: bool,
        /// Measure repeated shared discovery (process startup and mounting are excluded).
        #[arg(long, default_value = "0")]
        benchmark: usize,
    },
    /// Build the deterministic initial browser VFS from this licensed local installation.
    #[cfg(feature = "host")]
    Bundle {
        #[arg(long, default_value = "main_menu")]
        screen: String,
        #[arg(long, default_value = "fr")]
        locale: String,
        /// Content profile. `afubuki_lean` keeps non-critical game assets out of startup.
        #[arg(long, value_enum, default_value = "complete")]
        profile: vfs_bundle::InitialBundleProfile,
        #[arg(long, short = 'o')]
        out: PathBuf,
        /// JSON provenance and measurements (default: `<out>.manifest.json`).
        #[arg(long)]
        manifest_out: Option<PathBuf>,
        /// Also write physically separate, content-addressed hot/cold archives in a new directory.
        #[arg(long)]
        split_dir: Option<PathBuf>,
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// Folder view: subfolders and files directly under `prefix` (root if omitted).
    Ls {
        prefix: Option<String>,
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// Search by substring in internal paths.
    Find {
        /// Substring searched (case-insensitive).
        query: String,
        /// Filter by extension (e.g. `g4tx`, without the dot).
        #[arg(long)]
        ext: Option<String>,
        #[arg(long, short = 'n', default_value_t = 100)]
        limit: usize,
        /// JSON output (compact one-line array) for programmatic consumption.
        #[arg(long, short = 'j')]
        json: bool,
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// Info on one entry: size, container CPK, detected format (magic).
    Stat {
        path: String,
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// Preview of the content: structured decoding by detected format (hexdump fallback).
    Cat {
        path: String,
        /// Force the hexdump even if a structured decoder recognizes the format.
        #[arg(long)]
        hex: bool,
        /// Number of bytes shown in the hexdump (default 256).
        #[arg(long, default_value_t = 256)]
        len: usize,
        /// Decode the best texture of the file (.g4tx) and write a PNG here.
        #[arg(long)]
        png_out: Option<PathBuf>,
        /// Decode the raw audio (ADX) and write a WAV here.
        #[arg(long)]
        wav_out: Option<PathBuf>,
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// Extract a file, or every file under a prefix, to disk.
    Extract {
        /// Exact path, or VFS folder prefix.
        path: String,
        #[arg(long, short = 'o')]
        out: PathBuf,
        /// Filter by extension, as in `vfs find` (e.g. `cfg.bin`, `lua`, without the dot).
        ///
        /// Without it, extracting a prefix writes everything it contains; the filter avoids dumping every asset just to get the game tables.
        #[arg(long)]
        ext: Option<String>,
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// Histogram of the extensions of the whole VFS.
    Stats {
        #[arg(long, default_value_t = 50)]
        top: usize,
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// Format coverage: the share of the VFS the workspace can actually read.
    ///
    /// Reads the bytes of every file and passes them to the parsers; the extension is not trusted (not every `.cfg.bin` has the same format, `.g4nv` carries the magic `NAVM`) and a recognized magic does not prove the parse succeeds.
    ///
    /// Two levels:
    /// - by default, the **magic** only (`nie_formats::detect`) on the header;
    /// - `--parse`, the **full decoding** (`nie_formats::decode`), which alone tells apart the T2B containers (`objbin`/`cfg.bin`/`mevbin` share the same header).
    Formats {
        /// Try the full decoding, not only the magic recognition.
        #[arg(long)]
        parse: bool,
        /// Examine only the files under this prefix (e.g. `data/common/gamedata`).
        #[arg(long)]
        prefix: Option<String>,
        /// Stop after N files, for a quick sample rather than the full measurement.
        #[arg(long)]
        limit: Option<usize>,
        /// JSON output (one line) for programmatic consumption.
        #[arg(long, short = 'j')]
        json: bool,
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// Every format parser against the VFS, read in place: parse coverage per format.
    #[cfg(feature = "host")]
    Lint(vfs_lint::VfsOptions),
    /// Search characters by localized name, hash or internal code, then list their VFS files (models, textures, animations).
    Chara {
        /// Name, ID or internal code; leave empty with `--element`/`--position` to list the whole category (substituted by `""`, which matches `LIKE '%%'` in SQL).
        #[arg(default_value = "")]
        query: String,
        /// Print only the results, without listing the associated VFS files.
        #[arg(long)]
        no_paths: bool,
        /// Filter by element (e.g. `Feu`, `Vent`, `Forêt`, `Montagne`, `Néant`), case-insensitive.
        #[arg(long)]
        element: Option<String>,
        /// Filter by position (e.g. `GK`, `DF`, `MF`, `FW`), case-insensitive.
        #[arg(long)]
        position: Option<String>,
        /// JSON output (compact one-line array) for programmatic consumption.
        #[arg(long, short = 'j')]
        json: bool,
        #[arg(long, short = 'n', default_value_t = 50)]
        limit: usize,
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// Search native VFS skills by localized name, hash or internal code.
    Waza {
        /// Name, ID or internal code; leave empty with `--category`/`--element` to list the whole category.
        #[arg(default_value = "")]
        query: String,
        #[arg(long)]
        no_paths: bool,
        /// Filter by category, case-insensitive substring (e.g. `Tir` matches `Tir/Shoot`, real bilingual labels: `Dribble`, `Défense/Block`, `Arrêt/Keep`).
        #[arg(long)]
        category: Option<String>,
        /// Filter by element, case-insensitive.
        #[arg(long)]
        element: Option<String>,
        #[arg(long, short = 'j')]
        json: bool,
        #[arg(long, short = 'n', default_value_t = 50)]
        limit: usize,
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// List the configured rclone remotes (R2, S3, Google Drive, SSH, etc.).
    Remotes {
        /// Raw JSON output from `rclone`.
        #[arg(long, short = 'j')]
        json: bool,
    },
    /// Convert and export a `data` folder (or `data/packs`) to R2, S3, Google Drive or rclone.
    Cloud {
        /// `data` folder or game install (auto-detected if omitted).
        #[arg(long)]
        source: Option<PathBuf>,
        /// Destination (`r2://bucket/prefix`, `s3://...`, `gdrive://...`, `rclone://remote/path`).
        #[arg(long)]
        dest: String,
        /// Export mode: `packs`, `decrypted-packs` (recommended for compressed rclone destinations), `seekable-zstd` or `loose`.
        #[arg(long, default_value = "decrypted-packs")]
        mode: String,
        /// Concurrent parallel transfers.
        #[arg(long, default_value_t = 32)]
        transfers: usize,
        /// Concurrent parallel remote checkers.
        #[arg(long, default_value_t = 64)]
        checkers: usize,
        /// Zstd level (`seekable-zstd` mode, 1-22).
        #[arg(long, default_value_t = 4)]
        zstd_level: i32,
        /// Optional limit on packs per cycle (0 = all).
        #[arg(long, default_value_t = 0)]
        max_packs: usize,
        /// Continuous watch (interval in seconds) with incremental sync.
        #[arg(long)]
        watch_secs: Option<u64>,
        /// Plan and compute the manifest without copying bytes.
        #[arg(long)]
        dry_run: bool,
        /// Raw JSON output from `rclone`.
        #[arg(long, short = 'j')]
        json: bool,
    },
}

/// The Viola operations (dump, verify, pack, merge, crypto), served in process by nie_viola.
#[cfg(feature = "host")]
#[derive(Subcommand)]
enum ViolaOp {
    /// Extract the whole VFS to a folder: packs ordered by volume, memory-mapped, resumable after an interrupted dump.
    Dump {
        /// Dossier de sortie.
        #[arg(long, short = 'o')]
        out: PathBuf,
        /// VFS path filter. Comma-separated lists, `**` crosses `/`, `!` prefix excludes (`data/dx11/**,!**/movie/**`).
        #[arg(long)]
        filtre: Option<String>,
        /// Named preset (`wiki`, `assets`, `full`) — exclusive with `--filtre`.
        #[arg(long)]
        preset: Option<String>,
        /// Start from scratch instead of resuming the manifest left by a previous dump.
        #[arg(long)]
        sans_reprise: bool,
        /// Rewrite files even when the destination size already matches.
        #[arg(long)]
        tout_reecrire: bool,
        /// Number of rayon threads (default: one per core).
        #[arg(long)]
        threads: Option<usize>,
        /// Skip packs absent from `cpk_list.cfg.bin` (movies, sound_asset, updates); included by default.
        #[arg(long)]
        sans_extra: bool,
        /// Do not write the failure log `.nie-dump-echecs.json`.
        #[arg(long)]
        sans_journal: bool,
        /// Write the sorted content index `.nie-dump-index.tsv` (path, size, pack).
        #[arg(long)]
        index: bool,
        /// Do not require the extracted size to match the pack summary.
        #[arg(long)]
        sans_verification: bool,
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// Compare a dumped folder with the VFS index: what is missing, truncated or extra.
    Verify {
        /// Dumped folder to verify.
        #[arg(long, short = 'd')]
        dir: PathBuf,
        /// VFS path filter, same syntax as `dump`.
        #[arg(long)]
        filtre: Option<String>,
        /// Compare the content of one file in N with the VFS (0 = no comparison, 1 = all).
        #[arg(long, default_value_t = 500)]
        echantillon: usize,
        /// Also list files present in the folder but absent from the index.
        #[arg(long)]
        intrus: bool,
        /// Number of anomalies printed (the JSON report keeps them all).
        #[arg(long, default_value_t = 20)]
        limite: usize,
        /// Do not write the report `.nie-dump-verif.json`.
        #[arg(long)]
        sans_rapport: bool,
        #[arg(long)]
        threads: Option<usize>,
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// Switch the files of a mod out of the archives: the game then loads them from disk.
    Pack {
        /// Mod folder (tree relative to the VFS).
        #[arg(long)]
        mod_dir: PathBuf,
        /// Output folder; receives the files and the rewritten `cpk_list.cfg.bin`.
        #[arg(long, short = 'o')]
        out: PathBuf,
        /// Vanilla `cpk_list.cfg.bin` (default: the resolved game's).
        #[arg(long)]
        cpk_list: Option<PathBuf>,
        /// Target Switch (`romfs/data/...`) instead of PC (`data/...`).
        #[arg(long)]
        switch: bool,
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// Merge several mods in decreasing priority: the first folder wins.
    ///
    /// By default the merge is per field on `.cfg.bin`: two mods editing different fields of the same file both survive.
    Merge {
        /// Mod folders, from highest to lowest priority.
        #[arg(required = true, num_args = 1..)]
        sources: Vec<PathBuf>,
        /// Dossier de sortie.
        #[arg(long, short = 'o')]
        out: PathBuf,
        /// File-level merge (upstream behavior): no per-field merge.
        #[arg(long)]
        fichier: bool,
        #[arg(long)]
        game_dir: Option<PathBuf>,
    },
    /// (De)crypt a Criware file; the operation is an involution, so one direction is enough.
    Crypto {
        /// Fichier source.
        src: PathBuf,
        /// Fichier destination.
        #[arg(long, short = 'o')]
        out: PathBuf,
        /// Key in hexadecimal (`1717E18E`). Default: the fixed Viola key.
        #[arg(long, conflicts_with = "du_nom")]
        cle: Option<String>,
        /// Derive the key from the file name (CRC32), as for CPK packs.
        #[arg(long)]
        du_nom: bool,
    },
}

/// IEVR Ultimate Team launcher operations: team encryption, .utmod containers, save parking.
#[cfg(feature = "host")]
#[derive(Subcommand)]
pub enum LauncherOp {
    /// IEVR Ultimate Team team cryptography (AES-256-GCM + PBKDF2).
    Team {
        #[command(subcommand)]
        cmd: LauncherTeamOp,
    },
    /// Inspect .utmod / UTMOD2 mod containers.
    Package {
        #[command(subcommand)]
        cmd: LauncherPackageOp,
    },
    /// IEVR PC save operations (parking, restore, injection).
    Save {
        #[command(subcommand)]
        cmd: LauncherSaveOp,
    },
    /// Browse the spirit cards and special techniques (ievrsaveeditor).
    Spirit {
        #[command(subcommand)]
        cmd: LauncherSpiritOp,
    },
    /// IEVR Ultimate Team database, packs, formations and simulator.
    Ut {
        #[command(subcommand)]
        cmd: LauncherUtOp,
    },
}

#[cfg(feature = "host")]
#[derive(Subcommand)]
pub enum LauncherUtOp {
    /// List the IEVR Ultimate Team card packs with their prices and drop rates.
    Packs {
        /// JSON output.
        #[arg(long)]
        json: bool,
    },
    /// Simulate opening a card pack with the official probabilities.
    Open {
        /// Pack identifier (e.g. sobre-bronce, sobre-oro, sobre-icono).
        pack: String,
        /// Deterministic random seed (optional).
        #[arg(long)]
        seed: Option<u64>,
        /// JSON output.
        #[arg(long)]
        json: bool,
    },
    /// Search or list the players of the Ultimate Team catalogue.
    Players {
        /// Optional search term (name, nickname, ID).
        #[arg(long, short = 'q')]
        query: Option<String>,
        /// Filter by element (Aire, Fuego, Bosque, Montaña).
        #[arg(long, short = 'e')]
        element: Option<String>,
        /// Filter by rarity (Común, Raro, Legendario, Ícono, Basara).
        #[arg(long, short = 'r')]
        rarity: Option<String>,
        /// Maximum number of results (default: 20).
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// JSON output.
        #[arg(long)]
        json: bool,
    },
    /// Print the 2D coordinates of the field slots for a formation.
    Formation {
        /// Formation name (e.g. 4-3-3, 4-4-2, 3-5-2 Libertad, 3-4-3).
        name: String,
        /// JSON output.
        #[arg(long)]
        json: bool,
    },
    /// Estimate the market or quick-sell value of a team composition.
    Value {
        /// Path to the team JSON file (plain or encrypted).
        file: std::path::PathBuf,
        /// Decryption passphrase if the file is encrypted.
        #[arg(long, default_value = nie_launcher::DEFAULT_PASSPHRASE)]
        passphrase: String,
        /// JSON output.
        #[arg(long)]
        json: bool,
    },
}

#[cfg(feature = "host")]
#[derive(Subcommand)]
pub enum LauncherSpiritOp {
    /// List or search the hero and fabled spirit cards.
    Cards {
        /// Optional search term (name, variant, hex ID).
        #[arg(long, short = 'q')]
        query: Option<String>,
        /// JSON output instead of a text table.
        #[arg(long)]
        json: bool,
    },
    /// List or search the special techniques of the game.
    Moves {
        /// Optional search term (name or hex ID).
        #[arg(long, short = 'q')]
        query: Option<String>,
        /// Filter by category (Shot, Catch, Dribble, Block).
        #[arg(long, short = 'c')]
        category: Option<String>,
        /// JSON output instead of a text list.
        #[arg(long)]
        json: bool,
    },
}

#[cfg(feature = "host")]
#[derive(Subcommand)]
pub enum LauncherTeamOp {
    /// Decrypt a team JSON file exported from the web platform.
    Decrypt {
        /// Path to the encrypted JSON file.
        file: std::path::PathBuf,
        /// Custom passphrase (otherwise the default canonical key).
        #[arg(long, default_value = nie_launcher::DEFAULT_PASSPHRASE)]
        passphrase: String,
        /// Decrypted JSON output file (otherwise stdout).
        #[arg(long, short = 'o')]
        out: Option<std::path::PathBuf>,
    },
    /// Encrypt a team JSON file for the web platform.
    Encrypt {
        /// Path to the plain JSON file.
        file: std::path::PathBuf,
        /// Custom passphrase (otherwise the default canonical key).
        #[arg(long, default_value = nie_launcher::DEFAULT_PASSPHRASE)]
        passphrase: String,
        /// Encrypted JSON output file (otherwise stdout).
        #[arg(long, short = 'o')]
        out: Option<std::path::PathBuf>,
    },
}

#[cfg(feature = "host")]
#[derive(Subcommand)]
pub enum LauncherPackageOp {
    /// Inspect the metadata and header of a .utmod / UTMOD2 container.
    Info {
        /// Path to the .utmod file.
        file: std::path::PathBuf,
    },
}

#[cfg(feature = "host")]
#[derive(Subcommand)]
pub enum LauncherSaveOp {
    /// Atomically park the Steam save to .bk and install the mod save.
    Park {
        #[arg(long)]
        live_save: std::path::PathBuf,
        #[arg(long)]
        mod_save: std::path::PathBuf,
    },
    /// Restore the original Steam save from its .bk file.
    Restore {
        #[arg(long)]
        live_save: std::path::PathBuf,
        #[arg(long)]
        mod_save: std::path::PathBuf,
    },
    /// Inject a decrypted team into team 2 of the save.
    InjectTeam {
        #[arg(long)]
        save: std::path::PathBuf,
        #[arg(long)]
        team: std::path::PathBuf,
        #[arg(long)]
        out: Option<std::path::PathBuf>,
    },
}

#[derive(Subcommand)]
enum SaveOp {
    /// Decrypt and print a terse summary of a Lives save file.
    Read {
        /// Path to the save file (e.g. `002AB8F4-SYSTEMLIVE`).
        #[arg(long)]
        file: std::path::PathBuf,
        /// Also print the first N bytes of each blob body.
        #[arg(long, default_value_t = 0)]
        hexdump: usize,
    },
    /// Decrypt a save file and write the plaintext.
    Decrypt {
        /// Path to the encrypted save file.
        #[arg(long)]
        file: std::path::PathBuf,
        /// Output path for the plaintext.
        #[arg(long)]
        out: std::path::PathBuf,
    },
    /// Encrypt a plaintext file (from `decrypt`) and produce the save file.
    Encrypt {
        /// Path to the plaintext.
        #[arg(long)]
        file: std::path::PathBuf,
        /// Slot name (e.g. `002AB8F4-SYSTEMLIVE`) used to derive the key.
        #[arg(long)]
        slot: String,
        /// Output path for the encrypted file.
        #[arg(long)]
        out: std::path::PathBuf,
    },
    /// Edit one byte in a blob body and re-encrypt.
    Edit {
        /// Path to the save file.
        #[arg(long)]
        file: std::path::PathBuf,
        /// Internal blob name (e.g. `SYSTEM_data.bin`).
        #[arg(long)]
        blob: String,
        /// Offset in the blob body (decimal or `0x...`).
        #[arg(long, value_parser = parse_addr)]
        offset: i64,
        /// New byte value (decimal or `0x...`).
        #[arg(long, value_parser = parse_addr)]
        value: i64,
        /// Output file (default: overwrite the input).
        #[arg(long)]
        out: Option<std::path::PathBuf>,
    },
    /// Atomically park the Steam save to .bk and install the mod save (nie-launcher).
    #[cfg(feature = "host")]
    Park {
        #[arg(long)]
        live_save: std::path::PathBuf,
        #[arg(long)]
        mod_save: std::path::PathBuf,
    },
    /// Restore the original Steam save from its .bk file (nie-launcher).
    #[cfg(feature = "host")]
    Restore {
        #[arg(long)]
        live_save: std::path::PathBuf,
        #[arg(long)]
        mod_save: std::path::PathBuf,
    },
    /// Inject a decrypted team into team 2 of an IEVR PC save (nie-launcher).
    #[cfg(feature = "host")]
    InjectTeam {
        #[arg(long)]
        save: std::path::PathBuf,
        #[arg(long)]
        team: std::path::PathBuf,
        #[arg(long)]
        out: Option<std::path::PathBuf>,
    },
}

#[cfg(feature = "host")]
#[derive(Subcommand)]
enum QueueOp {
    /// Push an address (decimal or hex `0x...`).
    Push {
        #[arg(value_parser = parse_addr)]
        addr: i64,
    },
    /// Pop the next address.
    Pop,
    /// Frontier size.
    Len,
    /// Clear the frontier.
    Reset,
}

/// Steam operations: depot inspection and download through nie-steam.
#[derive(Subcommand, Debug)]
enum SteamOp {
    /// Inspect the depots (sizes, files) without downloading anything.
    List {
        /// Steam App ID (default: IEVR).
        app_id: Option<u32>,
        #[command(flatten)]
        commun: SteamCommun,
    },
    /// Download a Steam app to the target directory.
    Download {
        /// Steam App ID (default: IEVR).
        app_id: Option<u32>,
        /// Destination directory.
        #[arg(short = 'o', long, default_value = ".")]
        out: PathBuf,
        #[command(flatten)]
        commun: SteamCommun,
    },
    /// Download IEVR; shortcut for `download 2799860`.
    Sync {
        /// Destination directory.
        #[arg(short = 'o', long, default_value = ".")]
        out: PathBuf,
        #[command(flatten)]
        commun: SteamCommun,
    },
    /// Index a local Steam install into SQLite: libraries, apps, depots, compat tools, blake3 of game files.
    Index(SteamIndexArgs),
}

/// Options of `nie steam index`. Read-only: nothing in the Steam install is modified, and credential
/// artifacts (`ssfn*`, `loginusers.vdf`, `local.vdf`, `config.vdf`) are recorded by path, size and
/// blake3 only.
#[derive(clap::Args, Debug)]
struct SteamIndexArgs {
    /// Steam root to index (default: the discovered Steam install).
    #[arg(long)]
    steam: Option<PathBuf>,
    /// SQLite database (default: `<user data dir>/nie/steam-index.sqlite`).
    #[arg(long)]
    db: Option<PathBuf>,
    /// Print the scan as JSON and exit without touching the database.
    #[arg(long)]
    json: bool,
    /// Hash every game file with blake3 (resumable: unchanged files keep their hash).
    #[arg(long)]
    hash: bool,
    /// Set STEAM_ACCOUNT_NAME and STEAM_ID (most recent `loginusers.vdf` account) in this dotenv file.
    #[arg(long, value_name = "FILE")]
    env_file: Option<PathBuf>,
}

/// Options shared by the three Steam operations.
///
/// The environment variable names are fixed: renaming them breaks existing callers.
#[derive(clap::Args, Debug, Clone)]
struct SteamCommun {
    /// Steam account (empty = anonymous login).
    #[arg(short = 'u', long, env = "NIE_STEAM_USER")]
    username: Option<String>,
    /// Password (ignored if a refresh token is cached).
    #[arg(short = 'p', long, env = "NIE_STEAM_PASSWORD")]
    password: Option<String>,
    /// Steam branch.
    #[arg(short = 'b', long, default_value = "public")]
    branch: String,
    /// Target OS for depot filtering.
    #[arg(long, default_value = "windows")]
    os: String,
    /// Target architecture.
    #[arg(long)]
    arch: Option<String>,
    /// Target language.
    #[arg(long, default_value = "english")]
    language: String,
    /// Do not filter by OS, arch or language.
    #[arg(long)]
    all_platforms: bool,
    /// Explicit depots (repeatable).
    #[arg(long = "depot", value_name = "ID")]
    depot_ids: Vec<u32>,
    /// Concurrent chunks.
    #[arg(long, env = "NIE_STEAM_MAX_DOWNLOADS", default_value_t = 64)]
    max_downloads: usize,
    /// Depots downloaded in parallel.
    #[arg(long, env = "NIE_STEAM_MAX_DEPOT_PARALLEL", default_value_t = 6)]
    max_depot_parallel: usize,
    /// HTTP/2 connection pool size per CDN host.
    #[arg(long, env = "NIE_STEAM_CDN_POOL_SIZE", default_value_t = 128)]
    cdn_pool_size: usize,
    /// Disable the multi-file HTTP2 turbo pipeline and fall back to the sequential `steam-depot` client.
    #[arg(long)]
    no_turbo: bool,
    /// Destination rclone remote (e.g. `remote:`) to compress and upload packs continuously.
    #[arg(long, env = "NIE_VFS_CLOUD_REMOTE")]
    sync_remote: Option<String>,
    /// Continuous cloud export mode (`decrypted-packs`, `seekable-zstd`, `packs`, `loose`).
    #[arg(long, default_value = "decrypted-packs")]
    sync_mode: String,
    /// Concurrent parallel rclone transfers during `--sync-remote`.
    #[arg(long, default_value_t = 32)]
    sync_transfers: usize,
    /// Concurrent parallel rclone checkers during `--sync-remote`.
    #[arg(long, default_value_t = 64)]
    sync_checkers: usize,
    /// Re-download everything instead of skipping up-to-date files.
    #[arg(long)]
    no_verify: bool,
    /// Token store path.
    #[arg(long, env = "NIE_STEAM_TOKEN_STORE")]
    token_store: Option<PathBuf>,
}

/// `nie steam index`: scan, store, inventory and optionally hash a local Steam install.
fn steam_index(args: &SteamIndexArgs) -> anyhow::Result<()> {
    use nie_steam::index;

    let root = args
        .steam
        .clone()
        .or_else(nie_steam::find_steam_dir)
        .context("no Steam install found; pass --steam <root>")?;
    let scan = index::scan(&root);
    if args.json {
        println!("{}", serde_json::to_string(&scan)?);
        return Ok(());
    }
    let mut db = index::open(&args.db.clone().unwrap_or_else(index::default_db_path))?;
    index::store(&mut db, &scan)?;
    println!(
        "scan: {} libraries, {} apps, {} pics apps, {} packages",
        scan.libraries.len(),
        scan.apps.len(),
        scan.appinfo.as_ref().map_or(0, |i| i.apps.len()),
        scan.packageinfo.as_ref().map_or(0, |i| i.packages.len()),
    );
    if let Some(env_file) = &args.env_file {
        match index::account(&root) {
            Some(account) => {
                upsert_env_key(env_file, "STEAM_ACCOUNT_NAME", &account.account_name)?;
                upsert_env_key(env_file, "STEAM_ID", &account.steam_id)?;
                println!("env: set STEAM_ACCOUNT_NAME, STEAM_ID");
            }
            None => println!("env: no account found in loginusers.vdf"),
        }
    }
    println!(
        "inventory: {} files (paths + sizes)",
        index::inventory(&mut db, &scan.apps)?
    );
    if args.hash {
        let report =
            index::hash_pending(&mut db, |done, pending| println!("hash: {done}/{pending}"))?;
        println!(
            "hash: {} pending, {} hashed, {} errors",
            report.pending, report.hashed, report.errors
        );
    }
    Ok(())
}

/// Sets `KEY=value` in a dotenv file: replaces the line, else appends it. The value is never printed.
fn upsert_env_key(path: &std::path::Path, key: &str, value: &str) -> anyhow::Result<()> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let line = format!("{key}={value}");
    let prefix = format!("{key}=");
    let mut replaced = false;
    let mut lines: Vec<&str> = Vec::new();
    for existing in text.lines() {
        if !replaced && existing.starts_with(&prefix) {
            lines.push(&line);
            replaced = true;
        } else {
            lines.push(existing);
        }
    }
    if !replaced {
        lines.push(&line);
    }
    std::fs::write(path, lines.join("\n") + "\n")
        .with_context(|| format!("write {}", path.display()))
}

/// Run a Steam operation.
///
/// `nie_steam::depot` is async while the CLI is not: a runtime is built for the call instead of making all of `main` async for three subcommands.
fn steam_cmd(op: SteamOp) -> anyhow::Result<()> {
    use nie_steam::depot::options::SteamDownloadOptions;
    use nie_steam::{IEVR_STEAM_APP_ID, depot::downloader::SteamDepotDownloader};

    let (app_id, out, c) = match &op {
        SteamOp::List { app_id, commun } => (
            app_id.unwrap_or(IEVR_STEAM_APP_ID),
            PathBuf::from("."),
            commun,
        ),
        SteamOp::Download {
            app_id,
            out,
            commun,
        } => (app_id.unwrap_or(IEVR_STEAM_APP_ID), out.clone(), commun),
        SteamOp::Sync { out, commun } => (IEVR_STEAM_APP_ID, out.clone(), commun),
        SteamOp::Index(args) => return steam_index(args),
    };

    let opts = SteamDownloadOptions {
        app_id,
        install_dir: out,
        username: c.username.clone(),
        password: c.password.clone(),
        branch: c.branch.clone(),
        beta_password: None,
        os: c.os.clone(),
        arch: c.arch.clone(),
        language: c.language.clone(),
        all_platforms: c.all_platforms,
        depot_ids: c.depot_ids.clone(),
        max_downloads: c.max_downloads,
        verify: !c.no_verify,
        token_store_path: Some(
            c.token_store
                .clone()
                .unwrap_or_else(nie_steam::depot::token_store::default_path),
        ),
        // `NIE_STEAM_TOKEN_STORE` selects the token store (default: file).
        token_backend: Some(nie_steam::depot::token_backend::from_env(
            c.token_store.clone(),
        )?),
        guard_provider: None,
        // Stall guard: a silent depot is interrupted instead of sleeping
        // forever (see nie_steam::depot::options::SteamDownloadOptions::stall_timeout).
        stall_timeout: Some(std::time::Duration::from_secs(300)),
        transfer: nie_steam::depot::options::TransferOptions {
            max_depot_parallel: c.max_depot_parallel,
            cdn_pool_size: c.cdn_pool_size,
            ..Default::default()
        },
    };

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let dl = SteamDepotDownloader::new();

    rt.block_on(async {
        if matches!(op, SteamOp::List { .. }) {
            let infos = dl.list_depots(&opts).await?;
            if infos.is_empty() {
                println!("app_id={} depots=0", opts.app_id);
            }
            for info in &infos {
                #[allow(clippy::cast_precision_loss)]
                let mb = info.total_bytes as f64 / (1024.0 * 1024.0);
                println!(
                    "depot={} name={} manifest={} files={} size={mb:.1}MB",
                    info.depot_id,
                    info.name.as_deref().unwrap_or("(no name)"),
                    info.manifest_id,
                    info.file_count
                );
            }
            return Ok(());
        }

        if !c.no_turbo {
            let cfg = steam_turbo::TurboConfig {
                cdn_pool_size: c.cdn_pool_size,
                sync_remote: c.sync_remote.clone(),
                sync_mode: c.sync_mode.clone(),
                sync_transfers: c.sync_transfers,
                sync_checkers: c.sync_checkers,
            };
            let r = steam_turbo::download_app_turbo(&opts, &cfg).await?;
            println!(
                "ok app_id={} depots={} files={} skipped={} bytes={} sync_batches={} duration={:.1}s",
                r.app_id,
                r.depots.len(),
                r.file_count,
                r.skipped_files,
                r.downloaded_bytes,
                r.synced_batches,
                r.duration.as_secs_f64()
            );
            return Ok(());
        }

        let r = dl.download_app(&opts, None).await;
        if r.success {
            println!(
                "ok app_id={} depots={} files={} bytes={} duration={:.1}s",
                r.app_id,
                r.depots.len(),
                r.file_count,
                r.downloaded_bytes,
                r.duration.as_secs_f64()
            );
            Ok(())
        } else {
            anyhow::bail!("{}", r.error.as_deref().unwrap_or("unknown error"))
        }
    })
}

/// Subcommands of `nie live`: the already-running `nie.exe`.
#[cfg(feature = "host")]
#[derive(Subcommand)]
enum LiveOp {
    /// Describe the running `nie.exe`: PID, module base, ptrace permission.
    Attach {
        #[arg(long, short = 'j')]
        json: bool,
    },
}

/// Run a `nie live` operation.
///
/// `nie` does not launch the game (no Proton, Wine or virtual desktop): it reads a `nie.exe` already started by the official launch (Steam, then EAC). Reading goes through `ptrace`, Linux only; on a Windows host, `nie mem` reads the native process.
#[cfg(all(feature = "host", target_os = "linux"))]
fn live_cmd(op: LiveOp) -> anyhow::Result<()> {
    match op {
        LiveOp::Attach { json } => {
            let pid =
                nie_trace::find_pid_by_name(nie_trace::GAME_COMM).ok_or_else(|| {
                    anyhow::anyhow!(
                        "no `{}` process; launch the game through Steam",
                        nie_trace::GAME_COMM
                    )
                })?;
            let base = nie_trace::find_module_base(pid, nie_trace::GAME_COMM);
            let permis = nie_trace::likely_permitted(pid);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "pid": pid,
                        "moduleBase": base.map(|b| format!("{b:#x}")),
                        "ptraceScope": nie_trace::read_ptrace_scope(),
                        "readLikelyPermitted": permis,
                    }))?
                );
            } else {
                println!("pid           {pid}");
                match base {
                    Some(b) => println!("module_base   {b:#x}"),
                    None => println!("module_base   (not found)"),
                }
                println!("ptrace_scope  {}", nie_trace::read_ptrace_scope());
                println!(
                    "read          {}",
                    if permis { "allowed" } else { "DENIED" }
                );
            }
            Ok(())
        }
    }
}

#[cfg(all(feature = "host", not(target_os = "linux")))]
fn live_cmd(_op: LiveOp) -> anyhow::Result<()> {
    anyhow::bail!(
        "`nie live` reads a process through ptrace, which exists only on Linux. On a Windows host, \
         read the game with `nie mem`."
    )
}

/// Game root: the one passed as argument, otherwise the one the context designates.
///
/// Resolved by `resolve_game_dir`: the environment variables, then the current directory and its ancestors holding `data/cpk_list.cfg.bin`, then the executable directory.
fn racine_jeu(arg: Option<PathBuf>) -> PathBuf {
    arg.unwrap_or_else(nie_formats::vfs::resolve_game_dir)
}

/// Locate native game binaries without changing the logical VFS root.
/// Steam installations keep binaries at the root; this workspace keeps them in data/.
fn game_binary_dir(root: &std::path::Path) -> PathBuf {
    let nested = root.join("data");
    if !root.join("nie.exe").is_file() && nested.join("nie.exe").is_file() {
        nested
    } else {
        root.to_path_buf()
    }
}

/// Stack of the thread that runs the CLI.
///
/// The debug profile inlines nothing: the clap frames and the VFS mount exceed the default 1 MiB Windows stack, so every command overflowed. Release is not affected.
const PILE_CLI: usize = 64 * 1024 * 1024;

/// Run the `nie` command-line entry point.
pub fn main_entry() -> anyhow::Result<()> {
    std::thread::Builder::new()
        .stack_size(PILE_CLI)
        .spawn(run)?
        .join()
        .map_err(|_| anyhow::anyhow!("the command panicked"))?
}

/// Run the CLI with an explicit argument vector.
///
/// This is used by thin native bindings such as the standalone `nie-mcp`
/// executable while keeping command parsing and dispatch in one library.
pub fn main_entry_with<I, T>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString>,
{
    nie_tls::install();
    nie_tls::install();
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    std::thread::Builder::new()
        .stack_size(PILE_CLI)
        .spawn(move || {
            let cli = Cli::try_parse_from(args)?;
            dispatch(cli)
        })?
        .join()
        .map_err(|_| anyhow::anyhow!("the command panicked"))?
}

fn run() -> anyhow::Result<()> {
    nie_tls::install();
    nie_tls::install();
    // Internal CLI (consumed by the agent): minimal output. `RUST_LOG=info` re-enables traces.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .try_init()
        .ok();
    let cli = Cli::parse();
    dispatch(cli)
}

fn dispatch(cli: Cli) -> anyhow::Result<()> {
    match cli.cmd {
        Cmd::Serve(args) => serve_cmd::run(args),
        Cmd::Plugins { json } => plugins::run(json),
        Cmd::Wiki(args) => wiki_cmd::run(args),
        Cmd::New(args) => nie_scaffold::run(&args),
        Cmd::Bindgen(args) => nie_bindgen::run(&args),
        Cmd::Deps(args) => deps_cmd::run(args),
        #[cfg(feature = "host")]
        Cmd::Fm(args) => fm_cmd::run(args),
        #[cfg(feature = "term")]
        Cmd::Term(args) => term_cmd::run(args),
        #[cfg(feature = "host")]
        Cmd::Mcp => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            runtime.block_on(mcp::serve_stdio())
        }
        #[cfg(feature = "host")]
        Cmd::ComputerUse {
            surface,
            executable,
            ghidra_url,
        } => {
            println!(
                "{}",
                nie_computer_use::probe_cli(&surface, executable, ghidra_url)?
            );
            Ok(())
        }
        #[cfg(feature = "host")]
        Cmd::Mod { op } => mod_cmd::executer(op),
        #[cfg(feature = "host")]
        Cmd::Launcher { op } => launcher_cmd(op),
        #[cfg(feature = "host")]
        Cmd::Viola { op } => viola_cmd(op),
        Cmd::Format { src } => decode_cmd::format(&src),
        Cmd::Decode { src, out, quiet } => {
            if src.is_dir() {
                let out = out.unwrap_or_else(|| src.join("_decoded"));
                decode_cmd::dir(&src, &out, quiet)
            } else {
                decode_cmd::file(&src, out.as_deref(), quiet)
            }
        }
        #[cfg(feature = "host")]
        Cmd::RefreshTypedJson { dir, force, quiet } => {
            decode_cmd::refresh_typed(&dir, force, quiet)
        }
        Cmd::Steam { op } => steam_cmd(op),
        #[cfg(feature = "host")]
        Cmd::Live { op } => live_cmd(op),
        Cmd::Info { game_dir, json } => info_cmd(game_dir, json),
        Cmd::Locales {
            game_dir,
            samples,
            json,
            export,
            langs,
            locales_out,
            seed_locales,
            categories,
        } => locales_cmd(
            game_dir,
            samples,
            json,
            export.as_deref(),
            &langs,
            locales_out.as_deref(),
            seed_locales.as_deref(),
            &categories,
        ),
        Cmd::Convert {
            src,
            to,
            out,
            game_dir,
            reference,
            masque,
            toutes,
        } => {
            if toutes {
                convert_toutes(&src, &to, out.as_deref(), game_dir)
            } else {
                convert_cmd(
                    &src,
                    &to,
                    out.as_deref(),
                    game_dir,
                    reference.as_deref(),
                    masque,
                )
            }
        }
        #[cfg(feature = "host")]
        Cmd::Render { op } => match op {
            RenderOp::GlbPng {
                glb,
                out,
                width,
                height,
                angle,
            } => render_cmd::glb_png(
                &glb,
                &out,
                render_cmd::RenderConfig { width, height },
                angle,
            ),
            RenderOp::GlbGif {
                glb,
                out,
                width,
                height,
                frames,
                fps,
            } => render_cmd::glb_gif(
                &glb,
                &out,
                render_cmd::RenderConfig { width, height },
                frames,
                fps,
            ),
        },
        #[cfg(feature = "host")]
        Cmd::Lua {
            src,
            functions,
            calls,
            strings,
            crc32,
            limit,
        } => lua_cmd::run(
            &src,
            lua_cmd::Detail {
                functions,
                calls,
                strings,
                crc32,
                limit,
            },
        ),
        #[cfg(feature = "host")]
        Cmd::LuaRun {
            script,
            game_dir,
            instruction_limit,
            menu_host,
            disassemble,
        } => lua_run_cmd::run(
            &script,
            game_dir.as_deref(),
            instruction_limit,
            menu_host,
            disassemble,
        ),
        #[cfg(feature = "host")]
        Cmd::LuaAudit {
            game_dir,
            prefix,
            instruction_limit,
            menu_host,
        } => lua_audit_cmd::run(
            game_dir.as_deref(),
            prefix.as_deref(),
            instruction_limit,
            menu_host,
        ),
        #[cfg(feature = "host")]
        Cmd::Seed { db, json, exe } => seed(&db, &json, exe.as_deref()),
        #[cfg(feature = "host")]
        Cmd::SeedUi {
            db,
            game_dir,
            textures,
        } => seed_ui_cmd(&db, game_dir, textures),
        #[cfg(feature = "host")]
        Cmd::Strings {
            exe,
            db,
            min_len,
            sections,
            binary_id,
            no_xrefs,
            dry_run,
            sample,
        } => strings_cmd_run(
            &db,
            &exe,
            &strings_cmd::Options {
                min_len,
                sections,
                binary_id,
                no_xrefs,
                dry_run,
                sample,
            },
        ),
        #[cfg(feature = "host")]
        Cmd::Find {
            pattern,
            dir,
            glob,
            ext,
            r#type,
            hidden,
            no_ignore,
            depth,
            limit,
            case_sensitive,
            count,
        } => search_cmd::find(&search_cmd::FindArgs {
            pattern,
            dir,
            globs: glob,
            exts: ext,
            kind: r#type,
            hidden,
            no_ignore,
            depth,
            limit,
            case_sensitive,
            count,
        })
        .map(|_| ()),
        #[cfg(feature = "host")]
        Cmd::Grep {
            pattern,
            dir,
            glob,
            ext,
            ignore_case,
            hidden,
            no_ignore,
            limit,
            files_with_matches,
        } => search_cmd::grep(&search_cmd::GrepArgs {
            pattern,
            dir,
            globs: glob,
            exts: ext,
            ignore_case,
            hidden,
            no_ignore,
            limit,
            files_with_matches,
        })
        .map(|_| ()),
        #[cfg(feature = "host")]
        Cmd::Icons { op, game_dir } => {
            let racine = game_dir.unwrap_or_else(nie_formats::vfs::resolve_game_dir);
            icons_cmd::run(&op, &racine)
        }
        #[cfg(feature = "host")]
        Cmd::Avatar { op, game_dir, db } => {
            let racine = game_dir.unwrap_or_else(nie_formats::vfs::resolve_game_dir);
            avatar_cmd::run(&op, &racine, &db)
        }
        #[cfg(feature = "host")]
        Cmd::Ocgen { op } => ocgen_cmd::run(&op),
        #[cfg(feature = "host")]
        Cmd::Mode { op } => match op {
            ModeOp::Index { db, game_dir } => {
                let vfs = open_vfs(game_dir)?;
                let database = nie_index::Db::open(&db)
                    .with_context(|| format!("opening {}", db.display()))?;
                let (m, s, a, t) = mode_index::index(&database, &vfs)?;
                println!("mode index: {m} modes, {s} screens, {a} assets, {t} texts");
                Ok(())
            }
            ModeOp::Coverage {
                game_dir,
                out,
                strict,
            } => {
                let vfs = open_vfs(game_dir)?;
                let json = mode_index::menu_coverage_json(&vfs);
                let settings = json
                    .get("settings")
                    .and_then(serde_json::Value::as_object)
                    .context("coverage output without settings")?;
                let unique = settings
                    .get("unique")
                    .and_then(serde_json::Value::as_u64)
                    .context("coverage.settings.unique missing")?;
                let classified = settings
                    .get("classified")
                    .and_then(serde_json::Value::as_u64)
                    .context("coverage.settings.classified missing")?;
                let unclassified = settings
                    .get("unclassified")
                    .and_then(serde_json::Value::as_u64)
                    .context("coverage.settings.unclassified missing")?;
                let overlapping = settings
                    .get("overlapping")
                    .and_then(serde_json::Value::as_u64)
                    .context("coverage.settings.overlapping missing")?;
                let duplicates = settings
                    .get("duplicateStems")
                    .and_then(serde_json::Value::as_u64)
                    .context("coverage.settings.duplicateStems missing")?;
                let txt = serde_json::to_string_pretty(&json)?;
                match out {
                    Some(path) if path.as_os_str() != "-" => {
                        if let Some(parent) = path.parent()
                            && !parent.as_os_str().is_empty()
                        {
                            std::fs::create_dir_all(parent)?;
                        }
                        std::fs::write(&path, txt.as_bytes())
                            .with_context(|| format!("writing {}", path.display()))?;
                        println!(
                            "mode coverage -> {} ({} screens, {} classified, {} unclassified)",
                            path.display(),
                            unique,
                            classified,
                            unclassified
                        );
                    }
                    _ => println!("{txt}"),
                }
                if strict && (overlapping != 0 || duplicates != 0) {
                    anyhow::bail!(
                        "inconsistent coverage: {overlapping} overlaps, {duplicates} duplicated stems"
                    );
                }
                Ok(())
            }
            ModeOp::Export { db, out } => {
                let database = nie_index::Db::open(&db)
                    .with_context(|| format!("opening {}", db.display()))?;
                let json = mode_index::export_json(&database)?;
                let txt = serde_json::to_string_pretty(&json)?;
                match out {
                    Some(p) if p.as_os_str() != "-" => {
                        if let Some(parent) = p.parent()
                            && !parent.as_os_str().is_empty()
                        {
                            std::fs::create_dir_all(parent)?;
                        }
                        std::fs::write(&p, txt.as_bytes())
                            .with_context(|| format!("writing {}", p.display()))?;
                        println!("mode export -> {} ({} bytes)", p.display(), txt.len());
                    }
                    _ => println!("{txt}"),
                }
                Ok(())
            }
            ModeOp::Contenu {
                slug,
                out,
                game_dir,
                exe,
            } => {
                let vfs = open_vfs(game_dir.clone())?;
                let def = mode_index::MODES
                    .iter()
                    .find(|d| d.slug == slug)
                    .ok_or_else(|| {
                        let connus: Vec<&str> = mode_index::MODES.iter().map(|d| d.slug).collect();
                        anyhow::anyhow!("unknown mode `{slug}`; modes: {}", connus.join(", "))
                    })?;
                // The binary lives at the game root, not under `data/`: without `--exe`, look
                // where `resolve_game_dir` found the VFS.
                let exe = exe.or_else(|| {
                    let racine = game_dir
                        .clone()
                        .unwrap_or_else(nie_formats::vfs::resolve_game_dir);
                    let p = game_binary_dir(&racine).join("nie.exe");
                    p.exists().then_some(p)
                });
                let json = mode_index::contenu_json(&vfs, def, exe.as_deref())?;
                let txt = serde_json::to_string_pretty(&json)?;
                match out {
                    Some(p) if p.as_os_str() != "-" => {
                        if let Some(parent) = p.parent()
                            && !parent.as_os_str().is_empty()
                        {
                            std::fs::create_dir_all(parent)?;
                        }
                        std::fs::write(&p, txt.as_bytes())
                            .with_context(|| format!("writing {}", p.display()))?;
                        println!(
                            "mode content {slug} -> {} ({} bytes)",
                            p.display(),
                            txt.len()
                        );
                    }
                    _ => println!("{txt}"),
                }
                Ok(())
            }
        },
        #[cfg(feature = "host")]
        Cmd::Img { op } => img_cmd::run(&match op {
            ImgOp::Info { src } => img_cmd::Op::Info { src },
            ImgOp::Recolor {
                src,
                out,
                teinte,
                saturation,
                valeur,
                rampe,
                force,
                alpha_min,
            } => img_cmd::Op::Recolor {
                src,
                out,
                filtre: nie_formats::recolor::Recolor {
                    hue_shift: teinte,
                    saturation,
                    value: valeur,
                    ramp: match rampe {
                        Some(spec) => {
                            let mut r = nie_formats::recolor::Ramp::parse(&spec)
                                .map_err(|e| anyhow::anyhow!("--rampe: {e}"))?;
                            r.strength = force;
                            Some(r)
                        }
                        None => None,
                    },
                    alpha_min,
                },
            },
            ImgOp::Resize {
                src,
                out,
                width,
                height,
                filter,
                exact,
            } => img_cmd::Op::Resize {
                src,
                out,
                width,
                height,
                filter,
                exact,
            },
            ImgOp::Crop {
                src,
                out,
                x,
                y,
                w,
                h,
            } => img_cmd::Op::Crop {
                src,
                out,
                x,
                y,
                w,
                h,
            },
            ImgOp::Convert { src, out } => img_cmd::Op::Convert { src, out },
            ImgOp::Composite {
                base,
                overlay,
                out,
                x,
                y,
            } => img_cmd::Op::Composite {
                base,
                overlay,
                out,
                x,
                y,
            },
            ImgOp::Planche {
                srcs,
                out,
                manifeste,
                colonnes,
                marge,
                gouttiere,
                fond,
            } => img_cmd::Op::Planche {
                srcs,
                out,
                manifeste,
                colonnes,
                marge,
                gouttiere,
                fond,
            },
            ImgOp::Diff {
                rendu,
                reference,
                roi,
                out,
                downscale_ref,
                amplification,
            } => img_cmd::Op::Diff {
                rendu,
                reference,
                roi,
                out,
                downscale_ref,
                amplification,
            },
        }),
        #[cfg(feature = "host")]
        Cmd::Atlas { op } => atlas_cmd::run(op),
        #[cfg(feature = "host")]
        Cmd::Coverage { db } => coverage(&db),
        #[cfg(feature = "host")]
        Cmd::Queue { op, redis, tag } => queue(op, &redis, &tag),
        #[cfg(feature = "host")]
        Cmd::Propagate { db, rounds } => propagate(&db, rounds),
        #[cfg(feature = "host")]
        Cmd::Rtti { db, exe } => rtti(&db, &exe),
        #[cfg(feature = "host")]
        Cmd::Index { db, exe } => index(&db, &exe),
        #[cfg(feature = "host")]
        Cmd::Disasm { db, exe } => disasm(&db, &exe),
        #[cfg(feature = "host")]
        Cmd::Pdata { db, exe } => pdata(&db, &exe),
        #[cfg(feature = "host")]
        Cmd::Rebuild { db, exe, rounds } => rebuild(&db, &exe, rounds),
        #[cfg(feature = "host")]
        Cmd::Recover {
            db,
            exe,
            dry_run,
            ghidra_csv,
        } => recover_cmd(&db, &exe, dry_run, ghidra_csv.as_deref()),
        Cmd::Save { op } => save_cmd(op),
        #[cfg(feature = "host")]
        #[cfg(feature = "host")]
        Cmd::UniformMap { game_dir, out } => uniform_map(&racine_jeu(game_dir), &out),
        #[cfg(feature = "host")]
        Cmd::Textures {
            game_dir,
            limit,
            manifest,
            redis: use_redis,
            redis_url,
        } => textures(
            &racine_jeu(game_dir),
            limit,
            &manifest,
            use_redis,
            &redis_url,
        ),
        #[cfg(feature = "host")]
        Cmd::Mem { op } => mem_cmd(op),
        #[cfg(feature = "host")]
        Cmd::MenuPredecode {
            game_dir,
            redis_url,
        } => menu_predecode_cmd(&racine_jeu(game_dir), &redis_url),
        Cmd::Vfs { op } => vfs_cmd(op),
        #[cfg(feature = "host")]
        Cmd::Exe { op } => match op {
            ExeOp::Lint(options) => exe_lint::run(&options),
        },
        #[cfg(feature = "host")]
        Cmd::Vn { op, game_dir } => {
            let vfs = open_vfs(game_dir)?;
            vn_cmd::run(&op, &vfs)
        }
        #[cfg(feature = "host")]
        Cmd::Video { op, game_dir } => {
            let vfs = open_vfs(game_dir)?;
            video_cmd::run(&op, &vfs)
        }
    }
}

/// Execute one CLI command in process and capture its terminal output.
///
/// The command runs on the same 64 MiB stack as the standalone binary. The
/// returned JSON-friendly record is the shared adapter used by every MCP tool;
/// no child process, shell, Bun runtime, or FFI boundary is involved.
#[cfg(feature = "host")]
#[must_use]
pub fn execute_captured(command: String, args: Vec<String>) -> CapturedCommand {
    if args.len() > 128 {
        return CapturedCommand::failed("too many CLI arguments (maximum 128)");
    }
    if let Some(argument) = args
        .iter()
        .find(|argument| argument.len() > 16 * 1024 || argument.contains('\0'))
    {
        return CapturedCommand::failed(format!(
            "invalid CLI argument ({} bytes; maximum 16384 and NUL is forbidden)",
            argument.len()
        ));
    }
    let thread = std::thread::Builder::new()
        .name(format!("nie-cli-{command}"))
        .stack_size(PILE_CLI)
        .spawn(move || {
            output::capture(|| {
                let argv = std::iter::once("nie".to_owned())
                    .chain(std::iter::once(command))
                    .chain(args)
                    .collect::<Vec<_>>();
                let result = Cli::try_parse_from(argv)
                    .map_err(anyhow::Error::from)
                    .and_then(dispatch);
                result.map_err(|error| format!("{error:#}"))
            })
        });

    match thread {
        Ok(handle) => match handle.join() {
            Ok((result, captured)) => CapturedCommand::from_result(result, captured),
            Err(_) => CapturedCommand::failed("the CLI command panicked"),
        },
        Err(error) => CapturedCommand::failed(format!("failed to start CLI thread: {error}")),
    }
}

// --- nie mem: live RE through nie-trace (Linux/Wine and native Windows) ---

#[cfg(feature = "host")]
mod host_only_cmds {
    use super::*;

    pub(crate) fn mem_cmd(op: MemOp) -> anyhow::Result<()> {
        match op {
            MemOp::Maps { pid, module, all } => mem_maps(pid, &module, all),
            MemOp::Base { pid, module } => mem_base(pid, &module),
            MemOp::Read {
                addr,
                len,
                pid,
                output,
            } => mem_read(&addr, len, pid, output.as_deref()),
            MemOp::Write { addr, data, pid } => mem_write(&addr, &data, pid),
            MemOp::Dump {
                pid,
                module,
                all,
                output,
            } => mem_dump(pid, &module, all, &output),
            MemOp::Scan {
                pattern,
                pid,
                module,
                all,
                limit,
            } => mem_scan(&pattern, pid, &module, all, limit),
            MemOp::LuaField {
                name,
                pid,
                strings,
                nodes,
                radius,
                numeric,
            } => crate::mem_lua::lua_field(pid, &name, strings, nodes, radius, numeric),
            MemOp::Palettes {
                catalogue,
                pid,
                addr,
                len,
                output,
                fusionner,
            } => mem_palettes(&catalogue, pid, &addr, len, output.as_deref(), fusionner),
            MemOp::Catalog { category, json } => mem_catalog(category.as_deref(), json),
            MemOp::Recette {
                file,
                pid,
                module,
                force,
            } => mem_recette(&file, pid, &module, force),
        }
    }

    /// Read the avatar editor palettes from the live game and print them as JSON.
    fn mem_palettes(
        catalogue: &std::path::Path,
        pid: i32,
        addr: &str,
        len: usize,
        output: Option<&std::path::Path>,
        fusionner: bool,
    ) -> anyhow::Result<()> {
        let brut = std::fs::read_to_string(catalogue)
            .with_context(|| format!("reading the catalogue {}", catalogue.display()))?;
        let doc: serde_json::Value = serde_json::from_str(&brut)?;
        let mut attendus: Vec<u32> = Vec::new();
        if let Some(cats) = doc.get("categories").and_then(|c| c.as_array()) {
            for cat in cats {
                for c in cat
                    .get("couleurs")
                    .and_then(|c| c.as_array())
                    .into_iter()
                    .flatten()
                {
                    if let Some(v) = c.as_str().and_then(|h| u32::from_str_radix(h, 16).ok()) {
                        attendus.push(v);
                    }
                }
            }
        }
        attendus.sort_unstable();
        attendus.dedup();
        anyhow::ensure!(
            !attendus.is_empty(),
            "no palette id in the catalogue"
        );

        let debut = u64::from_str_radix(addr.trim_start_matches("0x"), 16)
            .with_context(|| format!("invalid address: {addr}"))?;
        let table = crate::mem_lua::palettes(pid, &attendus, debut, len)?;

        let json: serde_json::Map<String, serde_json::Value> = table
            .iter()
            .map(|(id, argb)| {
                (
                    format!("{id:08X}"),
                    serde_json::json!({
                        "rgb": format!("{:02X}{:02X}{:02X}", argb[1], argb[2], argb[3]),
                        "alpha": argb[0],
                    }),
                )
            })
            .collect();
        // Merging into the catalogue puts the colors within reach of any consumer that already loads it;
        // the client has nothing more to fetch.
        if fusionner {
            let mut catalogue_doc: serde_json::Value = serde_json::from_str(&brut)?;
            if let Some(obj) = catalogue_doc.as_object_mut() {
                obj.insert(
                    "couleursRgb".to_string(),
                    serde_json::Value::Object(json.clone()),
                );
            }
            std::fs::write(catalogue, serde_json::to_string(&catalogue_doc)?)
                .with_context(|| format!("writing {}", catalogue.display()))?;
            println!(
                "  {} / {} palette(s) merged into {}",
                table.len(),
                attendus.len(),
                catalogue.display()
            );
            return Ok(());
        }

        let doc = serde_json::Value::Object(json);
        match output {
            Some(p) => {
                std::fs::write(p, serde_json::to_string_pretty(&doc)?)?;
                println!(
                    "  {} / {} palette(s) -> {}",
                    table.len(),
                    attendus.len(),
                    p.display()
                );
            }
            None => println!("{}", serde_json::to_string_pretty(&doc)?),
        }
        Ok(())
    }

    /// Resolve and validate the pid (auto-detects nie.exe if 0) and check process access.
    ///
    /// `nie-trace` picks the native backend at compile time: `process_vm_readv` on Linux/Wine, `OpenProcess`/`ReadProcessMemory`/`VirtualQueryEx` on Windows. The game stays an existing process: this command neither launches nor stops it.
    fn mem_preflight(pid: i32) -> anyhow::Result<i32> {
        let pid = if pid <= 0 {
            let p = nie_trace::find_pid_by_name("nie.exe")
                .context("nie.exe not found: start the game or pass --pid")?;
            eprintln!("# nie.exe -> pid {p}");
            p
        } else {
            pid
        };
        // Process existence check: `/proc/<pid>` exists only on Linux. Elsewhere, leave it to
        // `nie-trace`: its Windows backend (`OpenProcess`/`VirtualQueryEx`) returns a
        // clear error if the pid is dead or inaccessible.
        #[cfg(target_os = "linux")]
        if !std::path::Path::new(&format!("/proc/{pid}")).exists() {
            anyhow::bail!("pid {pid} does not exist.");
        }
        #[cfg(windows)]
        if nie_trace::enumerate_regions(pid).is_empty() {
            anyhow::bail!(
                "pid {pid} is inaccessible: no such process, or insufficient rights \
             (the Windows backend uses OpenProcess/VirtualQueryEx; the reader needs \
             rights at least equal to the game's)."
            );
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        anyhow::bail!(
            "memory reading is not supported on this platform; use Linux/Wine or Windows"
        );
        #[cfg(target_os = "linux")]
        if !nie_trace::likely_permitted(pid) {
            let scope = nie_trace::read_ptrace_scope();
            eprintln!(
                "# warning: ptrace_scope={scope} and the reader is not an ancestor of {pid}. \
             Read probably denied (EPERM). Fix: \
             `echo 0 | sudo tee /proc/sys/kernel/yama/ptrace_scope`."
            );
        }
        Ok(pid)
    }

    fn mem_maps(pid: i32, module: &str, all: bool) -> anyhow::Result<()> {
        let pid = mem_preflight(pid)?;
        let maps = nie_trace::module_regions(pid, module, all);
        let mut total: u64 = 0;
        for m in &maps {
            total += m.size();
            println!(
                "  0x{:012x}-0x{:012x}  {}  {:>12}  {}",
                m.start,
                m.end,
                m.perms,
                m.size(),
                m.path
            );
        }
        let suffix = if all {
            String::new()
        } else {
            format!(" (module `{module}`)")
        };
        println!("\n  {} range(s), {total} bytes{suffix}", maps.len());
        Ok(())
    }

    fn mem_base(pid: i32, module: &str) -> anyhow::Result<()> {
        let pid = mem_preflight(pid)?;
        match nie_trace::find_module_base(pid, module) {
            Some(base) => println!("  {module} @ 0x{base:x} (pid {pid})"),
            None => anyhow::bail!("module `{module}` not found in the ranges of pid {pid}"),
        }
        Ok(())
    }

    fn mem_read(
        addr: &str,
        len: usize,
        pid: i32,
        output: Option<&std::path::Path>,
    ) -> anyhow::Result<()> {
        let pid = mem_preflight(pid)?;
        let address = mem_resolve_addr(addr, pid)?;
        let mut buf = vec![0u8; len];
        let got = nie_trace::read(pid, address, &mut buf).context("reading live memory")?;
        if got == 0 {
            anyhow::bail!("0 bytes read (unmapped range?)");
        }
        match output {
            Some(p) => {
                std::fs::write(p, &buf[..got])?;
                println!("  {got} bytes @ 0x{address:x} -> {}", p.display());
            }
            None => crate::print_hexdump(&buf[..got], address),
        }
        Ok(())
    }

    fn mem_write(addr: &str, data: &str, pid: i32) -> anyhow::Result<()> {
        let pid = mem_preflight(pid)?;
        let address = mem_resolve_addr(addr, pid)?;
        let bytes = mem_parse_write_data(data)?;
        nie_trace::write_exact(pid, address, &bytes).context("writing live memory")?;
        println!(
            "  {} byte(s) written @ 0x{address:x} (pid {pid})",
            bytes.len()
        );
        Ok(())
    }

    fn mem_parse_write_data(data: &str) -> anyhow::Result<Vec<u8>> {
        let trimmed = data.trim();
        if let Some(s) = trimmed.strip_prefix("str:") {
            return Ok(s.as_bytes().to_vec());
        }
        if let Some(s) = trimmed.strip_prefix("wstr:") {
            return Ok(s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect());
        }
        let cleaned = trimmed
            .strip_prefix("0x")
            .or_else(|| trimmed.strip_prefix("0X"))
            .unwrap_or(trimmed);
        let hex_clean: String = cleaned.chars().filter(|c| !c.is_whitespace()).collect();
        if hex_clean.len().is_multiple_of(2)
            && !hex_clean.is_empty()
            && hex_clean.chars().all(|c| c.is_ascii_hexdigit())
            && let Ok(b) = (0..hex_clean.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex_clean[i..i + 2], 16))
                .collect::<Result<Vec<u8>, _>>()
        {
            return Ok(b);
        }
        if let Ok(num) = trimmed.parse::<u64>() {
            if num <= u8::MAX as u64 {
                return Ok(vec![num as u8]);
            }
            if num <= u16::MAX as u64 {
                return Ok((num as u16).to_le_bytes().to_vec());
            }
            if num <= u32::MAX as u64 {
                return Ok((num as u32).to_le_bytes().to_vec());
            }
            return Ok(num.to_le_bytes().to_vec());
        }
        anyhow::bail!(
            "invalid data format: use hex bytes ('90 90', '0x9090'), 'str:...', 'wstr:...' or an integer"
        );
    }

    fn mem_dump(pid: i32, module: &str, all: bool, output: &std::path::Path) -> anyhow::Result<()> {
        let pid = mem_preflight(pid)?;
        let maps = nie_trace::module_regions(pid, module, all);
        let stats = nie_trace::dump_regions(pid, &maps, output)?;
        println!(
            "  {} range(s) dumped, {} bytes -> {}",
            stats.regions,
            stats.bytes,
            output
                .canonicalize()
                .unwrap_or_else(|_| output.to_path_buf())
                .display()
        );
        Ok(())
    }

    fn mem_scan(
        pattern: &str,
        pid: i32,
        module: &str,
        all: bool,
        limit: usize,
    ) -> anyhow::Result<()> {
        let pid = mem_preflight(pid)?;
        let (needle, label) = mem_parse_pattern(pattern)?;
        let maps = nie_trace::module_regions(pid, module, all);
        let base = nie_trace::find_module_base(pid, module);
        let hits = nie_trace::scan_regions(pid, &maps, base, &needle, limit);
        for h in &hits {
            let rva = match h.rva {
                Some(r) => format!(" ({module}+0x{r:x})"),
                None => String::new(),
            };
            println!("  0x{:012x}{rva}  [{}]", h.addr, h.perms);
        }
        let capped = if hits.len() >= limit {
            format!(" (limited to {limit})")
        } else {
            String::new()
        };
        println!("\n  {} hit(s) for {label}{capped}", hits.len());
        Ok(())
    }

    fn mem_catalog(category: Option<&str>, json: bool) -> anyhow::Result<()> {
        let cat_filter = category.and_then(|c| match c.to_ascii_lowercase().as_str() {
            "player" => Some(nie_trace::catalog::Category::Player),
            "match" => Some(nie_trace::catalog::Category::Match),
            "shop" => Some(nie_trace::catalog::Category::Shop),
            "spirit" => Some(nie_trace::catalog::Category::Spirit),
            "passive" => Some(nie_trace::catalog::Category::Passive),
            _ => None,
        });

        let entries: Vec<&'static nie_trace::catalog::Entry> = nie_trace::catalog::CATALOG
            .iter()
            .filter(|e| cat_filter.is_none() || cat_filter == Some(e.category))
            .collect();

        if json {
            let list: Vec<serde_json::Value> = entries
            .iter()
            .map(|e| {
                serde_json::json!({
                    "id": e.id,
                    "feature": e.feature,
                    "category": e.category.label(),
                    "kind": format!("{:?}", e.kind),
                    "ty": e.ty.name(),
                    "aob": e.aob,
                    "rva": e.rva.map(|r| format!("0x{r:X}")),
                    "staticAddr": e.static_addr().map(|a| format!("0x{a:X}")),
                    "field": e.field.map(|f| format!("0x{f:X}")),
                    "chain": e.chain.map(|c| c.iter().map(|offset| format!("+0x{offset:X}")).collect::<Vec<_>>()),
                    "doc": e.doc,
                })
            })
            .collect();
            println!("{}", serde_json::to_string_pretty(&list)?);
        } else {
            println!(
                "{:<22} {:<10} {:<6} {:<12} {:<16} {}",
                "ID", "CATEGORIE", "TYPE", "RVA", "OFFSET/CHAIN", "DESCRIPTION"
            );
            println!("{}", "-".repeat(100));
            for e in entries {
                let rva_str = e.rva.map_or_else(|| "-".into(), |r| format!("0x{r:08X}"));
                let locator = if let Some(field) = e.field {
                    format!("+0x{field:X}")
                } else if let Some(chain) = e.chain {
                    chain
                        .iter()
                        .map(|o| format!("+0x{o:X}"))
                        .collect::<Vec<_>>()
                        .join(" -> ")
                } else {
                    "-".into()
                };
                println!(
                    "{:<22} {:<10} {:<6} {:<12} {:<16} {}",
                    e.id,
                    e.category.label(),
                    e.ty.name(),
                    rva_str,
                    locator,
                    e.doc
                );
            }
        }
        Ok(())
    }

    fn mem_recette(
        file: &std::path::Path,
        pid: i32,
        module: &str,
        force: bool,
    ) -> anyhow::Result<()> {
        let pid = mem_preflight(pid)?;
        let content = std::fs::read_to_string(file)
            .with_context(|| format!("reading recipe {}", file.display()))?;
        let r = nie_trace::recette::parser(&content)
            .map_err(|e| anyhow::anyhow!("recipe syntax error: {e}"))?;

        let name = if r.nom.is_empty() {
            file.display().to_string()
        } else {
            r.nom.clone()
        };
        println!(
            "\n  recipe `{}` ({} rule(s)) on pid {pid} [{}]",
            name,
            r.regles.len(),
            if force {
                "WRITE ENABLED"
            } else {
                "DRY RUN"
            }
        );

        let rapport = nie_trace::recette::appliquer(pid, module, &r, !force);
        for res in &rapport.resultats {
            let statut = if res.erreur.is_none() && res.trouvees > 0 {
                "OK"
            } else {
                "FAILED"
            };
            let msg = res.erreur.as_deref().unwrap_or("");
            println!(
                "  [{statut}] rule {:?} (found={}, written={}): {msg}",
                res.regle, res.trouvees, res.ecrites
            );
        }
        println!(
            "\n  Summary: {} write(s), {} failure(s)",
            rapport.total_ecrites(),
            rapport.echecs()
        );
        if !force {
            println!("  Note: dry run by default. Pass --force to apply the writes.");
        }
        Ok(())
    }

    /// Parse `0x…` (absolute) or `module+0xRVA` (resolved through the module base).
    fn mem_resolve_addr(addr: &str, pid: i32) -> anyhow::Result<u64> {
        let s = addr.trim();
        if let Some(plus) = s.find('+') {
            let module = &s[..plus];
            let rva = parse_u64_hex(&s[plus + 1..])
                .with_context(|| format!("invalid RVA: {}", &s[plus + 1..]))?;
            let base = nie_trace::find_module_base(pid, module)
                .with_context(|| format!("module `{module}` not found"))?;
            return Ok(base + rva);
        }
        parse_u64_hex(s).with_context(|| format!("invalid address: {s}"))
    }

    fn parse_u64_hex(s: &str) -> anyhow::Result<u64> {
        let s = s.trim();
        let s = s
            .strip_prefix("0x")
            .or_else(|| s.strip_prefix("0X"))
            .unwrap_or(s);
        Ok(u64::from_str_radix(s, 16)?)
    }

    /// Parse a pattern: `wstr:text` (UTF-16LE), `str:text` (UTF-8), or hex bytes `48 8B 0D`.
    fn mem_parse_pattern(pattern: &str) -> anyhow::Result<(Vec<u8>, String)> {
        if let Some(text) = pattern.strip_prefix("wstr:") {
            let needle: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
            anyhow::ensure!(!needle.is_empty(), "empty wstr: pattern");
            return Ok((needle, format!("wstr \"{text}\"")));
        }
        if let Some(text) = pattern.strip_prefix("str:") {
            anyhow::ensure!(!text.is_empty(), "empty str: pattern");
            return Ok((text.as_bytes().to_vec(), format!("\"{text}\"")));
        }
        let hex: String = pattern
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-')
            .collect();
        anyhow::ensure!(
            !hex.is_empty() && hex.len().is_multiple_of(2),
            "hex pattern of odd length or empty"
        );
        let mut bytes = Vec::with_capacity(hex.len() / 2);
        for i in (0..hex.len()).step_by(2) {
            bytes.push(
                u8::from_str_radix(&hex[i..i + 2], 16)
                    .with_context(|| format!("invalid hex byte at {i}"))?,
            );
        }
        let label = format!(
            "hex {}",
            bytes
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect::<Vec<_>>()
                .join("-")
        );
        Ok((bytes, label))
    }

    pub(crate) fn seed(
        db_path: &std::path::Path,
        json: &std::path::Path,
        exe: Option<&std::path::Path>,
    ) -> anyhow::Result<()> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let mut db = nie_index::Db::open(db_path).context("opening the database")?;

        let (sha, size, path_str) = match exe {
            Some(p) => {
                let bytes = std::fs::read(p).with_context(|| format!("reading {}", p.display()))?;
                let mut h = Sha256::new();
                h.update(&bytes);
                (
                    hex::encode(h.finalize()),
                    bytes.len() as i64,
                    p.display().to_string(),
                )
            }
            None => ("unknown-nie-index".to_string(), 0, "nie.exe".to_string()),
        };
        let bin = db.upsert_binary(
            &path_str,
            &sha,
            "x86_64",
            64,
            NIE_IMAGE_BASE,
            size,
            None,
            None,
        )?;

        let stats = nie_seed::nie_index_json::ingest_file(&mut db, bin, json)?;
        let cov = db.snapshot_coverage(bin)?;
        println!(
            "seed fn={} call={} str={} const={} glob={} anchor={} cov={}/{} ({:.2}%)",
            stats.functions,
            stats.xrefs,
            stats.str_refs,
            stats.consts,
            stats.globals,
            stats.anchors,
            cov.classified,
            cov.total,
            cov.pct
        );
        Ok(())
    }

    pub(crate) fn seed_ui_cmd(
        db_path: &std::path::Path,
        game_dir: Option<PathBuf>,
        textures: bool,
    ) -> anyhow::Result<()> {
        let vfs = open_vfs(game_dir)?;
        let db = nie_index::Db::open(db_path)
            .with_context(|| format!("opening {}", db_path.display()))?;
        // `hash_name` may not exist yet on a fresh database.
        db.init().context("applying the schema")?;

        let stats = seed_ui::run(&db, &vfs, textures)?;
        let par_kind = stats
            .par_kind
            .iter()
            .map(|(k, n)| format!("{k}={n}"))
            .collect::<Vec<_>>()
            .join(" ");
        println!(
            "seed-ui screens={} objbin={} g4tx={} | {par_kind}",
            stats.screens, stats.objbins, stats.g4tx
        );
        println!(
            "  hash_name rows (source vfs-ui) = {}; crc_mismatch={}; skipped={}",
            stats.inserted, stats.crc_mismatch, stats.skipped
        );
        Ok(())
    }

    pub(crate) fn strings_cmd_run(
        db_path: &std::path::Path,
        exe: &std::path::Path,
        opts: &strings_cmd::Options,
    ) -> anyhow::Result<()> {
        let db = nie_index::Db::open(db_path)
            .with_context(|| format!("opening {}", db_path.display()))?;
        // `str.kind` / `func_str_ref.source` may be missing on an older database.
        db.init().context("applying the schema")?;

        let s = strings_cmd::run(&db, exe, opts)?;
        let par_sec = s
            .par_section
            .iter()
            .map(|(n, a, w)| format!("{n}={a}+{w}"))
            .collect::<Vec<_>>()
            .join(" ");
        println!(
            "strings binary={} ({}) | ascii={} utf16={} (too long={}) | {par_sec}",
            s.binary_id, s.binary_path, s.ascii, s.utf16, s.oversized
        );
        if s.dry_run {
            println!("  DRY RUN: nothing written");
        } else {
            println!("  str: {} rows inserted", s.str_inserted);
        }
        if !opts.no_xrefs {
            println!(
                "  .text: {} .pdata roots ({} with a function row), {} bodies decoded ({} truncated), {} instructions",
                s.roots, s.roots_mapped, s.funcs_scanned, s.bodies_truncated, s.insns
            );
            println!(
                "  hits: lea exact={} + suffix={} over {} lea rip | imm64={} | non-lea rip on string (not ingested) = {}",
                s.lea_hits, s.lea_suffix, s.lea_rip, s.imm_hits, s.rip_other_hits
            );
            println!(
                "  (function, string) pairs = {} over {} functions; suffixes materialized = {}",
                s.pairs, s.funcs_with_str, s.suffixes
            );
            println!(
                "  func_str_ref +{} (-{} replaced); xref str +{}",
                s.str_refs_inserted, s.str_refs_deleted, s.xrefs_inserted
            );
        }
        Ok(())
    }

    /// Binary to **read** for a measure or a propagation.
    ///
    /// Two binaries coexist: the Ghidra index (lowest `id`, misaligned addresses) and the `…#pdata` ground truth produced by `rebuild`. `ORDER BY id LIMIT 1` picks the first, which carries no function or anchor once the knowledge base was rebuilt on `.pdata`, so the command reports `0/0` without error. Prefer `#pdata` when it exists.
    fn analysis_binary(conn: &nie_index::rusqlite::Connection) -> anyhow::Result<i64> {
        conn.query_row(
            "SELECT id FROM binary ORDER BY (path LIKE '%#pdata') DESC, id LIMIT 1",
            [],
            |r| r.get(0),
        )
        .context("no indexed binary: run `nie seed` first")
    }

    pub(crate) fn coverage(db_path: &std::path::Path) -> anyhow::Result<()> {
        let db = nie_index::Db::open(db_path)?;
        let bin: i64 = analysis_binary(db.conn())?;
        let cov = nie_index::query::coverage(db.conn(), bin)?;
        let by_sub = nie_index::query::by_subsystem(db.conn(), bin)?;
        let subs = by_sub
            .iter()
            .map(|(ns, n)| format!("{ns}={n}"))
            .collect::<Vec<_>>()
            .join(" ");
        println!(
            "cov {}/{} ({:.2}%) named={} | {}",
            cov.classified, cov.total, cov.pct, cov.named, subs
        );
        Ok(())
    }

    pub(crate) fn queue(op: QueueOp, redis: &str, tag: &str) -> anyhow::Result<()> {
        let mut f = nie_queue::Frontier::connect(redis, tag)?;
        match op {
            QueueOp::Push { addr } => {
                let added = f.push(addr)?;
                println!("{}", if added { "added" } else { "already seen" });
            }
            QueueOp::Pop => match f.pop()? {
                Some(a) => println!("0x{a:x}"),
                None => println!("(empty frontier)"),
            },
            QueueOp::Len => println!("frontier: {} | seen: {}", f.len()?, f.seen_count()?),
            QueueOp::Reset => {
                f.reset()?;
                println!("frontier reset");
            }
        }
        Ok(())
    }

    pub(crate) fn propagate(db_path: &std::path::Path, rounds: usize) -> anyhow::Result<()> {
        let mut db = nie_index::Db::open(db_path).context("opening the database")?;
        let bin: i64 = analysis_binary(db.conn())?;

        let stats =
            nie_re_core::nie::loop_db::propagate_db(&mut db, bin, rounds).context("propagation")?;

        println!(
            "propagate rounds={} anchors(str/rtti/const)={}/{}/{} cov {:.2}%->{:.2}% (+{} fn)",
            stats.rounds,
            stats.anchored_str,
            stats.anchored_rtti,
            stats.anchored_const,
            stats.coverage_before,
            stats.coverage_after,
            stats.classified_after - stats.classified_before
        );
        Ok(())
    }

    pub(crate) fn rtti(
        db_path: &std::path::Path,
        exe_path: &std::path::Path,
    ) -> anyhow::Result<()> {
        let mut db = nie_index::Db::open(db_path).context("opening the database")?;
        let bin: i64 = db
            .conn()
            .query_row("SELECT id FROM binary ORDER BY id LIMIT 1", [], |r| {
                r.get(0)
            })
            .context("no indexed binary: run `nie seed` first")?;

        let bytes =
            std::fs::read(exe_path).with_context(|| format!("reading {}", exe_path.display()))?;

        let stats = nie_re_core::nie::rtti::parse_and_ingest(&mut db, bin, &bytes)
            .context("parsing RTTI")?;

        println!(
            "rtti col={} td={} classes={} bases={}",
            stats.candidates, stats.valid_type_descs, stats.classes_ingested, stats.bases_ingested
        );
        Ok(())
    }

    pub(crate) fn index(
        db_path: &std::path::Path,
        exe_path: &std::path::Path,
    ) -> anyhow::Result<()> {
        let mut db = nie_index::Db::open(db_path).context("opening the database")?;
        let bin: i64 = db
            .conn()
            .query_row("SELECT id FROM binary ORDER BY id LIMIT 1", [], |r| {
                r.get(0)
            })
            .context("no indexed binary: run `nie seed` first")?;

        let stats = nie_re_core::nie::indexer::triage_into(&mut db, bin, exe_path)
            .context("indexing PE")?;

        println!(
            "index fmt={} sections={} imports={} exports={}",
            stats.format, stats.sections, stats.imports, stats.exports
        );
        Ok(())
    }

    pub(crate) fn disasm(
        db_path: &std::path::Path,
        exe_path: &std::path::Path,
    ) -> anyhow::Result<()> {
        let mut db = nie_index::Db::open(db_path).context("opening the database")?;
        let bin: i64 = db
            .conn()
            .query_row("SELECT id FROM binary ORDER BY id LIMIT 1", [], |r| {
                r.get(0)
            })
            .context("no indexed binary: run `nie seed` first")?;

        // A/B: NIE_NO_INDIRECT=1 detects the LEAs (stats) but does not insert them.
        let skip_lea = std::env::var("NIE_NO_INDIRECT").is_ok();
        let stats = nie_re_core::nie::disasm::recover_call_edges(&mut db, bin, exe_path, skip_lea)
            .context("disassembling the call edges")?;

        println!(
            "disasm insn={} call={} jmp={} thunk={} miss={} cand={} new={} | lea_insn={} lea_cand={} lea_new={}",
            stats.instructions_decoded,
            stats.call_near,
            stats.jmp_near,
            stats.thunk_resolved,
            stats.near_target_miss,
            stats.edges_candidates,
            stats.edges_new,
            stats.lea_insns,
            stats.lea_candidates,
            stats.lea_edges_new
        );
        Ok(())
    }

    pub(crate) fn pdata(
        db_path: &std::path::Path,
        exe_path: &std::path::Path,
    ) -> anyhow::Result<()> {
        let mut db = nie_index::Db::open(db_path).context("opening the database")?;
        let bin: i64 = db
            .conn()
            .query_row("SELECT id FROM binary ORDER BY id LIMIT 1", [], |r| {
                r.get(0)
            })
            .context("no indexed binary: run `nie seed` first")?;

        let stats = nie_re_core::nie::pdata::discover_into(&mut db, bin, exe_path)
            .context("discovering .pdata")?;

        let pct_aligned = if stats.ghidra_total > 0 {
            100.0 * stats.overlap_ghidra as f64 / stats.ghidra_total as f64
        } else {
            0.0
        };
        println!(
            "pdata entries={} chained={} roots={} inserted={} | ghidra {}/{} aligned ({:.1}%) inside_body={}",
            stats.entries,
            stats.chained_fragments,
            stats.roots,
            stats.inserted,
            stats.overlap_ghidra,
            stats.ghidra_total,
            pct_aligned,
            stats.ghidra_inside_body
        );
        Ok(())
    }

    /// Resolve the id of the `…#pdata` binary (ground truth for function bounds).
    ///
    /// `rebuild` creates it from the first indexed binary; passes working on functions must target this one, not the original Ghidra index.
    fn pdata_binary_id(db: &nie_index::Db) -> anyhow::Result<i64> {
        db.conn()
            .query_row(
                "SELECT id FROM binary WHERE path LIKE '%#pdata' ORDER BY id LIMIT 1",
                [],
                |r| r.get(0),
            )
            .context("#pdata binary missing: run `nie rebuild` first")
    }

    pub(crate) fn recover_cmd(
        db_path: &std::path::Path,
        exe_path: &std::path::Path,
        dry_run: bool,
        ghidra_csv: Option<&std::path::Path>,
    ) -> anyhow::Result<()> {
        let mut db = nie_index::Db::open(db_path).context("opening the database")?;
        let bin = pdata_binary_id(&db)?;
        let mut observe = |step| eprintln!("recover: step done: {step:?}");
        let report = nie_re_core::nie::workflow::recover_with_observer(
            &mut db,
            exe_path,
            &nie_re_core::nie::workflow::RecoverOptions {
                binary_id: bin,
                dry_run,
                ghidra_csv: ghidra_csv.map(std::path::Path::to_path_buf),
            },
            &mut observe,
        )?;

        if let Some(gs) = report.ghidra {
            println!(
                "  ghidra: {} rows | {} default names dropped, {} unmatched addresses | {} names written ({} replacing a structural name)",
                gs.rows, gs.default_names, gs.unmatched, gs.named, gs.replaced_struct,
            );
        }
        let st = report.leaves;
        let pct_gap = if st.gap_bytes > 0 {
            100.0 * (st.recovered_gap_bytes + st.padding_bytes) as f64 / st.gap_bytes as f64
        } else {
            0.0
        };
        println!(
            "recover pdata={} B | gaps={} ({} B) | new candidates={} | leaves measured={} (by ref={} by scan={}) rejected={} | code measured={} B | code in gaps={} B padding={} B | residue={} B | explains {:.2}% of gaps | inserted={} thunks={} edges={}",
            st.pdata_bytes,
            st.gaps,
            st.gap_bytes,
            st.candidates,
            st.recovered,
            st.by_ref,
            st.by_scan,
            st.rejected,
            st.recovered_bytes,
            st.recovered_gap_bytes,
            st.padding_bytes,
            st.gap_bytes_left,
            pct_gap,
            st.inserted,
            st.thunks_named,
            st.edges_new,
        );
        println!(
            "  shapes: thunk={} const={} ptr={} stub={} | structural names={} | subsystem inherited={} | false bounds purged={}",
            st.shape_thunk,
            st.shape_const,
            st.shape_ptr,
            st.shape_stub,
            st.shape_named,
            st.shape_inherited,
            st.pruned,
        );
        let Some(enrichment) = report.enrichment else {
            return Ok(());
        };
        let av = enrichment.anonymous_vtables;
        println!(
            "  vtables without RTTI: {}/{} tables ({} slots, {} methods) | functions+={} cohesion={} names={}",
            av.tables,
            av.tables_seen,
            av.slots,
            av.methods,
            av.new_funcs,
            av.cohesion_edges,
            av.named,
        );
        let sr = enrichment.string_references;
        println!(
            "  strings: {} read | {} functions scanned, {} references ({} new) | {} identifying strings with a unique referent | {} semantic names",
            sr.strings, sr.scanned, sr.refs, sr.refs_new, sr.unique_idents, sr.named,
        );
        let fl = enrichment.func_lua;
        println!(
            "  funcLua: {} tables ({} entries, {} handlers) | functions+={} named={} script classes={}",
            fl.tables, fl.entries, fl.handlers, fl.new_funcs, fl.named, fl.classified,
        );
        let ad = enrichment.adjacency;
        println!(
            "  contiguity: {} classified | coherence {:.1}% measured over {} control cases (agreement with the existing labeling, not ground truth)",
            ad.classified,
            ad.precision_estimate(),
            ad.control_cases,
        );
        let cov = enrichment.coverage;
        println!(
            "  coverage: {}/{} classified ({:.2}%), {} named",
            cov.classified, cov.total, cov.pct, cov.named,
        );
        Ok(())
    }

    pub(crate) fn rebuild(
        db_path: &std::path::Path,
        exe_path: &std::path::Path,
        rounds: usize,
    ) -> anyhow::Result<()> {
        let mut db = nie_index::Db::open(db_path).context("opening the database")?;
        let source_binary_id = db
            .conn()
            .query_row("SELECT id FROM binary ORDER BY id LIMIT 1", [], |row| {
                row.get(0)
            })
            .context("no indexed binary: run `nie seed` first")?;
        let report = nie_re_core::nie::workflow::rebuild(
            &mut db,
            exe_path,
            nie_re_core::nie::workflow::RebuildOptions {
                source_binary_id,
                image_base: NIE_IMAGE_BASE,
                rounds,
                skip_indirect: std::env::var("NIE_NO_INDIRECT").is_ok(),
            },
        )?;
        let prop = report.propagation;
        let pct_conf = if prop.total > 0 {
            100.0 * report.classified_confident as f64 / prop.total as f64
        } else {
            0.0
        };
        let pct_named = if prop.total > 0 {
            100.0 * report.named_total as f64 / prop.total as f64
        } else {
            0.0
        };
        println!(
            "rebuild roots={} str={} ce={} rtti={} | vtable methods={} leaf+={} cohesion={} anchored={} named_struct={} | disasm new={} lea_new={} | named={}/{} ({:.2}%) | cov_raw={}/{} ({:.2}%) cov_conf>=0.3={}/{} ({:.2}%)",
            report.pdata.roots,
            report.pdata.str_refs_moved,
            report.pdata.ce_edges_mapped,
            report.pdata.rtti_copied,
            report.vtable.methods,
            report.vtable.new_leaf_funcs,
            report.vtable.cohesion_edges,
            report.vtable.class_anchored,
            report.vtable.named_struct,
            report.disassembly.edges_new,
            report.disassembly.lea_edges_new,
            report.named_total,
            prop.total,
            pct_named,
            prop.classified_after,
            prop.total,
            prop.coverage_after,
            report.classified_confident,
            prop.total,
            pct_conf,
        );
        Ok(())
    }

    /// An atlas region (sub-texture) of the manifest, with the same keys as `/tex-info` of `nie-model-serve`.
    #[derive(serde::Serialize)]
    struct TexRegion<'a> {
        name: &'a str,
        x: i16,
        y: i16,
        width: i16,
        height: i16,
    }

    /// NDJSON manifest entry for a G4TX texture.
    ///
    /// Carries `name` and `regions_detail` so texture names and atlas regions can be resolved; `regions` stays the count for existing consumers.
    #[derive(serde::Serialize)]
    struct TexEntry<'a> {
        path: &'a str,
        cpk: &'a str,
        name: &'a str,
        width: i32,
        height: i32,
        format: &'static str,
        mips: u8,
        regions: usize,
        #[serde(rename = "regionsDetail")]
        regions_detail: Vec<TexRegion<'a>>,
    }

    pub(crate) fn textures(
        game_dir: &std::path::Path,
        limit: usize,
        manifest_path: &std::path::Path,
        use_redis: bool,
        redis_url: &str,
    ) -> anyhow::Result<()> {
        use nie_formats::g4tx;
        use nie_formats::vfs::Vfs;

        let data_dir = game_dir.join("data");

        // Initialiser le VFS depuis cpk_list.cfg.bin
        let mut vfs = Vfs::new();
        vfs.init(&data_dir)
            .context("init VFS from cpk_list.cfg.bin")?;

        // Collect every indexed .g4tx path from the index the VFS just built (`vfs.iter()`).
        let mut all_g4tx: Vec<(String, String)> = vfs
            .iter()
            .filter(|(p, _)| p.ends_with(".g4tx"))
            .map(|(p, e)| (p.to_string(), e.cpk_filename.clone()))
            .collect();
        all_g4tx.sort();

        let total_found = all_g4tx.len();
        let to_process = all_g4tx.len().min(limit);
        let dropped = total_found.saturating_sub(limit);

        tracing::info!(
            total_found,
            to_process,
            dropped,
            ".g4tx files discovered"
        );

        // Prepare the manifest file
        if let Some(parent) = manifest_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let mut manifest_file = std::fs::File::create(manifest_path)
            .with_context(|| format!("creating manifest {}", manifest_path.display()))?;

        // Connexion Redis optionnelle
        let mut redis_conn: Option<redis::Connection> = if use_redis {
            match redis::Client::open(redis_url).and_then(|c| c.get_connection()) {
                Ok(conn) => {
                    tracing::info!("Redis connected: {redis_url}");
                    Some(conn)
                }
                Err(e) => {
                    tracing::warn!("Redis unavailable ({e}); continuing without Redis");
                    None
                }
            }
        } else {
            None
        };

        let mut parsed = 0usize;
        let mut failed = 0usize;

        for (internal_path, cpk_name) in all_g4tx.iter().take(limit) {
            let raw = match vfs.read(internal_path) {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!("extraction failed {internal_path}: {e}");
                    failed += 1;
                    continue;
                }
            };

            let g = match g4tx::parse(&raw) {
                Ok(g) => g,
                Err(e) => {
                    tracing::warn!("g4tx parse failed {internal_path}: {e}");
                    failed += 1;
                    continue;
                }
            };

            // For each texture in the g4tx container
            for tex in &g.textures {
                // Format: deduced from is_dds; mips: approximated from the dimensions
                // (the mips field is not in the public G4TX header: 0 is exposed as a sentinel)
                let format_str: &'static str = if tex.is_dds { "DDS" } else { "NXTCH" };
                let mips: u8 = 0; // G4txHeader exposes no explicit mips field

                let regions_detail: Vec<TexRegion> = tex
                    .sub_textures
                    .iter()
                    .map(|s| TexRegion {
                        name: s.name.as_str(),
                        x: s.x,
                        y: s.y,
                        width: s.width,
                        height: s.height,
                    })
                    .collect();

                let entry = TexEntry {
                    path: internal_path.as_str(),
                    cpk: cpk_name.as_str(),
                    name: tex.name.as_str(),
                    width: tex.width,
                    height: tex.height,
                    format: format_str,
                    mips,
                    regions: tex.sub_textures.len(),
                    regions_detail,
                };

                let line = serde_json::to_string(&entry).context("serializing JSON")?;
                writeln!(manifest_file, "{line}").context("writing manifest")?;

                // Push to Redis if enabled
                if let Some(ref mut conn) = redis_conn {
                    use redis::Commands;
                    let redis_path_key = format!("iev:tex:{internal_path}");
                    if let Err(e) = conn.sadd::<_, _, i64>("iev:tex:index", internal_path.as_str())
                    {
                        tracing::warn!("redis SADD failed: {e}");
                    }
                    if let Err(e) = conn.hset_multiple::<_, _, _, ()>(
                        &redis_path_key,
                        &[
                            ("name", tex.name.clone()),
                            ("width", tex.width.to_string()),
                            ("height", tex.height.to_string()),
                            ("format", format_str.to_string()),
                            ("mips", mips.to_string()),
                            ("regions", tex.sub_textures.len().to_string()),
                            ("cpk", cpk_name.clone()),
                        ],
                    ) {
                        tracing::warn!("redis HSET failed: {e}");
                    }
                    // Inverted region-name -> container index: this is what is missing to resolve
                    // "where is gtxt_rarity01_05" without reparsing every g4tx.
                    for region in &tex.sub_textures {
                        if let Err(e) = conn.sadd::<_, _, i64>(
                            format!("iev:tex:region:{}", region.name),
                            internal_path.as_str(),
                        ) {
                            tracing::warn!("redis SADD region failed: {e}");
                        }
                    }
                }
            }

            parsed += 1;
        }

        // Write the Redis meta
        if let Some(ref mut conn) = redis_conn {
            use redis::Commands;
            if let Err(e) = conn.hset_multiple::<_, _, _, ()>(
                "iev:tex:meta",
                &[
                    ("parsed", parsed.to_string()),
                    ("failed", failed.to_string()),
                    ("total_found", total_found.to_string()),
                    ("limit", limit.to_string()),
                    ("dropped", dropped.to_string()),
                ],
            ) {
                tracing::warn!("redis meta HSET failed: {e}");
            }
        }

        // Comptage Redis pour sortie terse
        let redis_count: usize = if let Some(ref mut conn) = redis_conn {
            use redis::Commands;
            conn.scard::<_, usize>("iev:tex:index").unwrap_or(0)
        } else {
            0
        };

        // Terse output (nie convention: one key=val line)
        if dropped > 0 {
            println!(
                "tex.parsed={parsed} tex.failed={failed} tex.total={total_found} tex.dropped={dropped} manifest={} redis_index={}",
                manifest_path.display(),
                redis_count
            );
        } else {
            println!(
                "tex.parsed={parsed} tex.failed={failed} tex.total={total_found} manifest={} redis_index={}",
                manifest_path.display(),
                redis_count
            );
        }

        Ok(())
    }
}
#[cfg(feature = "host")]
use host_only_cmds::{
    coverage, disasm, index, mem_cmd, pdata, propagate, queue, rebuild, recover_cmd, rtti, seed,
    seed_ui_cmd, strings_cmd_run, textures,
};

fn save_cmd(op: SaveOp) -> anyhow::Result<()> {
    use nie_save::io::{edit_blob_byte, hexdump_blob_body, print_summary, read_save, write_save};

    match op {
        SaveOp::Read { file, hexdump } => {
            let container = read_save(&file).map_err(|e| anyhow::anyhow!("reading save: {e}"))?;
            print_summary(&container);
            if hexdump > 0 {
                for (i, blob) in container.blobs.iter().enumerate() {
                    println!("  hexdump blob[{i}]:");
                    hexdump_blob_body(blob, hexdump);
                }
            }
        }
        SaveOp::Decrypt { file, out } => {
            // XOR direct sur le fichier brut : le keystream est involutif, donc
            // decrypt = encrypt = XOR(data, keystream).
            // serialize_plaintext() is NOT used, to stay byte-identical to the original.
            let raw =
                std::fs::read(&file).with_context(|| format!("reading {}", file.display()))?;
            let filename = file
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow::anyhow!("invalid file name: {}", file.display()))?;
            let key = nie_save::key_from_filename(filename);
            let mut plain = raw;
            nie_save::decrypt_block(&mut plain, 0, key);
            std::fs::write(&out, &plain)
                .with_context(|| format!("writing plaintext {}", out.display()))?;
            println!("decrypt ok={} bytes={}", out.display(), plain.len());
        }
        SaveOp::Encrypt { file, slot, out } => {
            let plain =
                std::fs::read(&file).with_context(|| format!("reading {}", file.display()))?;
            // Encrypt the plaintext with the key derived from the slot name
            let key = nie_save::key_from_filename(&slot);
            let mut enc = plain;
            nie_save::decrypt_block(&mut enc, 0, key);
            std::fs::write(&out, &enc).with_context(|| format!("writing {}", out.display()))?;
            println!(
                "encrypt slot={slot} key=0x{key:08X} out={} bytes={}",
                out.display(),
                enc.len()
            );
        }
        SaveOp::Edit {
            file,
            blob: blob_name,
            offset,
            value,
            out,
        } => {
            if !(0..=255).contains(&value) {
                anyhow::bail!("value {value} out of range [0, 255]");
            }
            if offset < 0 {
                anyhow::bail!("negative offset {offset}");
            }
            let mut container =
                read_save(&file).map_err(|e| anyhow::anyhow!("reading save: {e}"))?;
            let ok = edit_blob_byte(&mut container, &blob_name, offset as usize, value as u8);
            if !ok {
                anyhow::bail!(
                    "blob '{}' not found or offset {} out of range",
                    blob_name,
                    offset
                );
            }
            let out_path = out.as_deref().unwrap_or(&file);
            write_save(&container, out_path).map_err(|e| anyhow::anyhow!("writing save: {e}"))?;
            println!(
                "edit blob={blob_name} offset=0x{offset:X} value=0x{value:02X} out={}",
                out_path.display()
            );
        }
        #[cfg(feature = "host")]
        SaveOp::Park {
            live_save,
            mod_save,
        } => {
            launcher_save_park(&live_save, &mod_save)?;
        }
        #[cfg(feature = "host")]
        SaveOp::Restore {
            live_save,
            mod_save,
        } => {
            launcher_save_restore(&live_save, &mod_save)?;
        }
        #[cfg(feature = "host")]
        SaveOp::InjectTeam { save, team, out } => {
            launcher_save_inject_team(&save, &team, out.as_deref())?;
        }
    }
    Ok(())
}

#[cfg(feature = "host")]
mod host_launcher_and_maps {
    use super::*;

    pub(crate) fn launcher_save_park(
        live_save: &std::path::Path,
        mod_save: &std::path::Path,
    ) -> anyhow::Result<()> {
        let mut session = nie_launcher::SaveSession::new(live_save, mod_save);
        session
            .park_and_install_mod_save()
            .map_err(|e| anyhow::anyhow!("parking save: {e}"))?;
        session.is_active = false;
        println!(
            "park ok: live={} -> .bk, installed mod_save={}",
            live_save.display(),
            mod_save.display()
        );
        Ok(())
    }

    pub(crate) fn launcher_save_restore(
        live_save: &std::path::Path,
        mod_save: &std::path::Path,
    ) -> anyhow::Result<()> {
        let mut session = nie_launcher::SaveSession::new(live_save, mod_save);
        session.is_active = true;
        session
            .restore_original_save()
            .map_err(|e| anyhow::anyhow!("restoring save: {e}"))?;
        println!("restore ok: live={} restored from .bk", live_save.display());
        Ok(())
    }

    pub(crate) fn launcher_save_inject_team(
        save_path: &std::path::Path,
        team_path: &std::path::Path,
        out: Option<&std::path::Path>,
    ) -> anyhow::Result<()> {
        let team_content = std::fs::read_to_string(team_path)
            .with_context(|| format!("reading team {}", team_path.display()))?;
        let lineup: nie_launcher::TeamLineup = if let Ok(envelope) =
            serde_json::from_str::<nie_launcher::TeamExportEnvelope>(&team_content)
        {
            nie_launcher::decrypt_team_envelope(&envelope, nie_launcher::DEFAULT_PASSPHRASE)
                .map_err(|e| anyhow::anyhow!("team decryption: {e}"))?
        } else {
            serde_json::from_str(&team_content).context("deserializing TeamLineup")?
        };

        let mut container = nie_save::io::read_save(save_path)
            .map_err(|e| anyhow::anyhow!("reading save container: {e}"))?;

        let blob_idx = container
            .entries
            .iter()
            .position(|e| e.filename == "AUTOSAVE_data.bin" || e.filename == "SYSTEM_data.bin")
            .ok_or_else(|| {
                anyhow::anyhow!("no compatible data blob found in the save")
            })?;
        let blob = &mut container.blobs[blob_idx];

        nie_launcher::save::apply_team_to_autosave(&mut blob.body, &lineup)
            .map_err(|e| anyhow::anyhow!("injecting the team into the blob: {e}"))?;

        let target_out = out.unwrap_or(save_path);
        nie_save::io::write_save(&container, target_out)
            .map_err(|e| anyhow::anyhow!("writing the injected save: {e}"))?;

        println!(
            "inject team ok='{}' characters={} out={}",
            lineup.team_name,
            lineup.characters.len(),
            target_out.display()
        );
        Ok(())
    }

    pub(crate) fn launcher_cmd(op: LauncherOp) -> anyhow::Result<()> {
        match op {
            LauncherOp::Team { cmd } => match cmd {
                LauncherTeamOp::Decrypt {
                    file,
                    passphrase,
                    out,
                } => {
                    let content = std::fs::read_to_string(&file)
                        .with_context(|| format!("reading {}", file.display()))?;
                    let envelope: nie_launcher::TeamExportEnvelope = serde_json::from_str(&content)
                        .context("deserializing the encrypted envelope")?;
                    let lineup = nie_launcher::decrypt_team_envelope(&envelope, &passphrase)
                        .map_err(|e| anyhow::anyhow!("team decryption: {e}"))?;
                    let json = serde_json::to_string_pretty(&lineup)?;
                    if let Some(out_path) = out {
                        std::fs::write(&out_path, &json)
                            .with_context(|| format!("writing {}", out_path.display()))?;
                        println!(
                            "decrypt team ok={} team='{}' chars={}",
                            out_path.display(),
                            lineup.team_name,
                            lineup.characters.len()
                        );
                    } else {
                        println!("{json}");
                    }
                }
                LauncherTeamOp::Encrypt {
                    file,
                    passphrase,
                    out,
                } => {
                    let content = std::fs::read_to_string(&file)
                        .with_context(|| format!("reading {}", file.display()))?;
                    let lineup: nie_launcher::TeamLineup = serde_json::from_str(&content)
                        .context("deserializing the team composition")?;
                    let mut salt = [0u8; 16];
                    let mut iv = [0u8; 12];
                    for (i, b) in salt.iter_mut().enumerate() {
                        *b = ((i as u8).wrapping_mul(31)).wrapping_add(0x42);
                    }
                    for (i, b) in iv.iter_mut().enumerate() {
                        *b = ((i as u8).wrapping_mul(17)).wrapping_add(0x13);
                    }
                    let envelope =
                        nie_launcher::encrypt_team_envelope(&lineup, &passphrase, &salt, &iv)
                            .map_err(|e| anyhow::anyhow!("team encryption: {e}"))?;
                    let json = serde_json::to_string_pretty(&envelope)?;
                    if let Some(out_path) = out {
                        std::fs::write(&out_path, &json)
                            .with_context(|| format!("writing {}", out_path.display()))?;
                        println!(
                            "encrypt team ok={} team='{}'",
                            out_path.display(),
                            lineup.team_name
                        );
                    } else {
                        println!("{json}");
                    }
                }
            },
            LauncherOp::Package { cmd } => match cmd {
                LauncherPackageOp::Info { file } => {
                    let bytes = std::fs::read(&file)
                        .with_context(|| format!("reading package {}", file.display()))?;
                    let header = nie_launcher::inspect_package_header(&bytes)
                        .map_err(|e| anyhow::anyhow!("inspecting the package container: {e}"))?;
                    let json = serde_json::to_string_pretty(&header)?;
                    println!("{json}");
                }
            },
            LauncherOp::Save { cmd } => match cmd {
                LauncherSaveOp::Park {
                    live_save,
                    mod_save,
                } => {
                    launcher_save_park(&live_save, &mod_save)?;
                }
                LauncherSaveOp::Restore {
                    live_save,
                    mod_save,
                } => {
                    launcher_save_restore(&live_save, &mod_save)?;
                }
                LauncherSaveOp::InjectTeam { save, team, out } => {
                    launcher_save_inject_team(&save, &team, out.as_deref())?;
                }
            },
            LauncherOp::Spirit { cmd } => match cmd {
                LauncherSpiritOp::Cards { query, json } => {
                    let cards: Vec<&nie_launcher::SpiritCard> = if let Some(q) = query {
                        nie_launcher::search_spirit_cards(&q)
                    } else {
                        nie_launcher::all_spirit_cards().iter().collect()
                    };
                    if json {
                        println!("{}", serde_json::to_string_pretty(&cards)?);
                    } else {
                        println!("Spirits ({}):", cards.len());
                        for c in cards {
                            println!("  0x{} | {:<30} | {}", c.hex_id, c.name, c.variant);
                        }
                    }
                }
                LauncherSpiritOp::Moves {
                    query,
                    category,
                    json,
                } => {
                    let mut moves: Vec<&nie_launcher::SpecialMove> = if let Some(q) = query {
                        nie_launcher::search_special_moves(&q)
                    } else {
                        nie_launcher::all_special_moves().iter().collect()
                    };
                    if let Some(cat) = category {
                        moves.retain(|m| m.category.eq_ignore_ascii_case(&cat));
                    }
                    if json {
                        println!("{}", serde_json::to_string_pretty(&moves)?);
                    } else {
                        println!("Special techniques ({}):", moves.len());
                        for m in moves {
                            println!("  0x{} | {:<35} | {}", m.hex_id, m.name, m.category);
                        }
                    }
                }
            },
            LauncherOp::Ut { cmd } => launcher_ut_cmd(cmd)?,
        }
        Ok(())
    }

    fn launcher_ut_cmd(op: LauncherUtOp) -> anyhow::Result<()> {
        match op {
            LauncherUtOp::Packs { json } => {
                let db = nie_launcher::UtDatabase::open_default().ok();
                let packs = if let Some(ref d) = db {
                    d.get_packs().unwrap_or_default()
                } else {
                    vec![]
                };

                if json {
                    println!("{}", serde_json::to_string_pretty(&packs)?);
                } else {
                    println!(
                        "{:<18} | {:<22} | {:>7} | {:>6} | {:>6} | {:>6} | {:>6} | {:>6}",
                        "ID", "Nom", "Prix", "Comun", "Raro", "Legend", "Icono", "Basara"
                    );
                    println!(
                        "{:-<18}-+-{:-<22}-+-{:-<7}-+-{:-<6}-+-{:-<6}-+-{:-<6}-+-{:-<6}-+-{:-<6}",
                        "", "", "", "", "", "", "", ""
                    );
                    for p in &packs {
                        println!(
                            "{:<18} | {:<22} | {:>7} | {:>5.1}% | {:>5.1}% | {:>5.1}% | {:>5.2}% | {:>5.3}%",
                            p.id,
                            p.name,
                            p.price,
                            p.prob_common * 100.0,
                            p.prob_rare * 100.0,
                            p.prob_legendary * 100.0,
                            p.prob_icon * 100.0,
                            p.prob_basara * 100.0,
                        );
                    }
                }
            }
            LauncherUtOp::Open { pack, seed, json } => {
                let db = nie_launcher::UtDatabase::open_default()
                    .context("opening data/ievr-ut.sqlite")?;
                let packs = db.get_packs()?;
                let pack_def = packs
                    .into_iter()
                    .find(|p| p.id.eq_ignore_ascii_case(&pack))
                    .ok_or_else(|| anyhow::anyhow!("Unknown pack: {pack}"))?;
                let players = db.get_players()?;
                let result = nie_launcher::open_pack_with_optional_seed(&pack_def, &players, seed);

                if json {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!(
                        "PACK OPENING: {} ({} cards, {} coins)",
                        result.pack_name,
                        result.cards.len(),
                        pack_def.price
                    );
                    println!("{:-<60}", "");
                    for (i, c) in result.cards.iter().enumerate() {
                        println!(
                            "  Card #{}: [{:<18}] {:<24} ({} | {}) -> {:>6} coins",
                            i + 1,
                            c.rolled_rarity,
                            c.player.name,
                            c.player.element,
                            c.player.position,
                            c.quicksell_coins,
                        );
                    }
                    println!("{:-<60}", "");
                    println!(
                        "TOTAL QUICK-SELL VALUE: {} coins",
                        result.total_quicksell_value
                    );
                }
            }
            LauncherUtOp::Players {
                query,
                element,
                rarity,
                limit,
                json,
            } => {
                let db = nie_launcher::UtDatabase::open_default()
                    .context("opening data/ievr-ut.sqlite")?;
                let players = db.search_players(
                    query.as_deref(),
                    element.as_deref(),
                    rarity.as_deref(),
                    limit,
                )?;

                if json {
                    println!("{}", serde_json::to_string_pretty(&players)?);
                } else {
                    println!(
                        "{:<10} | {:<26} | {:<8} | {:<4} | {:<16} | {:<12}",
                        "CRC32", "Name", "Element", "Pos", "Team", "Rarity"
                    );
                    println!(
                        "{:-<10}-+-{:-<26}-+-{:-<8}-+-{:-<4}-+-{:-<16}-+-{:-<12}",
                        "", "", "", "", "", ""
                    );
                    for p in &players {
                        println!(
                            "{:<10} | {:<26} | {:<8} | {:<4} | {:<16} | {:<12}",
                            p.id, p.name, p.element, p.position, p.team_id, p.rarity
                        );
                    }
                    println!("Total: {} players shown", players.len());
                }
            }
            LauncherUtOp::Formation { name, json } => {
                let layout = nie_launcher::formation_layout(&name);
                if json {
                    println!("{}", serde_json::to_string_pretty(&layout)?);
                } else {
                    println!(
                        "FORMATION: {} ({} field slots)",
                        layout.name,
                        layout.slots.len()
                    );
                    println!(
                        "{:<8} | {:<6} | {:>6} | {:>6}",
                        "Key", "Poste", "X (%)", "Y (%)"
                    );
                    println!("{:-<8}-+-{:-<6}-+-{:-<6}-+-{:-<6}", "", "", "", "");
                    for s in &layout.slots {
                        println!(
                            "{:<8} | {:<6} | {:>5.1}% | {:>5.1}%",
                            s.key, s.label, s.x, s.y
                        );
                    }
                    println!("\nBreakdown by field line:");
                    for (bande, slots) in layout.group_by_pitch_third() {
                        let keys: Vec<_> = slots
                            .iter()
                            .map(|s| format!("{}({:.0}%)", s.label, s.x))
                            .collect();
                        println!("  {:<15}: {}", bande, keys.join(", "));
                    }
                }
            }
            LauncherUtOp::Value {
                file,
                passphrase,
                json,
            } => {
                let content = std::fs::read_to_string(&file)
                    .with_context(|| format!("reading {}", file.display()))?;

                let lineup = if let Ok(envelope) =
                    serde_json::from_str::<nie_launcher::TeamExportEnvelope>(&content)
                {
                    nie_launcher::decrypt_team_envelope(&envelope, &passphrase)
                        .map_err(|e| anyhow::anyhow!("team decryption: {e}"))?
                } else {
                    serde_json::from_str::<nie_launcher::TeamLineup>(&content)
                        .context("deserializing the team composition")?
                };

                let db = nie_launcher::UtDatabase::open_default().ok();
                let valuation = nie_launcher::calculate_squad_valuation(&lineup, db.as_ref());

                if json {
                    println!("{}", serde_json::to_string_pretty(&valuation)?);
                } else {
                    println!("SQUAD EVALUATION: '{}'", valuation.team_name);
                    println!("  Number of players: {}", valuation.total_players);
                    println!(
                        "  Quick-sell market value: {} coins",
                        valuation.total_quicksell_value
                    );
                    println!("  Rarity distribution:");
                    for (rar, count) in &valuation.rarity_counts {
                        println!("    - {:<18}: {} cards", rar, count);
                    }
                }
            }
        }
        Ok(())
    }

    /// Build an NDJSON CRC32-to-path manifest for all .g4md and .g4mg files of the VFS.
    /// Each JSON line: `{"crc":3735928559,"path":"chr/c01000010.g4md","cpk":"abc123.cpk"}`
    ///
    /// The CRC32 uses the game's algorithm (accumulator without final inversion, over the full file name without extension, lowercase, without path).
    pub(crate) fn uniform_map(
        game_dir: &std::path::Path,
        out: &std::path::Path,
    ) -> anyhow::Result<()> {
        use nie_formats::vfs::Vfs;
        use std::io::Write as IoWrite;

        let data_dir = game_dir.join("data");
        let mut vfs = Vfs::new();
        vfs.init(&data_dir)
            .context("init VFS from cpk_list.cfg.bin")?;

        tracing::info!(
            asset_count = vfs.asset_count(),
            cpk_count = vfs.cpk_count(),
            "VFS initialized"
        );

        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let mut f = std::fs::File::create(out)
            .with_context(|| format!("creating manifest {}", out.display()))?;

        let entries = nie_explore::uniform_map::build(&vfs);
        for entry in &entries {
            writeln!(f, "{}", serde_json::to_string(entry)?)?;
        }

        eprintln!(
            "uniform-map: {} .g4md/.g4mg files indexed -> {}",
            entries.len(),
            out.display()
        );
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // MenuPredecode
    // ---------------------------------------------------------------------------

    pub(crate) fn menu_predecode_cmd(
        game_dir: &std::path::Path,
        redis_url: &str,
    ) -> anyhow::Result<()> {
        // dump_root = game_dir/data (les sprite logicalPaths commencent par "dx11/...")
        let dump_root = game_dir.join("data");
        let packs_dir = game_dir.join("data").join("packs");

        let stats = menu_predecode::run(&dump_root, &packs_dir, redis_url, &[], true)?;

        println!(
            "decoded={} skipped={} failed={}",
            stats.decoded, stats.skipped, stats.failed
        );
        Ok(())
    }
}
#[cfg(feature = "host")]
use host_launcher_and_maps::{
    launcher_cmd, launcher_save_inject_team, launcher_save_park, launcher_save_restore,
    menu_predecode_cmd, uniform_map,
};

// --- nie vfs: CPK (VFS) explorer ---

// ---------------------------------------------------------------------------
// nie convert: a game asset to an interchange format
// ---------------------------------------------------------------------------

/// Read the source: disk file if present, otherwise VFS entry.
fn lire_source(src: &str, game_dir: Option<PathBuf>) -> anyhow::Result<Vec<u8>> {
    let p = std::path::Path::new(src);
    if p.is_file() {
        return std::fs::read(p).with_context(|| format!("reading `{src}`"));
    }
    let vfs = open_vfs(game_dir)?;
    vfs.read(src)
        .with_context(|| format!("`{src}` not found, neither on disk nor in the VFS"))
}

/// Load a comparison reference: local path, or `http(s)://` URL fetched by `reqwest`.
#[cfg(feature = "host")]
fn lire_reference(reference: &str) -> anyhow::Result<Vec<u8>> {
    if !reference.starts_with("http://") && !reference.starts_with("https://") {
        return std::fs::read(reference)
            .with_context(|| format!("reading reference `{reference}`"));
    }
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("initializing the tokio runtime for lire_reference")?;
    rt.block_on(async {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .context("building the reqwest client")?;
        let resp = client
            .get(reference)
            .send()
            .await
            .with_context(|| format!("HTTP download `{reference}`"))?
            .error_for_status()
            .with_context(|| format!("HTTP status failure for `{reference}`"))?;
        let bytes = resp
            .bytes()
            .await
            .with_context(|| format!("reading the body of `{reference}`"))?;
        Ok(bytes.to_vec())
    })
}

#[cfg(not(feature = "host"))]
fn lire_reference(reference: &str) -> anyhow::Result<Vec<u8>> {
    if !reference.starts_with("http://") && !reference.starts_with("https://") {
        return std::fs::read(reference)
            .with_context(|| format!("reading reference `{reference}`"));
    }
    anyhow::bail!("reqwest HTTP support is unavailable without the host feature for `{reference}`")
}

/// Describe the install: binary, VFS, dump corpus.
///
/// The binary sha256 tells which game version is in place.
fn info_cmd(game_dir: Option<PathBuf>, json: bool) -> anyhow::Result<()> {
    use sha2::{Digest, Sha256};

    let racine = racine_jeu(game_dir);
    if nie_formats::game_source::is_remote(&racine.to_string_lossy()) {
        let vfs = open_vfs(Some(racine.clone()))?;
        let (calls, bytes) = vfs.remote_stats().unwrap_or_default();
        if json {
            println!(
                "{}",
                serde_json::json!({"racine": racine.to_string_lossy(), "binaire": null, "vfs": {"montage": "remote-packs", "cpk_list": true, "entrees": vfs.asset_count(), "paquets": vfs.cpk_count()}, "transport": {"io_calls": calls, "payload_bytes": bytes}})
            );
        } else {
            println!(
                "Remote VFS: {}\n{} entries, {} CPKs; {bytes} payload bytes received",
                racine.display(),
                vfs.asset_count(),
                vfs.cpk_count()
            );
        }
        return Ok(());
    }
    let binary_dir = game_binary_dir(&racine);
    let exe = binary_dir.join("nie.exe");

    let (taille_exe, sha) = match std::fs::read(&exe) {
        Ok(octets) => {
            let mut h = Sha256::new();
            h.update(&octets);
            (Some(octets.len() as u64), Some(hex::encode(h.finalize())))
        }
        Err(_) => (None, None),
    };

    // Launch chain of the real game. `nie.exe` alone does not start: EAC launches the binary,
    // EAC requires EOS, and the game calls Steamworks. The forge can produce `nie.exe`
    // byte for byte; it produces none of the following, which are signed third-party
    // components. Listing them avoids assuming an identical `nie.exe` is enough.
    const CHAINE: [(&str, &str); 5] = [
        ("nie.exe", "game binary, produced by the forge"),
        (
            "EACLauncher.exe",
            "Easy Anti-Cheat: launches the game, third party, not reproducible",
        ),
        ("EasyAntiCheat/Settings.json", "EAC configuration"),
        (
            "EOSSDK-Win64-Shipping.dll",
            "Epic Online Services: required by EAC, third party",
        ),
        ("steam_api64.dll", "Steamworks: third party"),
    ];
    let chaine: Vec<(&str, &str, bool, u64)> = CHAINE
        .iter()
        .map(|(f, role)| {
            let m = std::fs::metadata(binary_dir.join(f)).ok();
            (*f, *role, m.is_some(), m.map_or(0, |m| m.len()))
        })
        .collect();
    let manquants = chaine.iter().filter(|(_, _, present, _)| !present).count();

    let cpk_list = nie_formats::game_source::local_data_dir(&racine)
        .unwrap_or_else(|| racine.join("data"))
        .join("cpk_list.cfg.bin");
    let vfs = open_vfs(Some(racine.clone())).ok();
    // Two mounts serve the same paths: the CPK packs, or an already extracted dump. Naming
    // the mount avoids reading an entry count without knowing where it comes from.
    let montage = match vfs.as_ref() {
        Some(v) if v.is_dump() => "dump",
        Some(_) => "packs",
        None => "none",
    };
    let entrees = vfs.as_ref().map_or(0, |v| v.iter().count());
    let paquets: std::collections::BTreeSet<String> =
        vfs.as_ref().map_or_else(Default::default, |v| {
            v.iter()
                .filter(|(_, e)| !e.cpk_filename.is_empty())
                .map(|(_, e)| e.cpk_filename.clone())
                .collect()
        });
    // The dump corpus decides whether the goldens really run: saying so here avoids
    // finding out too late that they pass without reading anything.
    let dumps = ["dump/gamedata", "data/common/gamedata"]
        .iter()
        .map(|r| racine.join(r))
        .find(|p| p.is_dir());

    if json {
        let v = serde_json::json!({
            "racine": racine.display().to_string(),
            "binaire": { "present": taille_exe.is_some(), "taille": taille_exe, "sha256": sha },
            "vfs": { "montage": montage, "cpk_list": cpk_list.is_file(), "entrees": entrees, "paquets": paquets.len() },
            "dumps_gamedata": dumps.as_ref().map(|p| p.display().to_string()),
            "chaine_de_lancement": chaine.iter().map(|(f, role, present, taille)| serde_json::json!({
                "fichier": f, "role": role, "present": present, "taille": taille,
            })).collect::<Vec<_>>(),
            "lancable": manquants == 0,
        });
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }

    println!("root        {}", racine.display());
    match (taille_exe, &sha) {
        (Some(t), Some(s)) => {
            println!("binary      nie.exe ({t} bytes)");
            println!("sha256      {s}");
        }
        _ => println!("binary      missing ({})", exe.display()),
    }
    println!(
        "cpk_list    {}",
        if cpk_list.is_file() {
            "present"
        } else {
            "missing"
        }
    );
    println!(
        "vfs         {montage}: {entrees} entries, {} packs",
        paquets.len()
    );
    match &dumps {
        Some(p) => println!("dumps       {}", p.display()),
        None => {
            println!("dumps       missing: the corpus-backed goldens will not run")
        }
    }

    println!(
        "
launch chain"
    );
    for (f, role, present, taille) in &chaine {
        let etat = if *present {
            format!("{taille:>10} B")
        } else {
            "   MISSING".to_string()
        };
        println!("  {etat}  {f:<28} {role}");
    }
    if manquants == 0 {
        println!(
            "
launchable  yes: all 5 components are present"
        );
    } else {
        println!(
            "
launchable  NO: {manquants} component(s) missing"
        );
    }
    Ok(())
}

/// Reports only locale tags and resource paths that are present in the mounted VFS.
///
/// The shared classifier derives tags from `common/text`, then sees whether the same exact tag
/// appears in menu textures, font resources, and Criware containers. It deliberately does not
/// turn an `<LG>` template into a path unless the resulting resource is indexed by the VFS.
#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per `locales` flag, passed straight from the parsed command line"
)]
fn locales_cmd(
    game_dir: Option<PathBuf>,
    samples: usize,
    json: bool,
    export: Option<&std::path::Path>,
    langs: &[String],
    locales_out: Option<&std::path::Path>,
    seed_locales: Option<&std::path::Path>,
    categories: &[String],
) -> anyhow::Result<()> {
    let vfs = open_vfs(game_dir)?;
    if export.is_some() || locales_out.is_some() {
        return export_vfs_locales(
            &vfs,
            export,
            langs,
            locales_out,
            seed_locales,
            categories,
            json,
        );
    }

    let report =
        nie_formats::locale::discover_locale_assets(vfs.iter().map(|(path, _)| path), samples);

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    if report.locales.is_empty() {
        println!("locales  no language proven by data/common/text in this VFS");
        return Ok(());
    }
    for locale in report.locales {
        println!("{}", locale.locale);
        for (kind, count) in locale.kinds {
            println!("  {kind:<8} {count}");
            if let Some(paths) = locale.samples.get(&kind) {
                for path in paths {
                    println!("    {path}");
                }
            }
        }
    }
    Ok(())
}

fn export_vfs_locales(
    vfs: &nie_formats::vfs::Vfs,
    export: Option<&std::path::Path>,
    langs: &[String],
    locales_out: Option<&std::path::Path>,
    seed_locales: Option<&std::path::Path>,
    categories: &[String],
    json_out: bool,
) -> anyhow::Result<()> {
    use std::collections::{BTreeMap, BTreeSet};

    const ALL_VFS_LANGS: &[&str] = &[
        "ja", "en", "fr", "es", "de", "it", "pt", "zh_hans", "zh_hant",
    ];
    // Every shipped language except the Japanese source text.
    const TRANSLATED_LANGS: &[&str] = &["en", "fr", "es", "de", "it", "pt", "zh_hans", "zh_hant"];
    const CORE_CATEGORIES: &[&str] = &[
        "chara",
        "chara_add",
        "skill",
        "team",
        "belong_team",
        "item",
        "map",
        "mission",
        "quest_title",
        "soccer_common",
        "soccer_technic",
        "soccer_suggest",
        "soccer_passive",
        "system",
        "menu",
        "setting",
        "help",
        "playstyle",
        "aura",
        "special_tactics",
        "super_tactics",
        "dictionary",
        "formation",
        "shop",
        "post",
        "craft",
        "data_file",
        "medal",
        "search",
        "trophy",
        "constellation",
    ];

    let target_langs: Vec<String> = if langs.is_empty() {
        vec![
            "ja".to_string(),
            "en".to_string(),
            "fr".to_string(),
            "es".to_string(),
        ]
    } else if langs.iter().any(|l| l == "all") {
        ALL_VFS_LANGS.iter().map(|s| (*s).to_string()).collect()
    } else {
        langs.to_vec()
    };

    let export_all_cats = categories.iter().any(|c| c == "all");
    let export_cat_filter: BTreeSet<&str> = if categories.is_empty() {
        CORE_CATEGORIES.iter().copied().collect()
    } else {
        categories.iter().map(String::as_str).collect()
    };

    let mut per_lang_all_hashes: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for lang in &target_langs {
        per_lang_all_hashes.insert(lang.clone(), BTreeMap::new());
    }

    #[derive(serde::Serialize)]
    struct HashEntry {
        category: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        ja_raw: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        romaji: Option<String>,
        #[serde(flatten)]
        texts: BTreeMap<String, String>,
    }

    #[derive(serde::Serialize)]
    struct JaTranslation {
        hash_id: String,
        category: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        romaji: Option<String>,
        #[serde(flatten)]
        texts: BTreeMap<String, String>,
    }

    let mut exported_entries: BTreeMap<String, HashEntry> = BTreeMap::new();
    let mut ja_to_translations: BTreeMap<String, JaTranslation> = BTreeMap::new();
    let mut category_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut ui_hashes: BTreeSet<String> = BTreeSet::new();

    for &(cat_key, file_stem) in nie_data::text::TEXT_FILES {
        let include_in_export = export_all_cats || export_cat_filter.contains(cat_key);
        let is_ui_cat = matches!(
            cat_key,
            "system" | "menu_setting" | "setting_menu" | "help" | "soccer" | "playstyle"
        );

        let mut lang_clean: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        let mut ja_raw_map: BTreeMap<String, String> = BTreeMap::new();

        for lang in &target_langs {
            let vfs_path = format!("data/common/text/{lang}/{file_stem}.cfg.bin");
            let Ok(bytes) = vfs.read(&vfs_path) else {
                continue;
            };
            let Some(root) = nie_formats::cfgbin::to_tagged_json(&bytes) else {
                continue;
            };
            let parsed = nie_data::text::parse_text_file(&root);
            let mut map = BTreeMap::new();
            let lang_store = per_lang_all_hashes.entry(lang.clone()).or_default();
            for (hash_id, txt) in parsed {
                let key = format!("0x{:08X}", hash_id.0);
                if is_ui_cat {
                    ui_hashes.insert(key.clone());
                }
                lang_store.insert(key.clone(), txt.clone());
                if include_in_export {
                    map.insert(key, txt);
                }
            }
            if include_in_export && lang == "ja" {
                for (hash_id, raw) in nie_data::text::parse_text_file_raw(&root) {
                    ja_raw_map.insert(format!("0x{:08X}", hash_id.0), raw);
                }
            }
            if include_in_export && !map.is_empty() {
                lang_clean.insert(lang.clone(), map);
            }
        }

        if !include_in_export {
            continue;
        }

        let mut cat_hashes: BTreeSet<String> = BTreeSet::new();
        for map in lang_clean.values() {
            cat_hashes.extend(map.keys().cloned());
        }
        category_counts.insert(cat_key.to_string(), cat_hashes.len());

        for hash_key in cat_hashes {
            let mut texts = BTreeMap::new();
            for lang in &target_langs {
                if let Some(val) = lang_clean.get(lang).and_then(|m| m.get(&hash_key)) {
                    texts.insert(lang.clone(), val.clone());
                }
            }
            let ja_raw = ja_raw_map
                .get(&hash_key)
                .filter(|r| r.contains('/'))
                .cloned();
            let romaji = ja_raw
                .as_deref()
                .and_then(nie_data::text::jp::furigana_readings_to_romaji)
                .or_else(|| {
                    texts
                        .get("ja")
                        .filter(|s| {
                            nie_data::text::jp::is_japanese(s) && !nie_data::text::jp::has_kanji(s)
                        })
                        .map(|s| nie_data::text::jp::to_romaji(s))
                });

            if let Some(ja_text) = texts.get("ja") {
                let norm_ja = ja_text.trim();
                if !norm_ja.is_empty()
                    && !norm_ja.contains('\n')
                    && norm_ja.chars().count() <= 48
                    && TRANSLATED_LANGS.iter().any(|l| texts.contains_key(*l))
                {
                    fn cat_rank(cat: &str) -> u8 {
                        match cat {
                            "skill" | "chara" | "team" | "aura" | "special_tactics"
                            | "super_tactics" => 4,
                            "map" | "quest_title" | "trophy" | "system" | "menu" => 3,
                            "item" | "help" | "post" | "dictionary" => 2,
                            _ => 1,
                        }
                    }
                    let is_real_translation = TRANSLATED_LANGS
                        .iter()
                        .any(|l| texts.get(*l).is_some_and(|v| v != norm_ja));
                    let cand_rank = cat_rank(cat_key);
                    let should_write = match ja_to_translations.get(norm_ja) {
                        None => true,
                        Some(existing) => {
                            let existing_real = TRANSLATED_LANGS
                                .iter()
                                .any(|l| existing.texts.get(*l).is_some_and(|v| v != norm_ja));
                            let existing_rank = cat_rank(&existing.category);
                            (!existing_real && is_real_translation)
                                || (is_real_translation == existing_real
                                    && cand_rank > existing_rank)
                        }
                    };
                    if should_write {
                        let mut tr_texts = BTreeMap::new();
                        for &l in TRANSLATED_LANGS {
                            if let Some(v) = texts.get(l) {
                                tr_texts.insert(l.to_string(), v.clone());
                            }
                        }
                        ja_to_translations.insert(
                            norm_ja.to_string(),
                            JaTranslation {
                                hash_id: hash_key.clone(),
                                category: cat_key.to_string(),
                                romaji: romaji.clone(),
                                texts: tr_texts,
                            },
                        );
                    }
                }
            }

            exported_entries.insert(
                hash_key,
                HashEntry {
                    category: cat_key.to_string(),
                    ja_raw,
                    romaji,
                    texts,
                },
            );
        }
    }

    // Use aho-corasick to verify key iconic Inazuma Japanese terms exist cleanly in the dictionary.
    let canonical_needles = [
        "亜風炉 照美",
        "アフロディ",
        "ゴッドノウズ",
        "イナズマイレブン",
        "円堂 守",
        "豪炎寺 修也",
        "鬼道 有人",
        "稲妻",
    ];
    let ac = aho_corasick::AhoCorasick::new(canonical_needles)?;
    let mut canonical_hits = 0usize;
    for ja_key in ja_to_translations.keys() {
        canonical_hits += ac.find_iter(ja_key).count();
    }

    let mut written_locales: BTreeMap<String, usize> = BTreeMap::new();
    if let Some(out_dir) = locales_out {
        std::fs::create_dir_all(out_dir)?;
        let mut seeded_by_lang: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        let mut seed_hex_union: BTreeSet<String> = BTreeSet::new();

        if let Some(seed_dir) = seed_locales {
            for lang in &target_langs {
                let p = seed_dir.join(format!("{lang}.json"));
                if p.is_file() {
                    let raw = std::fs::read_to_string(&p)?;
                    let map: BTreeMap<String, String> = serde_json::from_str(&raw)?;
                    for k in map.keys() {
                        if k.starts_with("0x") {
                            seed_hex_union.insert(k.clone());
                        }
                    }
                    seeded_by_lang.insert(lang.clone(), map);
                }
            }
        }

        let target_hex_keys: BTreeSet<String> = if seed_hex_union.is_empty() {
            ui_hashes.clone()
        } else {
            seed_hex_union.union(&ui_hashes).cloned().collect()
        };

        for lang in &target_langs {
            let mut merged: BTreeMap<String, String> =
                seeded_by_lang.remove(lang).unwrap_or_default();
            if let Some(vfs_hashes) = per_lang_all_hashes.get(lang) {
                for hex_key in &target_hex_keys {
                    if let Some(val) = vfs_hashes.get(hex_key) {
                        merged.insert(hex_key.clone(), val.clone());
                    }
                }
            }
            let out_path = out_dir.join(format!("{lang}.json"));
            std::fs::write(&out_path, serde_json::to_string_pretty(&merged)?)?;
            written_locales.insert(lang.clone(), merged.len());
        }
    }

    if let Some(export_path) = export {
        if let Some(parent) = export_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let payload = serde_json::json!({
            "source": "ievr-vfs",
            "langs": target_langs,
            "categories": category_counts,
            "canonical_hits": canonical_hits,
            "ja_term_count": ja_to_translations.len(),
            "hash_entry_count": exported_entries.len(),
            "ja_to_translations": ja_to_translations,
            "hash_entries": exported_entries,
        });
        std::fs::write(export_path, serde_json::to_string(&payload)?)?;
    }

    let summary = serde_json::json!({
        "langs": target_langs,
        "categories": category_counts.len(),
        "canonical_hits": canonical_hits,
        "ja_terms": ja_to_translations.len(),
        "hash_entries": exported_entries.len(),
        "written_locales": written_locales,
        "export": export.map(|p| p.display().to_string()),
    });
    if json_out {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        println!("{}", serde_json::to_string(&summary)?);
    }
    Ok(())
}

fn convert_cmd(
    src: &str,
    to: &str,
    out: Option<&std::path::Path>,
    game_dir: Option<PathBuf>,
    reference: Option<&str>,
    masque: bool,
) -> anyhow::Result<()> {
    use nie_formats::image_out::ImageOut;

    // "Sprite sheet" outputs: they render no image but the description of the
    // atlas regions, in the form the web expects.
    if matches!(to, "css" | "svg" | "json") {
        return convert_sprites(src, to, out, game_dir, masque);
    }

    let format = ImageOut::depuis_extension(to).ok_or_else(|| {
        let connus: Vec<&str> = ImageOut::TOUS.iter().map(|f| f.extension()).collect();
        anyhow::anyhow!(
            "unknown format `{to}`; supported formats: {}",
            connus.join(", ")
        )
    })?;

    let data = lire_source(src, game_dir)?;
    let produit = nie_formats::image_out::g4tx_vers(
        &data,
        nie_formats::g4tx_decode::basename_of(src),
        format,
    )
    .map_err(|e| anyhow::anyhow!("converting `{src}` to {}: {e}", format.extension()))?;

    let destination = out.map_or_else(
        || {
            let base = src.rsplit('/').next().unwrap_or(src);
            let tronc = base.rsplit_once('.').map_or(base, |(t, _)| t);
            PathBuf::from(format!("{tronc}.{}", format.extension()))
        },
        std::path::Path::to_path_buf,
    );
    std::fs::write(&destination, &produit)
        .with_context(|| format!("writing `{}`", destination.display()))?;

    println!("source      {src} ({} bytes)", data.len());
    println!(
        "format      {}{}",
        format.extension(),
        if format.sans_perte() {
            ""
        } else {
            " (lossy)"
        }
    );
    println!(
        "output      {} ({} bytes)",
        destination.display(),
        produit.len()
    );

    if let Some(reference) = reference {
        let attendu = lire_reference(reference)?;
        let empreinte = |b: &[u8]| {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(b);
            hex::encode(h.finalize())
        };
        let (a, b) = (empreinte(&produit), empreinte(&attendu));
        println!("reference   {reference} ({} bytes)", attendu.len());
        println!("sha256      produced={a}");
        println!("            expected={b}");
        if a == b {
            println!("verdict     byte-identical");
        } else {
            println!("verdict     DIFFERENT");
            anyhow::bail!("the output differs from the reference");
        }
    }
    Ok(())
}

/// Write **every** texture of a `.g4tx`, one image per texture.
///
/// `convert` without `--toutes` goes through `select_main_texture`, which keeps the texture named like the file; textures without a decodable DDS payload are reported and skipped.
fn convert_toutes(
    src: &str,
    to: &str,
    out: Option<&std::path::Path>,
    game_dir: Option<PathBuf>,
) -> anyhow::Result<()> {
    use nie_formats::image_out::{self, ImageOut};

    let format = ImageOut::depuis_extension(to).ok_or_else(|| {
        let connus: Vec<&str> = ImageOut::TOUS.iter().map(|f| f.extension()).collect();
        anyhow::anyhow!(
            "unknown format `{to}`; supported formats: {}",
            connus.join(", ")
        )
    })?;

    let data = lire_source(src, game_dir)?;
    let atlas = nie_formats::g4tx::parse(&data)
        .map_err(|e| anyhow::anyhow!("`{src}` is not a readable G4TX: {e}"))?;

    let dossier = out.map_or_else(|| PathBuf::from("."), std::path::Path::to_path_buf);
    std::fs::create_dir_all(&dossier)
        .with_context(|| format!("creating `{}`", dossier.display()))?;

    let base = src.rsplit('/').next().unwrap_or(src);
    let tronc = base.rsplit_once('.').map_or(base, |(t, _)| t);

    println!(
        "source      {src} ({} bytes, {} texture(s))",
        data.len(),
        atlas.textures.len()
    );
    let mut ecrites = 0usize;
    for tex in &atlas.textures {
        let Some((w, h, rgba)) = nie_formats::g4tx_decode::decode_texture_rgba(&data, tex) else {
            println!(
                "  - {:<32} {}x{}  NOT DECODABLE",
                tex.name, tex.width, tex.height
            );
            continue;
        };
        let produit = match image_out::encoder_rgba(&rgba, w, h, format) {
            Ok(p) => p,
            Err(e) => {
                println!("  - {:<32} {w}x{h}  ENCODING FAILED: {e}", tex.name);
                continue;
            }
        };
        // The texture name comes from the file and may carry separators. It is reduced to
        // what makes a safe file name rather than writing outside the requested folder.
        let nom: String = tex
            .name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let cible = dossier.join(format!("{tronc}__{nom}.{}", format.extension()));
        std::fs::write(&cible, &produit)
            .with_context(|| format!("writing `{}`", cible.display()))?;
        println!(
            "  - {:<32} {w}x{h}  {} regions  -> {} ({} bytes)",
            tex.name,
            tex.sub_textures.len(),
            cible.display(),
            produit.len()
        );
        ecrites += 1;
    }
    println!("written     {ecrites}/{}", atlas.textures.len());
    Ok(())
}

/// Convert a `.g4tx` atlas to a web sprite sheet (`css`, `svg` or `json`).
///
/// The `.g4tx` describes its own regions (name + rectangle), which are copied, not guessed. `css` writes **two** files (the sheet and the atlas as lossless WebP); `svg` and `json` write one, the SVG embedding the atlas.
fn convert_sprites(
    src: &str,
    to: &str,
    out: Option<&std::path::Path>,
    game_dir: Option<PathBuf>,
    masque: bool,
) -> anyhow::Result<()> {
    use nie_formats::image_out::ImageOut;
    use nie_formats::sprite_sheet;

    let data = lire_source(src, game_dir)?;
    let atlas = nie_formats::g4tx::parse(&data)
        .map_err(|e| anyhow::anyhow!("`{src}` is not a readable G4TX: {e}"))?;
    let feuille = sprite_sheet::depuis_g4tx(&atlas, 0)
        .ok_or_else(|| anyhow::anyhow!("`{src}` contains no texture"))?;

    let base = src.rsplit('/').next().unwrap_or(src);
    let tronc = base.rsplit_once('.').map_or(base, |(t, _)| t).to_string();
    let destination = out.map_or_else(
        || PathBuf::from(format!("{tronc}.{to}")),
        std::path::Path::to_path_buf,
    );

    println!("source      {src} ({} bytes)", data.len());
    println!(
        "atlas       {}: {}x{}",
        feuille.nom, feuille.largeur, feuille.hauteur
    );
    println!("regions     {}", feuille.len());
    if feuille.is_empty() {
        println!("note        plain image (no atlas region declared)");
    }

    let contenu = match to {
        "json" => feuille.vers_json(),
        "css" => {
            // L'atlas accompagne la feuille : WebP sans perte, le plus petit des formats exacts.
            let image = nie_formats::image_out::g4tx_vers(&data, &tronc, ImageOut::Webp)
                .map_err(|e| anyhow::anyhow!("encoding the atlas: {e}"))?;
            let image_path = destination.with_extension("webp");
            std::fs::write(&image_path, &image)
                .with_context(|| format!("writing `{}`", image_path.display()))?;
            let nom_image = image_path.file_name().map_or_else(
                || tronc.clone() + ".webp",
                |n| n.to_string_lossy().into_owned(),
            );
            println!(
                "atlas       {} ({} bytes)",
                image_path.display(),
                image.len()
            );
            let mode = if masque {
                sprite_sheet::ModeCss::Masque
            } else {
                sprite_sheet::ModeCss::Image
            };
            feuille.vers_css_mode(&nom_image, mode)
        }
        "svg" => {
            let image = nie_formats::image_out::g4tx_vers(&data, &tronc, ImageOut::Png)
                .map_err(|e| anyhow::anyhow!("encoding the atlas: {e}"))?;
            feuille.vers_svg(&sprite_sheet::data_uri("image/png", &image))
        }
        _ => unreachable!("format filtered by the caller"),
    };

    std::fs::write(&destination, contenu.as_bytes())
        .with_context(|| format!("writing `{}`", destination.display()))?;
    println!(
        "output      {} ({} bytes)",
        destination.display(),
        contenu.len()
    );
    Ok(())
}

#[cfg(feature = "host")]
fn viola_cmd(op: ViolaOp) -> anyhow::Result<()> {
    match op {
        ViolaOp::Dump {
            out,
            filtre,
            preset,
            sans_reprise,
            tout_reecrire,
            threads,
            sans_extra,
            sans_journal,
            index,
            sans_verification,
            game_dir,
        } => {
            // Un preset nomme se resout en specification de filtre ; un nom inconnu est une
            // erreur, jamais un dump silencieux du jeu entier.
            let filtre = match (filtre, preset) {
                (Some(_), Some(_)) => anyhow::bail!("--filtre and --preset are mutually exclusive"),
                (f, None) => f,
                (None, Some(p)) => Some(nie_viola::presets::resoudre(&p).ok_or_else(|| {
                    anyhow::anyhow!(
                        "unknown preset `{p}`; presets: {}",
                        nie_viola::presets::NOMS.join(", ")
                    )
                })?),
            };
            let vfs = open_vfs(game_dir)?;
            let options = nie_viola::DumpOptions {
                filtre,
                reprise: !sans_reprise,
                sauter_identiques: !tout_reecrire,
                threads,
                inclure_extra: !sans_extra,
                verifier_taille: !sans_verification,
                journal: !sans_journal,
                index_contenu: index,
                controler_casse: true,
            };
            let annuler = std::sync::atomic::AtomicBool::new(false);
            // The progress report writes a single line rewritten in place: called from
            // several threads, it must stay cheap.
            let progres = |p: nie_viola::DumpProgress| {
                eprint!(
                    "\r  {} / {} files, {:.1} GiB",
                    p.faits,
                    p.total,
                    p.octets as f64 / 1.073_741_824e9
                );
                let _ = std::io::stderr().flush();
            };
            let r = nie_viola::dump_all(&vfs, &out, &options, &annuler, &progres)
                .map_err(anyhow::Error::msg)?;
            eprintln!();
            println!("planned   {}", r.total);
            println!("extracted {}", r.extraits);
            println!("skipped   {}", r.sautes);
            println!("failures  {}", r.echecs);
            println!(
                "bytes     {} ({:.2} GiB)",
                r.octets,
                r.octets as f64 / 1.073_741_824e9
            );
            println!("packs resumed {}", r.packs_repris);
            // What `Vfs::iter` alone did not see: counting it makes the coverage gain
            // verifiable instead of assumed.
            if r.depuis_extra > 0 {
                println!(
                    "outside cpk_list {} (packs absent from the main index)",
                    r.depuis_extra
                );
            }
            // On NTFS the second of two homograph paths overwrites the first without error:
            // the output then holds fewer files than announced, and nothing said so.
            if r.collisions_casse > 0 {
                println!(
                    "case collisions {}: paths overwritten on NTFS, details in {}",
                    r.collisions_casse,
                    nie_viola::dump::chemin_journal(&out).display()
                );
            }
            // A total of failures cannot be fixed; a breakdown can.
            for (raison, n) in r.echecs_par_raison() {
                println!("  {:<20} {n}", raison.nom());
            }
            if r.echecs > 0 && !sans_journal {
                println!(
                    "log       {}",
                    nie_viola::dump::chemin_journal(&out).display()
                );
            }
            if index {
                println!(
                    "index     {}",
                    nie_viola::dump::chemin_index(&out).display()
                );
            }
            if r.annule {
                println!("cancelled yes");
            }
            Ok(())
        }
        ViolaOp::Verify {
            dir,
            filtre,
            echantillon,
            intrus,
            limite,
            sans_rapport,
            threads,
            game_dir,
        } => {
            let vfs = open_vfs(game_dir)?;
            let options = nie_viola::VerifOptions {
                filtre,
                inclure_extra: true,
                echantillon,
                threads,
            };
            let r = nie_viola::verifier(&vfs, &dir, &options).map_err(anyhow::Error::msg)?;
            println!("expected  {}", r.attendus);
            println!("matching  {} ({:.3} %)", r.conformes, r.couverture());
            println!("missing   {}", r.manquants);
            println!("size mismatches {}", r.tailles_divergentes);
            println!(
                "bytes     {} ({:.2} GiB)",
                r.octets,
                r.octets as f64 / 1.073_741_824e9
            );
            // A correct size does not prove correct content: a decryption with the wrong key
            // returns exactly the right number of bytes. The sample is what detects it.
            if r.compares > 0 {
                println!(
                    "contents compared {}; differing {}",
                    r.compares, r.contenus_divergents
                );
            }
            if r.illisibles > 0 {
                println!("unreadable {}", r.illisibles);
            }
            for c in r.constats.iter().take(limite) {
                println!(
                    "  {:<18} {} (expected {}, found {})",
                    c.anomalie.nom(),
                    c.chemin,
                    c.attendu,
                    c.trouve
                );
            }
            if r.constats.len() > limite {
                println!("  ... {} more", r.constats.len() - limite);
            }
            if intrus {
                let liste = nie_viola::verify::intrus(&vfs, &dir).map_err(anyhow::Error::msg)?;
                println!("not in index {}", liste.len());
                for c in liste.iter().take(limite) {
                    println!("  extra   {c}");
                }
            }
            if !sans_rapport {
                nie_viola::verify::ecrire_rapport(&dir, &r)?;
                println!(
                    "report    {}",
                    nie_viola::verify::chemin_rapport(&dir).display()
                );
            }
            println!(
                "verdict   {}",
                if r.conforme() {
                    "conforme"
                } else {
                    "NOT CONFORMING"
                }
            );
            Ok(())
        }
        ViolaOp::Pack {
            mod_dir,
            out,
            cpk_list,
            switch,
            game_dir,
        } => {
            let plateforme = if switch {
                nie_viola::Platform::Switch
            } else {
                nie_viola::Platform::Pc
            };
            let cpk_list = cpk_list
                .unwrap_or_else(|| racine_jeu(game_dir).join("data").join("cpk_list.cfg.bin"));
            let r = nie_viola::pack_mod(&cpk_list, &mod_dir, &out, plateforme)
                .map_err(anyhow::Error::msg)?;
            println!("updated    {}", r.mis_a_jour);
            println!("added      {}", r.ajoutes);
            println!("copied     {}", r.copies);
            println!("entries    {}", r.total);
            println!("envelope  {:?}", r.crypto);
            // A `cpk_list` already packed would stack the entries of a previous mod: say so.
            if r.loose_avant > 64 {
                eprintln!(
                    "warning: {} entries were already out of the packs; this cpk_list looks already packed",
                    r.loose_avant
                );
            }
            Ok(())
        }
        ViolaOp::Merge {
            sources,
            out,
            fichier,
            game_dir,
        } => {
            // La fusion au champ a besoin du vanilla ; le VFS le fournit sans exiger un dump.
            let vfs = if fichier {
                None
            } else {
                Some(open_vfs(game_dir)?)
            };
            let rapport = match &vfs {
                None => nie_viola::merge_dirs(&sources, &out, &nie_viola::MergeStrategy::Fichier),
                Some(vfs) => {
                    let resoudre = |rel: &str| vfs.read(rel).ok();
                    nie_viola::merge_dirs(
                        &sources,
                        &out,
                        &nie_viola::MergeStrategy::Semantique(&resoudre),
                    )
                }
            }
            .map_err(anyhow::Error::msg)?;
            println!("copied    {}", rapport.copies);
            println!("merged    {}", rapport.fusionnes);
            println!("conflicts {}", rapport.conflits.len());
            for c in &rapport.conflits {
                let repli = c.repli.as_deref().unwrap_or("");
                println!(
                    "  {}: mods {:?}, {} fields merged, {} in conflict {repli}",
                    c.chemin, c.rangs, c.champs_fusionnes, c.champs_en_desaccord
                );
            }
            Ok(())
        }
        ViolaOp::Crypto {
            src,
            out,
            cle,
            du_nom,
        } => {
            let cle = match (cle, du_nom) {
                (Some(hex), _) => {
                    nie_viola::CriwareKey::depuis_hex(&hex).map_err(anyhow::Error::msg)?
                }
                (None, true) => nie_viola::CriwareKey::DuNom(
                    src.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                ),
                (None, false) => nie_viola::CriwareKey::Viola,
            };
            let octets = nie_viola::crypt_file(&src, &out, &cle).map_err(anyhow::Error::msg)?;
            println!("key    {:08X}", cle.valeur());
            println!("bytes {octets}");
            Ok(())
        }
    }
}

fn vfs_cmd(op: VfsOp) -> anyhow::Result<()> {
    match op {
        VfsOp::Locate {
            game_dir,
            scan_root,
            json,
            benchmark,
        } => {
            anyhow::ensure!(benchmark <= 100_000, "--benchmark must be at most 100000");
            let started = std::time::Instant::now();
            let source = if let Some(root) = scan_root {
                nie_formats::game_source::scan(&root)?
            } else {
                nie_formats::game_source::resolve(game_dir.as_deref())?
            };
            let discovery_ms = started.elapsed().as_secs_f64() * 1000.0;
            if game_dir.is_some() {
                nie_formats::game_source::remember(&source)?;
            }
            let mut samples = Vec::with_capacity(benchmark);
            for _ in 0..benchmark {
                let before = std::time::Instant::now();
                nie_formats::game_source::resolve(None)?;
                samples.push(before.elapsed().as_secs_f64() * 1000.0);
            }
            samples.sort_by(f64::total_cmp);
            let measurement = (!samples.is_empty()).then(|| serde_json::json!({"samples": samples.len(), "p50_ms": samples[samples.len()/2], "p95_ms": samples[(samples.len()-1)*95/100], "max_ms": samples[samples.len()-1]}));
            if json {
                println!(
                    "{}",
                    serde_json::json!({"source": source, "remote": nie_formats::game_source::is_remote(&source), "discovery_ms": discovery_ms, "benchmark": measurement})
                );
            } else {
                println!("{source}");
                eprintln!("Discovery: {discovery_ms:.3} ms; benchmark: {measurement:?}");
            }
            Ok(())
        }
        #[cfg(feature = "host")]
        VfsOp::Bundle {
            screen,
            locale,
            profile,
            out,
            manifest_out,
            split_dir,
            game_dir,
        } => {
            let vfs = open_vfs(game_dir)?;
            let generated = vfs_bundle::generate(
                &vfs,
                &vfs_bundle::InitialBundleOptions {
                    screen,
                    locale,
                    profile,
                },
            )?;
            let manifest_out =
                manifest_out.unwrap_or_else(|| vfs_bundle::default_manifest_path(&out));
            let manifest = serde_json::to_vec_pretty(&generated.report)?;
            vfs_bundle::write_atomic(&out, &generated.bytes)?;
            if let Err(error) = vfs_bundle::write_atomic(&manifest_out, &manifest) {
                return Err(error.context(format!(
                    "bundle written to {}, but manifest publication failed",
                    out.display()
                )));
            }
            println!("bundle   {}", out.display());
            println!("manifest {}", manifest_out.display());
            println!("sha256   {}", generated.report.sha256);
            println!("entries  {}", generated.report.measurements.entries);
            println!("bytes    {}", generated.report.measurements.bundle_bytes);
            if let Some(directory) = split_dir {
                vfs_bundle::write_split_archives(&generated, &directory)?;
                println!("split    {}", directory.display());
            }
            Ok(())
        }
        VfsOp::Ls { prefix, game_dir } => vfs_ls(prefix.as_deref().unwrap_or(""), game_dir),
        VfsOp::Stat { path, game_dir } => vfs_stat(&path, game_dir),
        VfsOp::Cat {
            path,
            hex,
            len,
            png_out,
            wav_out,
            game_dir,
        } => vfs_cat(
            &path,
            hex,
            len,
            png_out.as_deref(),
            wav_out.as_deref(),
            game_dir,
        ),
        VfsOp::Extract {
            path,
            out,
            ext,
            game_dir,
        } => vfs_extract(&path, &out, ext.as_deref(), game_dir),
        VfsOp::Stats { top, game_dir } => vfs_stats(top, game_dir),
        #[cfg(feature = "host")]
        VfsOp::Lint(options) => vfs_lint::run(&options),
        VfsOp::Formats {
            parse,
            prefix,
            limit,
            json,
            game_dir,
        } => vfs_formats(parse, prefix.as_deref(), limit, json, game_dir),
        VfsOp::Find {
            query,
            ext,
            limit,
            json,
            game_dir,
        } => vfs_find(&query, ext.as_deref(), limit, json, game_dir),
        VfsOp::Chara {
            query,
            no_paths,
            element,
            position,
            json,
            limit,
            game_dir,
        } => {
            let opts = SearchOpts {
                show_paths: !no_paths,
                json,
                limit,
                game_dir,
            };
            vfs_search_chara(&query, element.as_deref(), position.as_deref(), opts)
        }
        VfsOp::Waza {
            query,
            no_paths,
            category,
            element,
            json,
            limit,
            game_dir,
        } => {
            let opts = SearchOpts {
                show_paths: !no_paths,
                json,
                limit,
                game_dir,
            };
            vfs_search_waza(&query, category.as_deref(), element.as_deref(), opts)
        }
        VfsOp::Remotes { json } => vfs_remotes(json),
        VfsOp::Cloud {
            source,
            dest,
            mode,
            transfers,
            checkers,
            zstd_level,
            max_packs,
            watch_secs,
            dry_run,
            json,
        } => vfs_cloud(
            source, &dest, &mode, transfers, checkers, zstd_level, max_packs, watch_secs, dry_run,
            json,
        ),
    }
}

/// Open the VFS from `game_dir`, or from what the machine offers when the argument is absent (game install, else extracted dump; see [`nie_formats::vfs::open_game`]).
///
/// An explicit root is served as is. `init` falls back to the dump by itself if `cpk_list.cfg.bin` is missing.
fn open_vfs(game_dir: Option<PathBuf>) -> anyhow::Result<nie_formats::vfs::Vfs> {
    let source = game_dir.as_ref().map(|path| path.to_string_lossy());
    nie_formats::vfs::open_source(source.as_deref()).map_err(Into::into)
}

fn vfs_ls(prefix: &str, game_dir: Option<PathBuf>) -> anyhow::Result<()> {
    let vfs = open_vfs(game_dir)?;
    let prefix = prefix.trim_matches('/');

    if let Some(role) = nie_explore::folder_roles::describe_folder(prefix) {
        println!("  role: {}", role.role);
        println!("  status: {}\n", role.status);
    }

    let mut dirs: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut files: Vec<(&str, &nie_formats::vfs::VfsEntry)> = Vec::new();

    for (path, entry) in vfs.iter() {
        let rest = if prefix.is_empty() {
            path
        } else if path == prefix {
            continue;
        } else if let Some(r) = path.strip_prefix(prefix).and_then(|r| r.strip_prefix('/')) {
            r
        } else {
            continue;
        };
        match rest.split_once('/') {
            Some((seg, _)) => {
                dirs.insert(seg.to_string());
            }
            None => files.push((path, entry)),
        }
    }

    for d in &dirs {
        println!("  {d}/");
    }
    files.sort_by_key(|(p, _)| *p);
    for (path, entry) in &files {
        let name = path.rsplit('/').next().unwrap_or(path);
        let cpk = if entry.cpk_filename.is_empty() {
            "<loose>"
        } else {
            entry.cpk_filename.as_str()
        };
        println!("  {:>10}  {name}  [{cpk}]", entry.file_size);
    }
    println!(
        "\n  {} subfolder(s), {} file(s)",
        dirs.len(),
        files.len()
    );
    Ok(())
}

/// An entry of `nie vfs find --json`: same compact one-line convention as [`SearchJsonEntry`] (`chara`/`waza`), but without a prior index: `find` works on any game install (VFS only).
#[derive(serde::Serialize)]
struct FindJsonEntry<'a> {
    path: &'a str,
    size: u32,
    cpk: &'a str,
}

fn vfs_find(
    query: &str,
    ext: Option<&str>,
    limit: usize,
    json: bool,
    game_dir: Option<PathBuf>,
) -> anyhow::Result<()> {
    let vfs = open_vfs(game_dir)?;
    let q = query.to_lowercase();
    let ext_dot = ext.map(|e| format!(".{}", e.trim_start_matches('.').to_lowercase()));

    let mut hits: Vec<(&str, &nie_formats::vfs::VfsEntry)> = vfs
        .iter()
        .filter(|(p, _)| p.to_lowercase().contains(&q))
        .filter(|(p, _)| {
            ext_dot
                .as_deref()
                .is_none_or(|e| p.to_lowercase().ends_with(e))
        })
        .collect();
    hits.sort_by_key(|(p, _)| *p);

    let total = hits.len();
    if json {
        let entries: Vec<FindJsonEntry> = hits
            .iter()
            .take(limit)
            .map(|(path, entry)| FindJsonEntry {
                path,
                size: entry.file_size,
                cpk: if entry.cpk_filename.is_empty() {
                    "<loose>"
                } else {
                    entry.cpk_filename.as_str()
                },
            })
            .collect();
        println!("{}", serde_json::to_string(&entries)?);
        return Ok(());
    }

    for (path, entry) in hits.iter().take(limit) {
        let cpk = if entry.cpk_filename.is_empty() {
            "<loose>"
        } else {
            entry.cpk_filename.as_str()
        };
        println!("  {:>10}  {path}  [{cpk}]", entry.file_size);
    }
    let capped = if total > limit {
        format!(" (limited to {limit})")
    } else {
        String::new()
    };
    println!("\n  {total} result(s){capped}");
    Ok(())
}

fn vfs_stat(path: &str, game_dir: Option<PathBuf>) -> anyhow::Result<()> {
    let vfs = open_vfs(game_dir)?;
    let entry = vfs
        .find(path)
        .ok_or_else(|| anyhow::anyhow!("`{path}` is not in the VFS"))?;
    println!("  path        {path}");
    println!("  size        {} bytes", entry.file_size);
    println!(
        "  cpk         {}",
        if entry.cpk_filename.is_empty() {
            "<loose>"
        } else {
            &entry.cpk_filename
        }
    );
    match vfs.read(path) {
        Ok(data) => {
            let label = nie_explore::describe_content(path, &data)
                .and_then(|lines| lines.into_iter().next())
                .unwrap_or_else(|| "format      raw / unrecognized".to_string());
            println!("  {label}");
            println!("  magic       {}", nie_explore::hex_prefix(&data, 16));
        }
        Err(e) => println!("  read        FAILED ({e})"),
    }
    Ok(())
}

fn vfs_cat(
    path: &str,
    hex: bool,
    len: usize,
    png_out: Option<&std::path::Path>,
    wav_out: Option<&std::path::Path>,
    game_dir: Option<PathBuf>,
) -> anyhow::Result<()> {
    /// Path of the streaming `.awb` associated with an `.acb` (same folder, same base).
    fn alloc_awb(base: &str) -> String {
        format!("{base}.awb")
    }

    /// Decode to WAV **in a large-stack thread**.
    ///
    /// `cridecoder` allocates its HCA synthesis tables on the stack: on the Windows main thread (1 MiB in debug) it overflows before producing a sample.
    fn decoder_wav(data: Vec<u8>) -> Option<Vec<u8>> {
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(move || nie_formats::cri_audio::decode_to_wav(&data).ok())
            .ok()?
            .join()
            .ok()?
    }

    let vfs = open_vfs(game_dir)?;
    let data = vfs
        .read(path)
        .with_context(|| format!("reading `{path}`"))?;
    println!("  {path}  ({} bytes)", data.len());

    if let Some(out) = png_out {
        match nie_formats::g4tx_decode::decode_best_to_png(
            &data,
            nie_formats::g4tx_decode::basename_of(path),
        ) {
            Some(png) => {
                std::fs::write(out, &png)?;
                println!("  PNG written -> {}", out.display());
            }
            None => println!("  PNG decoding failed (not a recognized texture)"),
        }
    }
    if let Some(out) = wav_out {
        // `decode_to_wav` dispatches by magic: ADX, HCA (IEVR key), AWB/AFS2, ACB.
        let mut wav = decoder_wav(data.clone());

        // Un `.acb` ne porte souvent que la table de cues : le son vit dans le `.awb` voisin
        // (streaming). C'est le cas de tous les `sound_asset/<lg>/*.acb` du jeu.
        if wav.is_none()
            && let Some(base) = path.strip_suffix(".acb")
        {
            let voisin = alloc_awb(base);
            if let Ok(awb) = vfs.read(&voisin) {
                println!(
                    "  ACB without data: falling back to {voisin} ({} bytes)",
                    awb.len()
                );
                wav = decoder_wav(awb);
            }
        }

        match wav {
            Some(w) => {
                std::fs::write(out, &w)?;
                println!("  WAV written -> {} ({} bytes)", out.display(), w.len());
            }
            None => println!("  audio decoding failed (neither ADX, HCA, nor a usable AWB/ACB)"),
        }
    }

    if !hex && let Some(lines) = nie_explore::describe_content(path, &data) {
        for l in lines {
            println!("  {l}");
        }
        return Ok(());
    }
    let n = data.len().min(len);
    print_hexdump(&data[..n], 0);
    if data.len() > n {
        println!(
            "  ... {} more byte(s) (--len to see more)",
            data.len() - n
        );
    }
    Ok(())
}

fn print_hexdump(data: &[u8], base: u64) {
    for (i, line) in data.chunks(16).enumerate() {
        let off = base + (i * 16) as u64;
        let mut hex = String::new();
        for j in 0..16 {
            if j < line.len() {
                hex.push_str(&format!("{:02x} ", line[j]));
            } else {
                hex.push_str("   ");
            }
        }
        let ascii: String = line
            .iter()
            .map(|&b| {
                if (0x20..0x7f).contains(&b) {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!("  0x{off:012x}  {hex} {ascii}");
    }
}

fn vfs_extract(
    path: &str,
    out: &std::path::Path,
    ext: Option<&str>,
    game_dir: Option<PathBuf>,
) -> anyhow::Result<()> {
    let vfs = open_vfs(game_dir)?;
    // Same semantics as `vfs find`: lowercase `.<ext>` suffix, which
    // covers compound extensions (`cfg.bin`).
    let ext_dot = ext.map(|e| format!(".{}", e.trim_start_matches('.').to_lowercase()));

    if ext_dot.is_none() && vfs.find(path).is_some() {
        let data = vfs.read(path)?;
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(out, &data)?;
        println!(
            "  1 file extracted ({} bytes) -> {}",
            data.len(),
            out.display()
        );
        return Ok(());
    }

    // No exact match: treat `path` as a folder prefix.
    let prefix = path.trim_end_matches('/');
    let sub_prefix = format!("{prefix}/");
    let matches: Vec<String> = vfs
        .iter()
        .filter(|(p, _)| *p == prefix || p.starts_with(&sub_prefix))
        .filter(|(p, _)| {
            ext_dot
                .as_deref()
                .is_none_or(|e| p.to_lowercase().ends_with(e))
        })
        .map(|(p, _)| p.to_string())
        .collect();
    anyhow::ensure!(
        !matches.is_empty(),
        "`{path}` is not in the VFS (neither an exact file nor a prefix){}",
        ext.map_or(String::new(), |e| format!(
            " (or no .{e} file under it)"
        ))
    );

    // One rayon task per file: the CRILAYLA decode and the disk write overlap across cores, and
    // the VFS reads each pack through one shared positioned-read handle.
    use rayon::prelude::*;
    let (ok, failed) = matches
        .par_iter()
        .map(|p| match vfs.read(p) {
            Ok(data) => {
                let rel = p.strip_prefix(prefix).unwrap_or(p).trim_start_matches('/');
                let dest = out.join(rel);
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                match std::fs::write(&dest, &data) {
                    Ok(()) => (1usize, 0usize),
                    Err(_) => (0, 1),
                }
            }
            Err(_) => (0, 1),
        })
        .reduce(|| (0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
    println!(
        "  {ok} file(s) extracted under {} ({failed} failure(s))",
        out.display()
    );
    Ok(())
}

fn vfs_stats(top: usize, game_dir: Option<PathBuf>) -> anyhow::Result<()> {
    let vfs = open_vfs(game_dir)?;
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (path, _) in vfs.iter() {
        let base = path.rsplit('/').next().unwrap_or(path);
        let ext = base
            .rsplit_once('.')
            .map(|(_, e)| e.to_lowercase())
            .unwrap_or_else(|| "<none>".to_string());
        *counts.entry(ext).or_default() += 1;
    }
    let mut v: Vec<_> = counts.into_iter().collect();
    v.sort_by_key(|(_, c)| std::cmp::Reverse(*c));

    println!(
        "  total = {} files, {} CPKs, {} extra entries, {} loose",
        vfs.asset_count(),
        vfs.cpk_count(),
        vfs.extra_count(),
        vfs.loose_count()
    );
    for (ext, c) in v.iter().take(top) {
        println!("  {c:>8}  .{ext}");
    }
    Ok(())
}

fn vfs_remotes(json: bool) -> anyhow::Result<()> {
    let raw = nie_formats::vfs_remote::rclone_list_remotes().map_err(|e| anyhow::anyhow!("{e}"))?;
    if json {
        println!("{raw}");
        return Ok(());
    }
    let remotes: Vec<serde_json::Value> = serde_json::from_str(&raw)?;
    if remotes.is_empty() {
        println!("  no rclone remote configured");
        return Ok(());
    }
    for r in remotes {
        let name = r["name"].as_str().unwrap_or("?");
        let kind = r["type"].as_str().unwrap_or("?");
        println!("  {name}: ({kind})");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn vfs_cloud(
    source: Option<PathBuf>,
    dest: &str,
    mode: &str,
    transfers: usize,
    checkers: usize,
    zstd_level: i32,
    max_packs: usize,
    watch_secs: Option<u64>,
    dry_run: bool,
    json: bool,
) -> anyhow::Result<()> {
    let resolved = match source {
        Some(p) => {
            if p.file_name().is_some_and(|n| n == "packs") {
                p.parent()
                    .map_or_else(|| p.clone(), std::path::Path::to_path_buf)
            } else if p.join("data").join("cpk_list.cfg.bin").is_file() {
                p.join("data")
            } else {
                p
            }
        }
        None => {
            let src_ref =
                nie_formats::game_source::resolve(None).map_err(|e| anyhow::anyhow!("{e}"))?;
            let p = PathBuf::from(&src_ref);
            if p.join("data").join("cpk_list.cfg.bin").is_file() {
                p.join("data")
            } else {
                p
            }
        }
    };
    let src_str = resolved.to_string_lossy();
    let raw = nie_formats::vfs_remote::rclone_export_packs_with_opts(
        &nie_formats::vfs_remote::RcloneExportOptions {
            local_data_dir: &src_str,
            dst_uri: dest,
            mode,
            dry_run,
            transfers,
            checkers,
            zstd_level,
            max_packs,
            filter: None,
            watch_secs,
        },
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    if json {
        println!("{raw}");
        return Ok(());
    }
    let report: serde_json::Value = serde_json::from_str(&raw)?;
    println!(
        "  mode={} dry_run={} dest={} packs={} logical={} copied_objects={} skipped={} transfers={} duration_ms={} sha256={}",
        report["mode"].as_str().unwrap_or(mode),
        dry_run,
        report["destination"].as_str().unwrap_or(dest),
        report["pack_count"].as_u64().unwrap_or(0),
        report["logical_count"].as_u64().unwrap_or(0),
        report["copied_objects"].as_u64().unwrap_or(0),
        report["skipped_objects"].as_u64().unwrap_or(0),
        report["transfers"].as_u64().unwrap_or(transfers as u64),
        report["duration_ms"].as_u64().unwrap_or(0),
        report["manifest_sha256"].as_str().unwrap_or(""),
    );
    Ok(())
}

/// What is known about an extension that `nie_formats::decode` does not route.
///
/// The dispatch table is not the whole workspace: Lua bytecode is read by `nie-lua` (which depends on `nie-formats`, so `decode` cannot call it), and a `.g4mg` cannot be decoded alone. These notes keep the measure from counting acquired capabilities and impossibilities of principle as the same "to do".
fn note_hors_dispatch(ext: &str) -> Option<&'static str> {
    match ext {
        "g4mg" => Some("raw vertex buffer, described by the sibling .g4md: nothing to decode alone"),
        "bin" => Some("includes Lua 5.2 bytecode (.lua.bin), loaded and run by nie-lua"),
        "webp" | "png" | "jpg" | "log" | "cfg" => Some("working file, not a game asset"),
        _ => None,
    }
}

/// Measures the share of the VFS the workspace reads, by parsing every file.
///
/// Unreadable files (missing pack, entry declared but absent on disk) are counted apart, so an install defect is not reported as an unported format.
fn vfs_formats(
    parse: bool,
    prefix: Option<&str>,
    limit: Option<usize>,
    json: bool,
    game_dir: Option<PathBuf>,
) -> anyhow::Result<()> {
    let vfs = open_vfs(game_dir)?;
    let mut chemins: Vec<&str> = vfs
        .iter()
        .map(|(p, _)| p)
        .filter(|p| prefix.is_none_or(|pref| p.starts_with(pref)))
        .collect();
    // Stable order: two runs must give the same number, and a probe must cover
    // the same sample every time.
    chemins.sort_unstable();
    if let Some(n) = limit
        && n < chemins.len()
    {
        // SPREAD sample, not the first n: the index is sorted by path, so the first n
        // all sit in one or two folders (`common/action`, `common/chr/_animal`) and
        // say nothing about the whole VFS. A regular step covers every family.
        let pas = chemins.len().div_ceil(n);
        chemins = chemins.into_iter().step_by(pas).collect();
    }
    anyhow::ensure!(!chemins.is_empty(), "no file matches");

    let mut par_format: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    let (mut reconnus, mut inconnus, mut illisibles) = (0usize, 0usize, 0usize);
    // Troisieme categorie, indispensable en mode `--parse` : un fichier dont le MAGIC est connu
    // mais que rien ne decode. Le fondre dans « inconnus » ferait passer un format identifie
    // pour un format absent — et c'est le cas de tous les `.g4mg`, qui n'ont de sens qu'avec
    // leur `.g4md` frere et ne se decodent donc pas seuls. C'est le reste a faire, chiffre.
    let mut sans_decodeur: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    let mut inconnus_par_ext: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    let mut exemples_inconnus: Vec<&str> = Vec::new();
    for chemin in &chemins {
        let Ok(octets) = vfs.read(chemin) else {
            illisibles += 1;
            continue;
        };
        let magic = match nie_formats::detect(&octets) {
            nie_formats::FileFormat::Unknown => None,
            f => Some(format!("{f:?}").to_lowercase()),
        };
        let nom = if parse {
            nie_formats::decode::decode(&octets).map(|d| d.format.to_string())
        } else {
            magic.clone()
        };
        match nom {
            Some(nom) => {
                reconnus += 1;
                *par_format.entry(nom).or_default() += 1;
            }
            None => match magic {
                Some(m) => *sans_decodeur.entry(m).or_default() += 1,
                None => {
                    inconnus += 1;
                    // Ventiler par extension : « 19 604 inconnus » ne dit pas quoi faire,
                    // « 15 876 .g4mg + 1 335 .vfxo » nomme les chantiers restants.
                    let base = chemin.rsplit('/').next().unwrap_or(chemin);
                    let ext = base
                        .rsplit_once('.')
                        .map_or_else(|| "<none>".to_string(), |(_, e)| e.to_lowercase());
                    *inconnus_par_ext.entry(ext).or_default() += 1;
                    if exemples_inconnus.len() < 10 {
                        exemples_inconnus.push(chemin);
                    }
                }
            },
        }
    }
    let n_sans_decodeur: usize = sans_decodeur.values().sum();

    let examines = chemins.len();
    let pct = |n: usize| n as f64 * 100.0 / examines as f64;
    if json {
        let v = serde_json::json!({
            "montage": if vfs.is_dump() { "dump" } else { "packs" },
            "mode": if parse { "parse" } else { "magic" },
            "examines": examines,
            "reconnus": reconnus,
            "sans_decodeur": n_sans_decodeur,
            "inconnus": inconnus,
            "illisibles": illisibles,
            "pct_reconnus": pct(reconnus),
            "par_format": par_format,
            "magic_sans_decodeur": sans_decodeur,
            "inconnus_par_extension": inconnus_par_ext,
        });
        println!("{}", serde_json::to_string(&v)?);
        return Ok(());
    }

    println!(
        "  mount {} | mode {} | {examines} files examined",
        if vfs.is_dump() { "dump" } else { "packs" },
        if parse { "full parse" } else { "magic" },
    );
    println!("  recognized {reconnus:>8}  ({:.2} %)", pct(reconnus));
    if n_sans_decodeur > 0 {
        println!(
            "  known magic, no standalone decoder: {n_sans_decodeur} ({:.2} %): {}",
            pct(n_sans_decodeur),
            sans_decodeur
                .iter()
                .map(|(f, n)| format!("{f} x{n}"))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    println!("  unknown    {inconnus:>8}  ({:.2} %)", pct(inconnus));
    if illisibles > 0 {
        println!(
            "  unreadable {illisibles:>8}  ({:.2} %): missing on disk, not a missing format",
            pct(illisibles)
        );
    }
    let mut classe: Vec<(&String, &usize)> = par_format.iter().collect();
    classe.sort_by(|a, b| b.1.cmp(a.1));
    for (format, n) in classe {
        println!("  {n:>8}  {format}");
    }
    if !inconnus_par_ext.is_empty() {
        println!("\n  unrecognized, by extension:");
        let mut v: Vec<(&String, &usize)> = inconnus_par_ext.iter().collect();
        v.sort_by(|a, b| b.1.cmp(a.1));
        for (ext, n) in v.iter().take(15) {
            match note_hors_dispatch(ext) {
                Some(note) => println!("    {n:>8}  .{ext}  {note}"),
                None => println!("    {n:>8}  .{ext}"),
            }
        }
        println!(
            "\n  `unknown` = absent from the `nie_formats::decode` dispatch table, NOT\n  \
             necessarily unreadable by the workspace: the notes above say who can already read them."
        );
    }
    if !exemples_inconnus.is_empty() {
        println!("\n  unrecognized examples:");
        for chemin in &exemples_inconnus {
            println!("    {chemin}");
        }
    }
    Ok(())
}

/// Options shared by `vfs_search_chara` and `vfs_search_waza`, independent of the per-family category filters.
struct SearchOpts {
    show_paths: bool,
    json: bool,
    limit: usize,
    game_dir: Option<PathBuf>,
}

/// Search a character in the VFS `.cfg.bin` files (name, ID or internal code) with optional element/position filters, then list its VFS files (the internal code, e.g. `c01000100`, appears in the model, texture and animation paths).
fn vfs_domain_search(
    domain: &str,
    query: &str,
    filters: &[(&str, Option<&str>)],
    show_paths: bool,
    json: bool,
    limit: usize,
    game_dir: Option<PathBuf>,
) -> anyhow::Result<()> {
    let vfs = open_vfs(game_dir)?;
    let mut request = serde_json::json!({"domain":domain, "query":query, "limit":limit});
    for (key, value) in filters {
        if let Some(value) = value {
            request[*key] = serde_json::json!(value);
        }
    }
    let mut result =
        nie_explore::domain_data::query_vfs(&vfs, &request).map_err(anyhow::Error::msg)?;
    if show_paths
        && let Some(items) = result
            .get_mut("items")
            .and_then(serde_json::Value::as_array_mut)
    {
        for item in items {
            let code = item
                .get("internal_code")
                .or_else(|| item.get("skill_id_str"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let paths: Vec<_> = if code.is_empty() {
                Vec::new()
            } else {
                vfs.iter()
                    .filter(|(path, _)| path.contains(code))
                    .map(|(path, _)| path.to_owned())
                    .collect()
            };
            item["related_paths"] = serde_json::json!(paths);
        }
    }
    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        println!("{}", serde_json::to_string_pretty(&result)?);
    }
    Ok(())
}

fn vfs_search_chara(
    query: &str,
    element: Option<&str>,
    position: Option<&str>,
    opts: SearchOpts,
) -> anyhow::Result<()> {
    let SearchOpts {
        show_paths,
        json,
        limit,
        game_dir,
    } = opts;
    vfs_domain_search(
        "characters",
        query,
        &[("element", element), ("position", position)],
        show_paths,
        json,
        limit,
        game_dir,
    )
}

/// Search a technique in the VFS `.cfg.bin` files (name, ID or internal code) with optional category/element filters, then list its VFS files (the internal code, e.g. `whs00010`, appears in the cut-in, telop and video paths).
fn vfs_search_waza(
    query: &str,
    category: Option<&str>,
    element: Option<&str>,
    opts: SearchOpts,
) -> anyhow::Result<()> {
    let SearchOpts {
        show_paths,
        json,
        limit,
        game_dir,
    } = opts;
    vfs_domain_search(
        "skills",
        query,
        &[("category", category), ("element", element)],
        show_paths,
        json,
        limit,
        game_dir,
    )
}

#[cfg(test)]
mod installation_tests {
    use super::game_binary_dir;

    #[test]
    fn binary_layout_preserves_vfs_root_and_prefers_root_executable() {
        let root = std::env::temp_dir().join(format!("nie-binary-layout-{}", std::process::id()));
        std::fs::create_dir_all(root.join("data")).unwrap();
        assert_eq!(game_binary_dir(&root), root);
        std::fs::write(root.join("data/nie.exe"), []).unwrap();
        assert_eq!(game_binary_dir(&root), root.join("data"));
        std::fs::write(root.join("nie.exe"), []).unwrap();
        assert_eq!(game_binary_dir(&root), root);
        std::fs::remove_dir_all(root).unwrap();
    }
}
