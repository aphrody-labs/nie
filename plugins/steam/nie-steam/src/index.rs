// SPDX-License-Identifier: Apache-2.0
//! Read-only index of a local Steam install, stored as SQLite (feature `index`).
//!
//! [`scan`] reads the install with the parsers of this crate: libraries (`libraryfolders.vdf`),
//! apps and their installed depots (`appmanifest_*.acf`), `depotcache` manifests, the PICS caches
//! (`appcache/appinfo.vdf`, `packageinfo.vdf`), Workshop items, compatibility tools, controller
//! configs and `userdata` file names. [`store`] writes the scan, [`inventory`] registers every
//! file of every installed game and [`hash_pending`] fills the blake3 of those files.
//!
//! Credential policy: `ssfn*`, `config/loginusers.vdf`, `config/local.vdf`, `config/config.vdf`
//! and `%LOCALAPPDATA%/Steam/local.vdf` are recorded by path, size and blake3 only and never
//! parsed. The one exception is [`account`], which reads the identifiers (account name, SteamID64)
//! of the most recent `loginusers.vdf` account. `config.vdf` is not parsed, so compatibility
//! tools come from directories, not from `CompatToolMapping`.
//!
//! Schema: every table except `files` is rebuilt by [`store`]; `files` is incremental.
//!
//! | Table | Columns |
//! | --- | --- |
//! | `meta` | `key`, `value` (`steam_root`, `scanned_at`, `appinfo_version`, `packageinfo_version`) |
//! | `libraries` | `idx`, `path`, `label`, `total_size`, `app_count` |
//! | `apps` | `appid`, `name`, `installdir`, `library`, `install_path`, `exists_on_disk`, `size_on_disk`, `buildid`, `state_flags`, `last_updated`, `acf_path` |
//! | `app_depots` | `appid`, `depot`, `manifest`, `size` (`InstalledDepots` of the ACF) |
//! | `depotcache` | `path`, `depot`, `gid`, `size` (`<depot>_<gid>.manifest`) |
//! | `pics_apps` | `appid`, `name`, `type`, `change_number`, `last_updated` (apps the client cached metadata for; close to, not proof of, ownership) |
//! | `pics_packages` | `packageid`, `change_number`, `appids` JSON, `depotids` JSON |
//! | `workshop` | `appid`, `item`, `size`, `timeupdated`, `manifest`, `path`, `exists_on_disk`, `library` |
//! | `compat_tools` | `name`, `path`, `source` (`compatibilitytools.d` or `steamapps/common/Proton*`) |
//! | `controller_configs` | `path`, `size` |
//! | `userdata` | `account_id`, `config_file` (names in `userdata/<id>/config` only) |
//! | `credential_artifacts` | `path`, `size`, `blake3`, `error` |
//! | `files` | `path`, `appid`, `size`, `mtime_ms`, `blake3`, `error`, `hashed_at` |

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rayon::prelude::*;
use rusqlite::{Connection, params};
use serde::Serialize;

use crate::SteamError;
use crate::config::{library_folders, login_users};
use crate::install::scan_steam_libraries;
use crate::vdf::{BinValue, VdfValue, parse_appinfo, parse_packageinfo, parse_vdf};

/// Files hashed and committed per transaction by [`hash_pending`].
const HASH_BATCH: usize = 500;

const SCHEMA: &str = "
PRAGMA journal_mode = WAL;
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT);
CREATE TABLE IF NOT EXISTS libraries (idx TEXT, path TEXT PRIMARY KEY, label TEXT, total_size INTEGER, app_count INTEGER);
CREATE TABLE IF NOT EXISTS apps (appid INTEGER PRIMARY KEY, name TEXT, installdir TEXT, library TEXT, install_path TEXT,
  exists_on_disk INTEGER, size_on_disk INTEGER, buildid INTEGER, state_flags INTEGER, last_updated INTEGER, acf_path TEXT);
