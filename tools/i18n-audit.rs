//! Zero-dependency single-file Rust i18n auditor, linter, formatter & rewriter
//! for Aphrody Labs repos (TSV catalogs compatible with storytold/craft-i18n).
//!
//! Usage:
//!   rustc -O tools/i18n-audit.rs -o target/i18n-audit.exe
//!   i18n-audit --root <repo_dir> [--catalog <path.tsv>] [--fix] [--fmt-tsv]

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut root = PathBuf::from(".");
    let mut catalog = PathBuf::from("locales/messages.tsv");
    let mut fix = false;
    let mut fmt_tsv = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--root" if i + 1 < args.len() => {
                root = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--catalog" if i + 1 < args.len() => {
                catalog = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--fix" => {
                fix = true;
                i += 1;
            }
            "--fmt-tsv" => {
                fmt_tsv = true;
                i += 1;
            }
            "-h" | "--help" => {
                println!("i18n-audit --root <dir> [--catalog <tsv>] [--fix] [--fmt-tsv]");
                return;
            }
            other => {
                eprintln!("unknown arg: {other}");
                process::exit(2);
            }
        }
    }

    let mut rs_files = Vec::new();
    let mut tsv_files = Vec::new();
    walk(&root, &mut rs_files, &mut tsv_files);

    let mut tsv_errors = 0usize;
    for tsv in &tsv_files {
        if let Err(e) = lint_and_fmt_tsv(tsv, fmt_tsv) {
            eprintln!("TSV_ERR {}: {e}", tsv.display());
            tsv_errors += 1;
        }
    }

    let mut extracted: BTreeMap<String, String> = BTreeMap::new();
    let mut findings = 0usize;
    for rs in &rs_files {
        let Ok(src) = fs::read_to_string(rs) else { continue };
        let (count, rewritten) = scan_rust_file(rs, &src, fix, &mut extracted);
        findings += count;
        if fix && rewritten != src {
            if let Err(e) = fs::write(rs, rewritten) {
                eprintln!("WRITE_ERR {}: {e}", rs.display());
            }
        }
    }

    if fix && !extracted.is_empty() {
        let cat_path = if catalog.is_absolute() { catalog } else { root.join(catalog) };
        if let Some(parent) = cat_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut lines = vec!["key\ten".to_string()];
        for (k, v) in &extracted {
            lines.push(format!("{k}\t{}", v.replace('\t', " ").replace('\n', "\\n")));
        }
        let _ = fs::write(&cat_path, lines.join("\n") + "\n");
    }

    println!(
        "{{\"rs_files\":{},\"tsv_files\":{},\"tsv_errors\":{},\"hardcoded_findings\":{},\"fixed\":{}}}",
        rs_files.len(),
        tsv_files.len(),
        tsv_errors,
        findings,
        fix
    );

    if tsv_errors > 0 {
        process::exit(1);
    }
}

fn walk(dir: &Path, rs: &mut Vec<PathBuf>, tsv: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let p = entry.path();
        if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
            if matches!(name, ".git" | "target" | "node_modules" | "third_party" | "vendor") {
                continue;
            }
        }
        if p.is_dir() {
            walk(&p, rs, tsv);
        } else if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
            match ext {
                "rs" => rs.push(p),
                "tsv" => tsv.push(p),
                _ => {}
            }
        }
    }
}

fn lint_and_fmt_tsv(path: &Path, fmt: bool) -> Result<(), String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut header_comments = Vec::new();
    let mut seen = BTreeSet::new();
    let mut rows: BTreeMap<(String, String), String> = BTreeMap::new();

    for (idx, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            if rows.is_empty() {
                header_comments.push(line.to_string());
            }
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() != 3 {
            return Err(format!("line {} has {} cols, expected 3 (context\\tsource\\ttranslation)", idx + 1, parts.len()));
        }
        let key = (parts[0].to_string(), parts[1].to_string());
        if !seen.insert(key.clone()) {
            return Err(format!("duplicate entry ({:?}, {:?}) at line {}", key.0, key.1, idx + 1));
        }
        rows.insert(key, line.to_string());
    }

    if fmt {
        let mut out = String::with_capacity(raw.len());
        for c in &header_comments {
            out.push_str(c);
            out.push('\n');
        }
        for row in rows.values() {
            out.push_str(row);
            out.push('\n');
        }
        if out != raw {
            fs::write(path, out).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn scan_rust_file(
    path: &Path,
    src: &str,
    fix: bool,
    extracted: &mut BTreeMap<String, String>,
) -> (usize, String) {
    let mut count = 0usize;
    let mut out = String::with_capacity(src.len());
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("msg");

    for (idx, line) in src.lines().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with("//") && (trimmed.contains("println!(\"") || trimmed.contains("eprintln!(\"")) {
            if let Some(start) = line.find("!(\"") {
                let rest = &line[start + 3..];
                if let Some(end) = rest.find('"') {
                    let literal = &rest[..end];
                    if !literal.is_empty() && !literal.starts_with('{') && literal.contains(' ') {
                        count += 1;
                        let key = format!("{stem}.L{}", idx + 1);
                        extracted.insert(key.clone(), literal.to_string());
                        if fix {
                            let replaced = line.replacen(
                                &format!("\"{literal}\""),
                                &format!("rust_i18n::t!(\"{key}\")"),
                                1,
                            );
                            out.push_str(&replaced);
                            out.push('\n');
                            continue;
                        }
                    }
                }
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    (count, out)
}
