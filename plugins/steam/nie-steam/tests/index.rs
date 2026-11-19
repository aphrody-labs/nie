// SPDX-License-Identifier: Apache-2.0
//! `nie_steam::index` on a synthetic Steam install built in a temp directory.
#![cfg(feature = "index")]

use std::fs;
use std::path::Path;

use nie_steam::index::{self, Account};
use rusqlite::Connection;

fn vdf_escape(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "\\\\")
}

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// A Steam root with one library, one installed app (two depots, three files), a workshop item,
/// a depotcache manifest, a Proton tool, an `ssfn` file and two login users.
fn fixture(root: &Path) {
    let steamapps = root.join("steamapps");
    write(
        &steamapps.join("libraryfolders.vdf"),
        &format!(
            "\"libraryfolders\"\n{{\n\"0\"\n{{\n\"path\" \"{}\"\n\"label\" \"main\"\n\"totalsize\" \"1000\"\n\"apps\"\n{{\n\"111\" \"42\"\n}}\n}}\n}}\n",
            vdf_escape(root)
        ),
    );
    write(
        &steamapps.join("appmanifest_111.acf"),
        "\"AppState\"\n{\n\"appid\" \"111\"\n\"name\" \"Test Game\"\n\"installdir\" \"TestGame\"\n\"StateFlags\" \"4\"\n\"SizeOnDisk\" \"42\"\n\"buildid\" \"7\"\n\"LastUpdated\" \"99\"\n\"InstalledDepots\"\n{\n\"112\"\n{\n\"manifest\" \"9001\"\n\"size\" \"30\"\n}\n\"113\"\n{\n\"manifest\" \"9002\"\n\"size\" \"12\"\n}\n}\n}\n",
    );
    let game = steamapps.join("common").join("TestGame");
    write(&game.join("a.bin"), "alpha");
    write(&game.join("data").join("b.bin"), "bravo");
    write(&game.join("data").join("c.bin"), "");
    write(
        &steamapps.join("workshop").join("appworkshop_111.acf"),
        "\"AppWorkshop\"\n{\n\"WorkshopItemsInstalled\"\n{\n\"555\"\n{\n\"size\" \"8\"\n\"timeupdated\" \"5\"\n\"manifest\" \"77\"\n}\n}\n}\n",
    );
    fs::create_dir_all(
        steamapps
            .join("workshop")
            .join("content")
            .join("111")
            .join("555"),
    )
    .unwrap();
    fs::create_dir_all(steamapps.join("common").join("Proton 9.0")).unwrap();
    fs::create_dir_all(root.join("compatibilitytools.d").join("GE-Proton9-1")).unwrap();
    write(
        &root.join("depotcache").join("112_9001.manifest"),
        "manifest",
    );
    write(&root.join("ssfn1234"), "secret");
    write(
        &root.join("config").join("loginusers.vdf"),
        "\"users\"\n{\n\"76561190000000001\"\n{\n\"AccountName\" \"older\"\n\"MostRecent\" \"0\"\n\"Timestamp\" \"10\"\n}\n\"76561190000000002\"\n{\n\"AccountName\" \"newer\"\n\"MostRecent\" \"1\"\n\"Timestamp\" \"20\"\n}\n}\n",
    );
    write(
        &root
            .join("userdata")
            .join("1001")
            .join("config")
            .join("localconfig.vdf"),
        "x",
    );
}

fn count(db: &Connection, table: &str) -> i64 {
    db.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

#[test]
fn scan_reads_libraries_apps_depots_and_tools() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let scan = index::scan(dir.path());

    assert_eq!(scan.libraries.len(), 1);
    assert_eq!(scan.libraries[0].label, "main");
    assert_eq!(scan.libraries[0].app_count, 1);

    assert_eq!(scan.apps.len(), 1);
    let app = &scan.apps[0];
    assert_eq!(
        (app.appid, app.name.as_str(), app.buildid),
        (111, "Test Game", 7)
    );
    assert!(app.exists);
    let depots: Vec<_> = app
        .depots
        .iter()
        .map(|d| (d.depot, d.manifest.as_deref(), d.size))
        .collect();
    assert_eq!(
        depots,
        [(112, Some("9001"), Some(30)), (113, Some("9002"), Some(12))]
    );

    assert_eq!(scan.depotcache.len(), 1);
    assert_eq!(
        (
            scan.depotcache[0].depot.as_str(),
            scan.depotcache[0].gid.as_str()
        ),
        ("112", "9001")
    );

    assert_eq!(scan.workshop.len(), 1);
    assert_eq!(
        (scan.workshop[0].item.as_str(), scan.workshop[0].size),
        ("555", Some(8))
    );
    assert!(scan.workshop[0].exists);

    let tools: Vec<_> = scan
        .compat_tools
        .iter()
        .map(|t| (t.name.as_str(), t.source))
        .collect();
    assert!(tools.contains(&("Proton 9.0", "steamapps/common")));
    assert!(tools.contains(&("GE-Proton9-1", "compatibilitytools.d")));

    assert_eq!(scan.userdata[0].account_id, "1001");
    assert_eq!(scan.userdata[0].config_files, ["localconfig.vdf"]);

    let creds: Vec<_> = scan
        .credential_artifacts
        .iter()
        .map(|c| c.path.as_str())
        .collect();
    assert!(creds.iter().any(|p| p.ends_with("ssfn1234")));
    assert!(creds.iter().any(|p| p.ends_with("loginusers.vdf")));
    let ssfn = scan
        .credential_artifacts
        .iter()
        .find(|c| c.path.ends_with("ssfn1234"))
        .unwrap();
    assert_eq!(
        ssfn.blake3.as_deref(),
        Some(blake3::hash(b"secret").to_hex().as_str())
    );
}