CREATE TABLE IF NOT EXISTS app_depots (appid INTEGER, depot INTEGER, manifest TEXT, size INTEGER, PRIMARY KEY (appid, depot));
CREATE TABLE IF NOT EXISTS depotcache (path TEXT PRIMARY KEY, depot INTEGER, gid TEXT, size INTEGER);
CREATE TABLE IF NOT EXISTS pics_apps (appid INTEGER PRIMARY KEY, name TEXT, type TEXT, change_number INTEGER, last_updated INTEGER);
CREATE TABLE IF NOT EXISTS pics_packages (packageid INTEGER PRIMARY KEY, change_number INTEGER, appids TEXT, depotids TEXT);
CREATE TABLE IF NOT EXISTS workshop (appid INTEGER, item TEXT, size INTEGER, timeupdated INTEGER, manifest TEXT, path TEXT,
  exists_on_disk INTEGER, library TEXT, PRIMARY KEY (appid, item));
CREATE TABLE IF NOT EXISTS compat_tools (name TEXT, path TEXT PRIMARY KEY, source TEXT);
CREATE TABLE IF NOT EXISTS controller_configs (path TEXT PRIMARY KEY, size INTEGER);
CREATE TABLE IF NOT EXISTS userdata (account_id TEXT, config_file TEXT, PRIMARY KEY (account_id, config_file));
CREATE TABLE IF NOT EXISTS credential_artifacts (path TEXT PRIMARY KEY, size INTEGER, blake3 TEXT, error TEXT);
CREATE TABLE IF NOT EXISTS files (path TEXT PRIMARY KEY, appid INTEGER, size INTEGER, mtime_ms INTEGER,
  blake3 TEXT, error TEXT, hashed_at INTEGER);
CREATE INDEX IF NOT EXISTS files_appid ON files (appid);
CREATE INDEX IF NOT EXISTS files_pending ON files (blake3) WHERE blake3 IS NULL;
";

/// Tables rebuilt by [`store`].
const REBUILT: [&str; 11] = [
    "libraries",
    "apps",
    "app_depots",
    "depotcache",
    "pics_apps",
    "pics_packages",
    "workshop",
    "compat_tools",
    "controller_configs",
    "userdata",
    "credential_artifacts",
];

/// Everything [`scan`] finds in one Steam install.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Scan {
    /// Steam root that was scanned.
    pub steam_root: String,
    /// Entries of `libraryfolders.vdf`.
    pub libraries: Vec<Library>,
    /// Installed apps of every library.
    pub apps: Vec<App>,
    /// `depotcache/*.manifest` files.
    pub depotcache: Vec<DepotCacheEntry>,
    /// Summary of `appcache/appinfo.vdf`.
    pub appinfo: Option<PicsApps>,
    /// Summary of `appcache/packageinfo.vdf`.
    pub packageinfo: Option<PicsPackages>,
    /// Installed Workshop items.
    pub workshop: Vec<WorkshopItem>,
    /// Compatibility tools (Proton, Steam Linux Runtime).
    pub compat_tools: Vec<CompatTool>,
    /// Controller configuration files.
    pub controller_configs: Vec<FileEntry>,
    /// `userdata/<account>` summaries.
    pub userdata: Vec<UserData>,
    /// Credential artifacts: path, size and blake3 only.
    pub credential_artifacts: Vec<CredentialArtifact>,
}

/// One entry of `libraryfolders.vdf`.
#[derive(Debug, Clone, Serialize)]
pub struct Library {
    /// Entry index.
    pub index: String,
    /// Library root.
    pub path: String,
    /// User label.
    pub label: String,
    /// Disk size recorded by the client.
    pub total_size: Option<u64>,
    /// Apps recorded for this library.
    pub app_count: usize,
}

