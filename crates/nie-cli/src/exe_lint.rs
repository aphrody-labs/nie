//! `nie exe lint`: which classes and functions of nie.exe have a typed Rust equivalent.
//!
//! The classes come from the RTTI that `nie rtti` stored, the functions from the index that
//! `nie index` stored (`rtti_class` and `function` in the database). The Rust side is read from the
//! workspace sources: a class has a typed equivalent when a `struct`, `enum`, `trait`, `type` or
//! `union` carries its normalised name; a function when an `fn` does, or when its address appears
//! as a `0x...` literal. A class or function named only in a comment is reported as documented.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use nie_index::rusqlite::{Connection, OpenFlags};
use serde::Serialize;

/// Options of `nie exe lint`.
#[derive(Debug, Clone, clap::Args)]
pub struct ExeOptions {
    /// Index database written by `nie index` and `nie rtti`.
    #[arg(long, default_value = "var/nie.sqlite")]
    pub db: PathBuf,
    /// Rows listed per section in the text report.
    #[arg(long, default_value_t = 10)]
    pub examples: usize,
    /// JSON output.
    #[arg(long)]
    pub json: bool,
}

/// Where a Rust item or mention was found.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Evidence {
    /// Workspace-relative file.
    pub file: String,
    /// 1-based line.
    pub line: usize,
}

/// A class or function and what the workspace says about it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Row {
    /// Class name or function name.
    pub name: String,
    /// Virtual address (functions) or vtable address (classes).
    pub vaddr: Option<i64>,
    /// A Rust item with the matching name or address.
    pub typed: Option<Evidence>,
    /// A comment that names it.
    pub documented: Option<Evidence>,
}

/// Counts for one kind of item.
#[derive(Debug, Default, Clone, Serialize, PartialEq)]
pub struct Totals {
    /// Items in the database.
    pub total: usize,
    /// Items with a usable name (functions only; equals `total` for classes).
    pub named: usize,
    /// Items with a typed Rust equivalent.
    pub typed: usize,
    /// Items named in a comment only.
    pub documented: usize,
    /// `typed` over `named`, in percent.
    pub pct_typed: f64,
}

/// The report.
#[derive(Debug, Clone, Serialize)]
pub struct ExeReport {
    /// Binary path recorded by `nie index`.
    pub binary: String,
    /// Binary SHA-256.
    pub sha256: String,
    /// The `coverage` row the classification loop stored last, if any: (total, named, classified).
    pub classified: Option<(i64, i64, i64)>,
    /// Class totals.
    pub classes: Totals,
    /// Function totals.
    pub functions: Totals,
    /// Every class.
    pub class_rows: Vec<Row>,
    /// Functions with a typed equivalent or a mention.
    pub function_rows: Vec<Row>,
}

/// What to look for in the sources.
#[derive(Debug, Default)]
pub struct Wanted {
    /// Normalised type names (classes).
    pub types: HashSet<String>,
    /// Normalised function names.
    pub fns: HashSet<String>,
    /// Lowercase hexadecimal addresses without prefix or leading zeros.
    pub addrs: HashSet<String>,
}

/// What the sources contain.
#[derive(Debug, Default)]
pub struct Found {
    /// Items defining a wanted type name.
    pub types: HashMap<String, Evidence>,
    /// Comments naming a wanted type.
    pub type_mentions: HashMap<String, Evidence>,
    /// Items defining a wanted function name.
    pub fns: HashMap<String, Evidence>,
    /// Comments naming a wanted function.
    pub fn_mentions: HashMap<String, Evidence>,
    /// Literals equal to a wanted address.
    pub addrs: HashMap<String, Evidence>,
}