#[test]
fn account_is_the_most_recent_login() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    assert_eq!(
        index::account(dir.path()),
        Some(Account {
            account_name: "newer".into(),
            steam_id: "76561190000000002".into(),
        })
    );
    assert_eq!(index::account(&dir.path().join("missing")), None);
}

#[test]
fn store_is_rebuilt_on_every_run() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let mut db = index::open(&dir.path().join("index.sqlite")).unwrap();
    let scan = index::scan(dir.path());
    index::store(&mut db, &scan).unwrap();
    index::store(&mut db, &scan).unwrap();

    assert_eq!(count(&db, "libraries"), 1);
    assert_eq!(count(&db, "apps"), 1);
    assert_eq!(count(&db, "app_depots"), 2);
    assert_eq!(count(&db, "depotcache"), 1);
    assert_eq!(count(&db, "workshop"), 1);
    let (name, exists): (String, i64) = db
        .query_row(
            "SELECT name, exists_on_disk FROM apps WHERE appid = 111",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((name.as_str(), exists), ("Test Game", 1));
    let root: String = db
        .query_row("SELECT value FROM meta WHERE key = 'steam_root'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(root, dir.path().to_string_lossy());
}

#[test]
fn hashing_is_resumable_and_follows_changes() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let mut db = index::open(&dir.path().join("index.sqlite")).unwrap();
    let scan = index::scan(dir.path());
    index::store(&mut db, &scan).unwrap();

    assert_eq!(index::inventory(&mut db, &scan.apps).unwrap(), 3);
    assert_eq!(count(&db, "files"), 3);
    let first = index::hash_pending(&mut db, |_, _| {}).unwrap();
    assert_eq!((first.pending, first.hashed, first.errors), (3, 3, 0));

    let hash_of = |db: &Connection, suffix: &str| -> Option<String> {
        db.query_row(
            "SELECT blake3 FROM files WHERE path LIKE ?",
            [format!("%{suffix}")],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(
        hash_of(&db, "a.bin"),
        Some(blake3::hash(b"alpha").to_hex().to_string())
    );
    assert_eq!(
        hash_of(&db, "c.bin"),
        Some(blake3::hash(b"").to_hex().to_string())
    );

    // Unchanged files keep their hash and are not pending again.
    index::inventory(&mut db, &scan.apps).unwrap();
    let rerun = index::hash_pending(&mut db, |_, _| {}).unwrap();
    assert_eq!((rerun.pending, rerun.hashed), (0, 0));

    // A changed file (new size) loses its hash and is hashed again.
    fs::write(
        dir.path().join("steamapps/common/TestGame/a.bin"),
        "alpha-changed",
    )
    .unwrap();
    index::inventory(&mut db, &scan.apps).unwrap();
    assert_eq!(hash_of(&db, "a.bin"), None);
    let again = index::hash_pending(&mut db, |_, _| {}).unwrap();
    assert_eq!((again.pending, again.hashed), (1, 1));
    assert_eq!(
        hash_of(&db, "a.bin"),
        Some(blake3::hash(b"alpha-changed").to_hex().to_string())
    );
}

#[test]
fn unreadable_file_is_recorded_as_error_not_retried() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let mut db = index::open(&dir.path().join("index.sqlite")).unwrap();
    let scan = index::scan(dir.path());
    index::store(&mut db, &scan).unwrap();
    index::inventory(&mut db, &scan.apps).unwrap();
    fs::remove_file(dir.path().join("steamapps/common/TestGame/a.bin")).unwrap();
    let report = index::hash_pending(&mut db, |_, _| {}).unwrap();
    assert_eq!((report.pending, report.hashed, report.errors), (3, 3, 1));
    assert_eq!(index::hash_pending(&mut db, |_, _| {}).unwrap().pending, 0);
}

#[test]
fn scan_of_a_missing_root_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let scan = index::scan(&dir.path().join("nothing"));
    assert!(scan.libraries.is_empty() && scan.apps.is_empty() && scan.depotcache.is_empty());
}