/// An installed app (`appmanifest_<appid>.acf`).
#[derive(Debug, Clone, Serialize)]
pub struct App {
    /// Library root.
    pub library: String,
    /// App id.
    pub appid: u32,
    /// Display name.
    pub name: String,
    /// Directory under `steamapps/common`.
    pub installdir: String,
    /// `steamapps/common/<installdir>`.
    pub install_path: String,
    /// The install directory exists.
    pub exists: bool,
    /// `SizeOnDisk`.
    pub size_on_disk: u64,
    /// Active build id.
    pub buildid: u64,
    /// `StateFlags`.
    pub state_flags: u64,
    /// `LastUpdated`.
    pub last_updated: u64,
    /// Installed depots with their manifest gid and size.
    pub depots: Vec<AppDepot>,
    /// Manifest file.
    pub acf_path: String,
}

/// One entry of `InstalledDepots`.
#[derive(Debug, Clone, Serialize)]
pub struct AppDepot {
    /// Depot id.
    pub depot: u32,
    /// Manifest gid.
    pub manifest: Option<String>,
    /// Depot size.
    pub size: Option<u64>,
}

/// `depotcache/<depot>_<gid>.manifest`.
#[derive(Debug, Clone, Serialize)]
pub struct DepotCacheEntry {
    /// Manifest file.
    pub path: String,
    /// Depot id as written in the file name.
    pub depot: String,
    /// Manifest gid as written in the file name.
    pub gid: String,
    /// File size.
    pub size: u64,
}

/// Summary of `appinfo.vdf`.
#[derive(Debug, Clone, Serialize)]
pub struct PicsApps {
    /// File magic.
    pub version: u32,
    /// Steam universe.
    pub universe: u32,
    /// One row per cached app.
    pub apps: Vec<PicsApp>,
}

/// One cached app of `appinfo.vdf`.
#[derive(Debug, Clone, Serialize)]
pub struct PicsApp {
    /// App id.
    pub appid: u32,
    /// PICS change number.
    pub change: u32,
    /// Unix time of the last update.
    pub last_updated: u32,
    /// `common/name`.
    pub name: Option<String>,
    /// `common/type`.
    pub kind: Option<String>,
}

/// Summary of `packageinfo.vdf`.
#[derive(Debug, Clone, Serialize)]
pub struct PicsPackages {
    /// File magic.
    pub version: u32,
    /// Steam universe.
    pub universe: u32,
    /// One row per cached package.
    pub packages: Vec<PicsPackage>,
}

/// One cached package of `packageinfo.vdf`.
#[derive(Debug, Clone, Serialize)]
pub struct PicsPackage {
    /// Package id.
    pub packageid: u32,
    /// PICS change number.
    pub change: u32,
    /// App ids of the package.
    pub appids: Vec<String>,
    /// Depot ids of the package.
    pub depotids: Vec<String>,
}

/// An installed Workshop item.
#[derive(Debug, Clone, Serialize)]
pub struct WorkshopItem {
    /// Library root.
    pub library: String,
    /// Owning app id.
    pub appid: String,
    /// Workshop item id.
    pub item: String,
    /// Item size.
    pub size: Option<u64>,
    /// Unix time of the last update.
    pub timeupdated: Option<u64>,
    /// Manifest gid.
    pub manifest: Option<String>,
    /// `steamapps/workshop/content/<appid>/<item>`.
    pub path: String,
    /// The item directory exists.
    pub exists: bool,
}

/// A compatibility tool directory.
#[derive(Debug, Clone, Serialize)]
pub struct CompatTool {
    /// Directory name.
    pub name: String,
    /// Directory path.
    pub path: String,
    /// `compatibilitytools.d` or `steamapps/common`.
    pub source: &'static str,
}

/// A file by path and size.
#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    /// File path.
    pub path: String,
    /// File size.
    pub size: u64,
}

/// `userdata/<account>`: file names only.
#[derive(Debug, Clone, Serialize)]
pub struct UserData {
    /// Account id (directory name).
    pub account_id: String,
    /// File names in `config/`.
    pub config_files: Vec<String>,
    /// Number of per-app directories.
    pub app_dirs: usize,
}