/// Lowercase and drop underscores, so `CharaParam`, `chara_param` and `CHARA_PARAM` meet.
#[must_use]
pub fn normalize(identifier: &str) -> String {
    identifier
        .chars()
        .filter(|c| *c != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

/// The normalised names a class can go by: the last path segment without template arguments,
/// and the same without the MSVC `C` prefix (`CCharaParam` is `CharaParam`).
#[must_use]
pub fn class_keys(name: &str) -> Vec<String> {
    let base = name.split('<').next().unwrap_or(name);
    let last = base.rsplit("::").next().unwrap_or(base).trim();
    let mut keys = vec![normalize(last)];
    let mut chars = last.chars();
    if let (Some('C'), Some(second)) = (chars.next(), chars.next())
        && second.is_uppercase()
    {
        keys.push(normalize(&last[1..]));
    }
    keys.retain(|k| k.len() >= 4);
    keys
}

/// Whether the index gave a function a real name rather than an address placeholder.
#[must_use]
pub fn usable_function_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    name.len() >= 5
        && !["fun_", "sub_", "thunk_", "loc_", "dat_", "lab_"]
            .iter()
            .any(|p| lower.starts_with(p))
}

/// Lowercase hex of a literal such as `0x1400_AB`, without prefix, underscores and leading zeros.
#[must_use]
pub fn hex_literal(token: &str) -> Option<String> {
    let digits = token
        .strip_prefix("0x")
        .or_else(|| token.strip_prefix("0X"))?;
    let digits = [
        "usize", "isize", "u64", "i64", "u32", "i32", "u16", "i16", "u8", "i8",
    ]
    .iter()
    .find_map(|suffix| digits.strip_suffix(suffix))
    .unwrap_or(digits);
    let digits: String = digits.chars().filter(|c| *c != '_').collect();
    if digits.len() < 5 || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let lower = digits.to_lowercase();
    Some(lower.trim_start_matches('0').to_owned())
}

/// Scans the text of one file.
pub fn scan_text(file: &str, text: &str, wanted: &Wanted, found: &mut Found) {
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        let comment =
            trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*');
        let evidence = || Evidence {
            file: file.to_owned(),
            line: index + 1,
        };
        let mut previous = String::new();
        let bytes = line.as_bytes();
        let mut at = 0;
        while at < bytes.len() {
            let c = bytes[at];
            if !(c.is_ascii_alphanumeric() || c == b'_') {
                at += 1;
                continue;
            }
            let start = at;
            while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_') {
                at += 1;
            }
            let token = &line[start..at];
            if c.is_ascii_digit() {
                if let Some(addr) = hex_literal(token)
                    && wanted.addrs.contains(&addr)
                {
                    found.addrs.entry(addr).or_insert_with(evidence);
                }
                continue;
            }
            let key = normalize(token);
            if comment {
                if wanted.types.contains(&key) {
                    found
                        .type_mentions
                        .entry(key.clone())
                        .or_insert_with(evidence);
                }
                if wanted.fns.contains(&key) {
                    found.fn_mentions.entry(key).or_insert_with(evidence);
                }
            } else {
                match previous.as_str() {
                    "struct" | "enum" | "trait" | "type" | "union"
                        if wanted.types.contains(&key) =>
                    {
                        found.types.entry(key).or_insert_with(evidence);
                    }
                    "fn" if wanted.fns.contains(&key) => {
                        found.fns.entry(key).or_insert_with(evidence);
                    }
                    _ => {}
                }
            }
            previous = token.to_owned();
        }
    }
}

const SKIPPED_DIRS: [&str; 7] = [
    "target",
    "node_modules",
    "vendor",
    "third_party",
    "dist",
    "var",
    "xtask",
];

/// This file names classes and functions in its own tests, so the scan skips it.
const SKIPPED_FILES: [&str; 1] = ["exe_lint.rs"];

/// Scans every `.rs` file under `root` (this file, `xtask` and build output excluded).
pub fn scan_tree(root: &Path, wanted: &Wanted) -> Found {
    let mut found = Found::default();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if !name.starts_with('.')
                    && !name.starts_with("target")
                    && !SKIPPED_DIRS.contains(&name.as_str())
                {
                    stack.push(path);
                }
            } else if name.ends_with(".rs")
                && !SKIPPED_FILES.contains(&name.as_str())
                && let Ok(text) = std::fs::read_to_string(&path)
            {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                scan_text(&rel, &text, wanted, &mut found);
            }
        }
    }
    found
}