/// A credential artifact, recorded by path, size and blake3 only.
#[derive(Debug, Clone, Serialize)]
pub struct CredentialArtifact {
    /// File path.
    pub path: String,
    /// File size.
    pub size: Option<u64>,
    /// blake3 of the content.
    pub blake3: Option<String>,
    /// Read error, when hashing failed.
    pub error: Option<String>,
}

/// Identifiers of a `loginusers.vdf` account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Account {
    /// Login account name.
    pub account_name: String,
    /// SteamID64.
    pub steam_id: String,
}

/// Outcome of [`hash_pending`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HashReport {
    /// Files that had no blake3 yet.
    pub pending: usize,
    /// Files hashed in this call.
    pub hashed: usize,
    /// Files that could not be read.
    pub errors: usize,
}

/// Opens (creating it and its parent directory) the index database and applies the schema.
///
/// # Errors
/// I/O or SQLite failure.
pub fn open(path: &Path) -> Result<Connection, SteamError> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let db = Connection::open(path)?;
    db.execute_batch(SCHEMA)?;
    Ok(db)
}

/// Default database path: `<user data dir>/nie/steam-index.sqlite`.
#[must_use]
pub fn default_db_path() -> PathBuf {
    dirs_next::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("nie")
        .join("steam-index.sqlite")
}