fn pct(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 * 100.0 / whole as f64
    }
}

/// The report for classes and functions read from the database.
#[must_use]
pub fn build_report(
    binary: (String, String),
    classified: Option<(i64, i64, i64)>,
    classes: &[(String, Option<i64>)],
    functions: &[(i64, Option<String>)],
    found: &Found,
) -> ExeReport {
    let mut class_rows = Vec::with_capacity(classes.len());
    for (name, vaddr) in classes {
        let keys = class_keys(name);
        let typed = keys.iter().find_map(|k| found.types.get(k)).cloned();
        let documented = keys
            .iter()
            .find_map(|k| found.type_mentions.get(k))
            .cloned();
        class_rows.push(Row {
            name: name.clone(),
            vaddr: *vaddr,
            typed,
            documented,
        });
    }
    let mut function_rows = Vec::new();
    let mut named = 0;
    let mut typed_count = 0;
    let mut documented_count = 0;
    for (vaddr, name) in functions {
        let usable = name.as_deref().filter(|n| usable_function_name(n));
        if usable.is_some() {
            named += 1;
        }
        let key = usable.map(normalize);
        let by_name = key.as_ref().and_then(|k| found.fns.get(k)).cloned();
        let by_addr = found.addrs.get(&format!("{vaddr:x}")).cloned();
        let typed = by_name.or(by_addr);
        let documented = if typed.is_none() {
            key.as_ref().and_then(|k| found.fn_mentions.get(k)).cloned()
        } else {
            None
        };
        if typed.is_some() {
            typed_count += 1;
        } else if documented.is_some() {
            documented_count += 1;
        }
        if typed.is_some() || documented.is_some() {
            function_rows.push(Row {
                name: usable.unwrap_or_default().to_owned(),
                vaddr: Some(*vaddr),
                typed,
                documented,
            });
        }
    }
    let class_typed = class_rows.iter().filter(|r| r.typed.is_some()).count();
    let class_documented = class_rows
        .iter()
        .filter(|r| r.typed.is_none() && r.documented.is_some())
        .count();
    ExeReport {
        binary: binary.0,
        sha256: binary.1,
        classified,
        classes: Totals {
            total: classes.len(),
            named: classes.len(),
            typed: class_typed,
            documented: class_documented,
            pct_typed: pct(class_typed, classes.len()),
        },
        functions: Totals {
            total: functions.len(),
            named,
            typed: typed_count,
            documented: documented_count,
            pct_typed: pct(typed_count, named),
        },
        class_rows,
        function_rows,
    }
}

/// The wanted names and addresses for the database rows.
#[must_use]
pub fn wanted_for(
    classes: &[(String, Option<i64>)],
    functions: &[(i64, Option<String>)],
) -> Wanted {
    let mut wanted = Wanted::default();
    for (name, _) in classes {
        wanted.types.extend(class_keys(name));
    }
    for (vaddr, name) in functions {
        wanted.addrs.insert(format!("{vaddr:x}"));
        if let Some(name) = name.as_deref().filter(|n| usable_function_name(n)) {
            wanted.fns.insert(normalize(name));
        }
    }
    wanted
}

type Loaded = (
    (String, String),
    Option<(i64, i64, i64)>,
    Vec<(String, Option<i64>)>,
    Vec<(i64, Option<String>)>,
);