fn lossy(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn read_text(p: &Path) -> Option<VdfValue> {
    parse_vdf(&fs::read_to_string(p).ok()?).ok()
}

fn list_dir(p: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(p)
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    v.sort();
    v
}

fn walk_files(root: &Path, out: &mut Vec<PathBuf>) {
    for p in list_dir(root) {
        if p.is_dir() {
            walk_files(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn size_of(p: &Path) -> u64 {
    fs::metadata(p).map_or(0, |m| m.len())
}

fn file_entry(p: &Path) -> FileEntry {
    FileEntry {
        path: lossy(p),
        size: size_of(p),
    }
}

/// blake3 of a file, streamed. Returns `(size, hex)`.
///
/// # Errors
/// The file cannot be opened or read.
pub fn blake3_file(path: &Path) -> io::Result<(u64, String)> {
    let mut hasher = blake3::Hasher::new();
    let size = io::copy(&mut fs::File::open(path)?, &mut hasher)?;
    Ok((size, hasher.finalize().to_hex().to_string()))
}

fn scan_apps(root: &Path, scan: &mut Scan, lib_paths: &mut Vec<PathBuf>) {
    let libs = scan_steam_libraries(root).unwrap_or_default();
    lib_paths.extend(libs.iter().map(|l| l.path.clone()));
    if !lib_paths.iter().any(|p| p == root) {
        lib_paths.push(root.to_path_buf());
    }
    for lib in &libs {
        let library = lossy(&lib.path);
        for m in &lib.apps {
            let acf = lib
                .steamapps_path
                .join(format!("appmanifest_{}.acf", m.app_id));
            // Depot -> manifest gid / size come from the raw ACF (AppManifest keeps ids only).
            let depots = read_text(&acf)
                .as_ref()
                .and_then(|doc| doc.get_path(&["AppState", "InstalledDepots"]))
                .and_then(VdfValue::entries)
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(|(depot, node)| {
                            Some(AppDepot {
                                depot: depot.parse().ok()?,
                                manifest: node.get_str("manifest").map(str::to_owned),
                                size: node.get_u64("size"),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            let install_path = lib.steamapps_path.join("common").join(&m.install_dir);
            scan.apps.push(App {
                library: library.clone(),
                appid: m.app_id,
                name: m.name.clone(),
                installdir: m.install_dir.clone(),
                install_path: lossy(&install_path),
                exists: install_path.is_dir(),
                size_on_disk: m.size_on_disk,
                buildid: m.build_id,
                state_flags: m.state_flags,
                last_updated: m.last_updated,
                depots,
                acf_path: lossy(&acf),
            });
        }
        scan_workshop(&lib.steamapps_path, &library, scan);
        // Proton and the Steam Linux Runtime ship as apps under common/.
        for p in list_dir(&lib.steamapps_path.join("common")) {
            let name = file_name(&p);
            if name.starts_with("Proton") || name.starts_with("SteamLinuxRuntime") {
                scan.compat_tools.push(CompatTool {
                    name,
                    path: lossy(&p),
                    source: "steamapps/common",
                });
            }
        }
    }
}

fn scan_workshop(steamapps: &Path, library: &str, scan: &mut Scan) {
    let workshop = steamapps.join("workshop");
    for p in list_dir(&workshop) {
        let name = file_name(&p);
        let Some(appid) = name
            .strip_prefix("appworkshop_")
            .and_then(|s| s.strip_suffix(".acf"))
        else {
            continue;
        };
        let doc = read_text(&p);
        let installed = doc
            .as_ref()
            .and_then(|d| d.get_path(&["AppWorkshop", "WorkshopItemsInstalled"]))
            .and_then(VdfValue::entries)
            .unwrap_or_default();
        for (item, node) in installed {
            let dir = workshop.join("content").join(appid).join(item);
            scan.workshop.push(WorkshopItem {
                library: library.to_owned(),
                appid: appid.to_owned(),
                item: item.clone(),
                size: node.get_u64("size"),
                timeupdated: node.get_u64("timeupdated"),
                manifest: node.get_str("manifest").map(str::to_owned),
                path: lossy(&dir),
                exists: dir.is_dir(),
            });
        }
    }
}

fn scan_depotcache(lib_paths: &[PathBuf], scan: &mut Scan) {
    for lib in lib_paths {
        for dir in [
            lib.join("depotcache"),
            lib.join("steamapps").join("depotcache"),
        ] {
            for p in list_dir(&dir) {
                let name = file_name(&p);
                let Some(stem) = name.strip_suffix(".manifest") else {
                    continue;
                };
                let (depot, gid) = stem.split_once('_').unwrap_or((stem, ""));
                scan.depotcache.push(DepotCacheEntry {
                    path: lossy(&p),
                    depot: depot.to_owned(),
                    gid: gid.to_owned(),
                    size: size_of(&p),
                });
            }
        }
    }
}

fn scan_pics(root: &Path, scan: &mut Scan) {
    scan.appinfo = fs::read(root.join("appcache").join("appinfo.vdf"))
        .ok()
        .and_then(|bytes| parse_appinfo(&bytes).ok())
        .map(|f| PicsApps {
            version: f.version,
            universe: f.universe,
            apps: f
                .apps
                .iter()
                .map(|a| {
                    let common =
                        a.kv.as_ref()
                            .and_then(|k| k.get("appinfo").or(Some(k)))
                            .and_then(|k| k.get("common"));
                    let text =
                        |key: &str| common.and_then(|c| c.get(key)).and_then(BinValue::as_text);
                    PicsApp {
                        appid: a.app_id,
                        change: a.change_number,
                        last_updated: a.last_updated,
                        name: text("name"),
                        kind: text("type"),
                    }
                })
                .collect(),
        });
    scan.packageinfo = fs::read(root.join("appcache").join("packageinfo.vdf"))
        .ok()
        .and_then(|bytes| parse_packageinfo(&bytes).ok())
        .map(|f| PicsPackages {
            version: f.version,
            universe: f.universe,
            packages: f
                .packages
                .iter()
                .map(|p| {
                    let node = p.kv.get(&p.package_id.to_string()).unwrap_or(&p.kv);
                    let ids = |key: &str| -> Vec<String> {
                        node.get(key)
                            .and_then(BinValue::entries)
                            .map(|e| e.iter().filter_map(|(_, v)| v.as_text()).collect())
                            .unwrap_or_default()
                    };
                    PicsPackage {
                        packageid: p.package_id,
                        change: p.change_number,
                        appids: ids("appids"),
                        depotids: ids("depotids"),
                    }
                })
                .collect(),
        });
}

fn scan_controller_and_userdata(root: &Path, scan: &mut Scan) {
    for dir in [
        root.join("controller_config"),
        root.join("steamapps")
            .join("common")
            .join("Steam Controller Configs"),
    ] {
        let mut files = Vec::new();
        walk_files(&dir, &mut files);
        scan.controller_configs
            .extend(files.iter().map(|p| file_entry(p)));
    }
    // Controller configs of userdata live under app 241100.
    for acct in list_dir(&root.join("userdata")) {
        if !acct.is_dir() {
            continue;
        }
        let mut controller = Vec::new();
        walk_files(&acct.join("241100"), &mut controller);
        scan.controller_configs
            .extend(controller.iter().map(|p| file_entry(p)));
        scan.userdata.push(UserData {
            account_id: file_name(&acct),
            config_files: list_dir(&acct.join("config"))
                .iter()
                .map(|p| file_name(p))
                .collect(),
            app_dirs: list_dir(&acct).iter().filter(|p| p.is_dir()).count(),
        });
    }
}

fn scan_credentials(root: &Path, scan: &mut Scan) {
    let mut paths: Vec<PathBuf> = list_dir(root)
        .into_iter()
        .filter(|p| file_name(p).starts_with("ssfn"))
        .collect();
    for name in ["loginusers.vdf", "local.vdf", "config.vdf"] {
        paths.push(root.join("config").join(name));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        paths.push(PathBuf::from(local).join("Steam").join("local.vdf"));
    }
    scan.credential_artifacts = paths
        .iter()
        .filter(|p| p.is_file())
        .map(|p| match blake3_file(p) {
            Ok((size, hex)) => CredentialArtifact {
                path: lossy(p),
                size: Some(size),
                blake3: Some(hex),
                error: None,
            },
            Err(e) => CredentialArtifact {
                path: lossy(p),
                size: None,
                blake3: None,
                error: Some(e.to_string()),
            },
        })
        .collect();
}

/// Reads one Steam install. Missing or unreadable parts are skipped, never fatal.
#[must_use]
pub fn scan(root: &Path) -> Scan {
    let mut scan = Scan {
        steam_root: lossy(root),
        ..Scan::default()
    };
    scan.libraries = read_text(&root.join("steamapps").join("libraryfolders.vdf"))
        .as_ref()
        .map(library_folders)
        .unwrap_or_default()
        .into_iter()
        .map(|f| Library {
            index: f.index,
            path: f.path,
            label: f.label,
            total_size: f.total_size,
            app_count: f.apps.len(),
        })
        .collect();
    let mut lib_paths = Vec::new();
    scan_apps(root, &mut scan, &mut lib_paths);
    scan_depotcache(&lib_paths, &mut scan);
    for p in list_dir(&root.join("compatibilitytools.d")) {
        if p.is_dir() {
            scan.compat_tools.push(CompatTool {
                name: file_name(&p),
                path: lossy(&p),
                source: "compatibilitytools.d",
            });
        }
    }
    scan_pics(root, &mut scan);
    scan_controller_and_userdata(root, &mut scan);
    scan_credentials(root, &mut scan);
    scan
}

/// The most recent `loginusers.vdf` account: identifiers only.
#[must_use]
pub fn account(root: &Path) -> Option<Account> {
    let users = read_text(&root.join("config").join("loginusers.vdf"))
        .map(|d| login_users(&d))
        .unwrap_or_default();
    users
        .iter()
        .find(|u| u.most_recent)
        .or_else(|| users.iter().max_by_key(|u| u.timestamp.unwrap_or(0)))
        .map(|u| Account {
            account_name: u.account_name.clone(),
            steam_id: u.steam_id.clone(),
        })
}

/// Replaces every table except `files` with the content of `scan`, in one transaction.
///
/// # Errors
/// SQLite failure.
pub fn store(db: &mut Connection, scan: &Scan) -> Result<(), SteamError> {
    let tx = db.transaction()?;
    for table in REBUILT {
        tx.execute(&format!("DELETE FROM {table}"), [])?;
    }
    for l in &scan.libraries {
        tx.execute(
            "INSERT OR REPLACE INTO libraries VALUES (?,?,?,?,?)",
            params![
                l.index,
                l.path,
                l.label,
                sql_int(l.total_size),
                l.app_count as i64
            ],
        )?;
    }
    for a in &scan.apps {
        tx.execute(
            "INSERT OR REPLACE INTO apps VALUES (?,?,?,?,?,?,?,?,?,?,?)",
            params![
                a.appid,
                a.name,
                a.installdir,
                a.library,
                a.install_path,
                a.exists,
                sql_int(Some(a.size_on_disk)),
                sql_int(Some(a.buildid)),
                sql_int(Some(a.state_flags)),
                sql_int(Some(a.last_updated)),
                a.acf_path
            ],
        )?;
        for d in &a.depots {
            tx.execute(
                "INSERT OR REPLACE INTO app_depots VALUES (?,?,?,?)",
                params![a.appid, d.depot, d.manifest, sql_int(d.size)],
            )?;
        }
    }
    for d in &scan.depotcache {
        tx.execute(
            "INSERT OR REPLACE INTO depotcache VALUES (?,?,?,?)",
            params![
                d.path,
                d.depot.parse::<i64>().ok(),
                d.gid,
                sql_int(Some(d.size))
            ],
        )?;
    }
    for a in scan.appinfo.iter().flat_map(|i| &i.apps) {
        tx.execute(
            "INSERT OR REPLACE INTO pics_apps VALUES (?,?,?,?,?)",
            params![a.appid, a.name, a.kind, a.change, a.last_updated],
        )?;
    }
    for p in scan.packageinfo.iter().flat_map(|i| &i.packages) {
        tx.execute(
            "INSERT OR REPLACE INTO pics_packages VALUES (?,?,?,?)",
            params![
                p.packageid,
                p.change,
                serde_json::to_string(&p.appids).unwrap_or_default(),
                serde_json::to_string(&p.depotids).unwrap_or_default()
            ],
        )?;
    }
    for w in &scan.workshop {
        tx.execute(
            "INSERT OR REPLACE INTO workshop VALUES (?,?,?,?,?,?,?,?)",
            params![
                w.appid.parse::<i64>().ok(),
                w.item,
                sql_int(w.size),
                sql_int(w.timeupdated),
                w.manifest,
                w.path,
                w.exists,
                w.library
            ],
        )?;
    }
    for c in &scan.compat_tools {
        tx.execute(
            "INSERT OR REPLACE INTO compat_tools VALUES (?,?,?)",
            params![c.name, c.path, c.source],
        )?;
    }
    for c in &scan.controller_configs {
        tx.execute(
            "INSERT OR REPLACE INTO controller_configs VALUES (?,?)",
            params![c.path, sql_int(Some(c.size))],
        )?;
    }
    for u in &scan.userdata {
        for f in &u.config_files {
            tx.execute(
                "INSERT OR REPLACE INTO userdata VALUES (?,?)",
                params![u.account_id, f],
            )?;
        }
    }
    for c in &scan.credential_artifacts {
        tx.execute(
            "INSERT OR REPLACE INTO credential_artifacts VALUES (?,?,?,?)",
            params![c.path, sql_int(c.size), c.blake3, c.error],
        )?;
    }
    let appinfo_version = scan.appinfo.as_ref().map(|i| i.version.to_string());
    let packageinfo_version = scan.packageinfo.as_ref().map(|i| i.version.to_string());
    for (key, value) in [
        ("steam_root", Some(scan.steam_root.clone())),
        ("scanned_at", Some(unix_ms().to_string())),
        ("appinfo_version", appinfo_version),
        ("packageinfo_version", packageinfo_version),
    ] {
        tx.execute(
            "INSERT OR REPLACE INTO meta VALUES (?,?)",
            params![key, value.unwrap_or_default()],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// SQLite integers are signed 64-bit: larger values saturate.
fn sql_int(value: Option<u64>) -> Option<i64> {
    value.map(|v| i64::try_from(v).unwrap_or(i64::MAX))
}

fn unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

fn mtime_ms(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// Registers every file of every installed game in `files`.
///
/// A file whose size or mtime changed loses its blake3 and is hashed again by
/// [`hash_pending`]; an unchanged file keeps it. Returns the number of files seen.
///
/// # Errors
/// SQLite failure.
pub fn inventory(db: &mut Connection, apps: &[App]) -> Result<usize, SteamError> {
    let mut seen = 0;
    for app in apps.iter().filter(|a| a.exists) {
        let mut files = Vec::new();
        collect_regular_files(Path::new(&app.install_path), &mut files);
        let tx = db.transaction()?;
        {
            let mut get = tx.prepare("SELECT size, mtime_ms FROM files WHERE path = ?")?;
            let mut up = tx.prepare(
                "INSERT INTO files (path, appid, size, mtime_ms) VALUES (?,?,?,?)
                 ON CONFLICT(path) DO UPDATE SET appid=excluded.appid, size=excluded.size,
                   mtime_ms=excluded.mtime_ms, blake3=NULL, error=NULL, hashed_at=NULL",
            )?;
            for path in files {
                let Ok(meta) = fs::metadata(&path) else {
                    continue;
                };
                seen += 1;
                let path = lossy(&path);
                let size = i64::try_from(meta.len()).unwrap_or(i64::MAX);
                let mtime = mtime_ms(&meta);
                let known = get
                    .query_row([&path], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
                    .ok();
                if known != Some((size, mtime)) {
                    up.execute(params![path, app.appid, size, mtime])?;
                }
            }
        }
        tx.commit()?;
    }
    Ok(seen)
}

fn collect_regular_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for entry in rd.filter_map(Result::ok) {
        match entry.file_type() {
            Ok(t) if t.is_dir() => collect_regular_files(&entry.path(), out),
            Ok(t) if t.is_file() => out.push(entry.path()),
            _ => {}
        }
    }
}

/// Hashes every `files` row that has neither a blake3 nor an error, in parallel batches of 500
/// committed one by one: an interrupted run resumes where it stopped. `progress(done, pending)`
/// is called after each batch.
///
/// # Errors
/// SQLite failure.
pub fn hash_pending(
    db: &mut Connection,
    mut progress: impl FnMut(usize, usize),
) -> Result<HashReport, SteamError> {
    let paths: Vec<String> = db
        .prepare("SELECT path FROM files WHERE blake3 IS NULL AND error IS NULL ORDER BY path")?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let mut report = HashReport {
        pending: paths.len(),
        ..HashReport::default()
    };
    for chunk in paths.chunks(HASH_BATCH) {
        let hashed: Vec<_> = chunk
            .par_iter()
            .map(|p| (p, blake3_file(Path::new(p))))
            .collect();
        let now = unix_ms();
        let tx = db.transaction()?;
        {
            let mut set =
                tx.prepare("UPDATE files SET blake3 = ?, error = ?, hashed_at = ? WHERE path = ?")?;
            for (path, result) in &hashed {
                match result {
                    Ok((_, hex)) => set.execute(params![hex, None::<String>, now, path])?,
                    Err(e) => {
                        report.errors += 1;
                        set.execute(params![None::<String>, e.to_string(), now, path])?
                    }
                };
            }
        }
        tx.commit()?;
        report.hashed += hashed.len();
        progress(report.hashed, report.pending);
    }
    Ok(report)
}