/// Reads the newest indexed binary, its classes and its functions.
///
/// # Errors
/// When the database is missing, empty or does not have the index tables.
pub fn load(db: &Path) -> Result<Loaded> {
    if !db.is_file() {
        bail!(
            "index database `{}` not found: run `nie index --exe <nie.exe>` and `nie rtti --exe <nie.exe>` first (or pass --db)",
            db.display()
        );
    }
    let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("opening {}", db.display()))?;
    let (id, path, sha): (i64, String, String) = conn
        .query_row(
            "SELECT id, path, sha256 FROM binary ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .context("the database holds no indexed binary: run `nie index` first")?;
    let classified = conn
        .query_row(
            "SELECT total_funcs, named, classified FROM coverage WHERE binary_id = ?1 ORDER BY id DESC LIMIT 1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok();
    let classes = conn
        .prepare("SELECT name, vtable_vaddr FROM rtti_class WHERE binary_id = ?1 ORDER BY name")?
        .query_map([id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let functions = conn
        .prepare("SELECT vaddr, name FROM function WHERE binary_id = ?1 ORDER BY vaddr")?
        .query_map([id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if classes.is_empty() {
        eprintln!("note: no RTTI classes in the database, run `nie rtti` to fill them");
    }
    Ok(((path, sha), classified, classes, functions))
}

/// The human readable report.
#[must_use]
pub fn render(report: &ExeReport, examples: usize) -> String {
    let mut out = format!("binary {} (sha256 {})\n", report.binary, report.sha256);
    if let Some((total, named, classified)) = report.classified {
        out.push_str(&format!(
            "index: {total} functions, {named} named, {classified} classified\n"
        ));
    }
    let line = |label: &str, t: &Totals| {
        format!(
            "{label:<10} {:>7} total  {:>7} named  {:>6} typed ({:.2} %)  {:>6} documented only\n",
            t.total, t.named, t.typed, t.pct_typed, t.documented
        )
    };
    out.push_str(&line("classes", &report.classes));
    out.push_str(&line("functions", &report.functions));
    let show = |out: &mut String, title: &str, rows: Vec<&Row>| {
        if rows.is_empty() {
            return;
        }
        out.push_str(&format!("\n{title}\n"));
        for row in rows.into_iter().take(examples) {
            let at = row.typed.as_ref().or(row.documented.as_ref());
            let place = at.map_or(String::new(), |e| format!("  {}:{}", e.file, e.line));
            out.push_str(&format!("  {}{place}\n", row.name));
        }
    };
    show(
        &mut out,
        "classes with a typed equivalent",
        report
            .class_rows
            .iter()
            .filter(|r| r.typed.is_some())
            .collect(),
    );
    show(
        &mut out,
        "classes without one",
        report
            .class_rows
            .iter()
            .filter(|r| r.typed.is_none())
            .collect(),
    );
    show(
        &mut out,
        "functions with a typed equivalent",
        report
            .function_rows
            .iter()
            .filter(|r| r.typed.is_some())
            .collect(),
    );
    out
}

/// The git top level, else the current directory.
fn repo_root() -> PathBuf {
    std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()))
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Reads the database, scans the workspace and prints the report.
///
/// # Errors
/// When the database cannot be read.
pub fn run(options: &ExeOptions) -> Result<()> {
    let (binary, classified, classes, functions) = load(&options.db)?;
    let wanted = wanted_for(&classes, &functions);
    let root = repo_root();
    let found = scan_tree(&root, &wanted);
    let report = build_report(binary, classified, &classes, &functions, &found);
    if options.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", render(&report, options.examples));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wanted() -> Wanted {
        let classes = vec![
            ("lives::CCharaParam".to_owned(), Some(0x1000)),
            ("lives::Missing".to_owned(), None),
        ];
        let functions = vec![
            (0x1400_1234_i64, Some("LoadChara".to_owned())),
            (0x2000, Some("FUN_00002000".to_owned())),
        ];
        wanted_for(&classes, &functions)
    }

    #[test]
    fn names_normalise_across_conventions() {
        assert_eq!(normalize("CHARA_PARAM"), "charaparam");
        assert_eq!(normalize("CharaParam"), "charaparam");
        assert_eq!(
            class_keys("lives::CCharaParam"),
            ["ccharaparam", "charaparam"]
        );
        assert_eq!(class_keys("ns::Vector<int>"), ["vector"]);
        assert!(class_keys("ns::Box<int>").is_empty());
        assert!(class_keys("a::Io").is_empty());
    }

    #[test]
    fn placeholder_function_names_are_not_names() {
        assert!(usable_function_name("LoadChara"));
        assert!(!usable_function_name("FUN_140001234"));
        assert!(!usable_function_name("sub_1400"));
        assert!(!usable_function_name("run"));
    }

    #[test]
    fn hex_literals_are_normalised() {
        assert_eq!(hex_literal("0x1400_1234").as_deref(), Some("14001234"));
        assert_eq!(hex_literal("0x0001400u32").as_deref(), Some("1400"));
        assert_eq!(hex_literal("0xFF"), None);
        assert_eq!(hex_literal("12345"), None);
    }

    #[test]
    fn scanning_finds_types_functions_addresses_and_mentions() {
        let source = "\
/// Mirrors lives::Missing in the exe.
pub struct CharaParam { hp: u32 }
fn load_chara() {}
const ENTRY: u64 = 0x1400_1234;
";
        let mut found = Found::default();
        scan_text("crates/x/src/lib.rs", source, &wanted(), &mut found);
        assert_eq!(
            found.types["charaparam"],
            Evidence {
                file: "crates/x/src/lib.rs".into(),
                line: 2
            }
        );
        assert_eq!(found.fns["loadchara"].line, 3);
        assert_eq!(found.addrs["14001234"].line, 4);
        assert_eq!(found.type_mentions["missing"].line, 1);
    }

    #[test]
    fn the_report_counts_typed_documented_and_unnamed() {
        let classes = vec![
            ("lives::CCharaParam".to_owned(), Some(1)),
            ("lives::Missing".to_owned(), None),
        ];
        let functions = vec![
            (0x1400_1234_i64, Some("LoadChara".to_owned())),
            (0x2000, Some("FUN_00002000".to_owned())),
        ];
        let mut found = Found::default();
        scan_text(
            "a.rs",
            "struct CharaParam;\n// notes on Missing\nfn load_chara() {}\n",
            &wanted_for(&classes, &functions),
            &mut found,
        );
        let report = build_report(
            ("nie.exe".into(), "00".into()),
            None,
            &classes,
            &functions,
            &found,
        );
        assert_eq!(
            (
                report.classes.total,
                report.classes.typed,
                report.classes.documented
            ),
            (2, 1, 1)
        );
        assert!((report.classes.pct_typed - 50.0).abs() < 1e-9);
        assert_eq!(
            (
                report.functions.total,
                report.functions.named,
                report.functions.typed
            ),
            (2, 1, 1)
        );
        assert!((report.functions.pct_typed - 100.0).abs() < 1e-9);
        let text = render(&report, 5);
        assert!(text.contains("CharaParam") && text.contains("a.rs:1"));
    }

    #[test]
    fn a_missing_database_is_explained() {
        let err = load(Path::new("definitely/not/here.sqlite"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("nie index") && err.contains("nie rtti"));
    }

    #[test]
    fn a_synthetic_database_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nie.sqlite");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE binary(id INTEGER PRIMARY KEY, path TEXT, sha256 TEXT);
             CREATE TABLE rtti_class(id INTEGER PRIMARY KEY, binary_id INTEGER, name TEXT, vtable_vaddr INTEGER);
             CREATE TABLE function(id INTEGER PRIMARY KEY, binary_id INTEGER, vaddr INTEGER, name TEXT);
             CREATE TABLE coverage(id INTEGER PRIMARY KEY, binary_id INTEGER, total_funcs INTEGER, named INTEGER, classified INTEGER);
             INSERT INTO binary VALUES (1, 'nie.exe', 'abc');
             INSERT INTO rtti_class VALUES (1, 1, 'lives::Thing', 4096);
             INSERT INTO function VALUES (1, 1, 5000, 'DoThing');
             INSERT INTO coverage VALUES (1, 1, 10, 4, 2);",
        )
        .unwrap();
        drop(conn);
        let (binary, classified, classes, functions) = load(&path).unwrap();
        assert_eq!(binary.0, "nie.exe");
        assert_eq!(classified, Some((10, 4, 2)));
        assert_eq!(classes, [("lives::Thing".to_owned(), Some(4096))]);
        assert_eq!(functions, [(5000, Some("DoThing".to_owned()))]);
    }
}
