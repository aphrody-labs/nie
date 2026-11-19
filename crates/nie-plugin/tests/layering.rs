//! Layering gate: the framework crates carry no knowledge of an application.
//!
//! `docs/specs/framework-layering.json` classifies every crate under `crates/`, `plugins/*/` and
//! `apps/` as framework or app. This test fails when
//! - a crate is not classified, or is classified twice;
//! - a framework crate names an application identifier in its sources, manifest or docs;
//! - a framework crate depends on an app crate.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

fn layering() -> Value {
    let text = fs::read_to_string(root().join("docs/specs/framework-layering.json"))
        .expect("docs/specs/framework-layering.json");
    serde_json::from_str(&text).expect("layering json")
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .expect("array")
        .iter()
        .map(|s| s.as_str().expect("string").to_owned())
        .collect()
}

/// Package name and normalised relative path of every crate present in the checkout.
fn crates_on_disk() -> BTreeMap<String, String> {
    let root = root();
    let mut dirs = Vec::new();
    for group in ["crates", "apps"] {
        if let Ok(rd) = fs::read_dir(root.join(group)) {
            dirs.extend(rd.flatten().map(|e| e.path()));
        }
    }
    if let Ok(rd) = fs::read_dir(root.join("plugins")) {
        for plugin in rd.flatten() {
            if let Ok(rd) = fs::read_dir(plugin.path()) {
                dirs.extend(rd.flatten().map(|e| e.path()));
            }
        }
    }
    let mut out = BTreeMap::new();
    for dir in dirs {
        let manifest = dir.join("Cargo.toml");
        let Ok(text) = fs::read_to_string(&manifest) else {
            continue;
        };
        let table: toml::Table = text.parse().expect("manifest");
        let name = table["package"]["name"].as_str().expect("name").to_owned();
        let rel = dir
            .strip_prefix(&root)
            .expect("inside root")
            .to_string_lossy()
            .replace('\\', "/");
        out.insert(name, rel);
    }
    out
}

fn files(dir: &Path, exts: &[String], out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            files(&path, exts, out);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| exts.iter().any(|x| x == e))
        {
            out.push(path);
        }
    }
}

fn nie_dependencies(manifest: &Path) -> Vec<String> {
    let table: toml::Table = fs::read_to_string(manifest)
        .expect("manifest")
        .parse()
        .expect("toml");
    let mut deps = Vec::new();
    let mut collect = |t: &toml::Table| {
        for key in ["dependencies", "build-dependencies", "dev-dependencies"] {
            if let Some(d) = t.get(key).and_then(|d| d.as_table()) {
                deps.extend(d.keys().filter(|k| k.starts_with("nie-")).cloned());
            }
        }
    };
    collect(&table);
    if let Some(targets) = table.get("target").and_then(|t| t.as_table()) {
        for t in targets.values().filter_map(|t| t.as_table()) {
            collect(t);
        }
    }
    deps
}

#[test]
fn every_crate_is_classified_once() {
    let doc = layering();
    let mut classified: BTreeMap<String, &str> = BTreeMap::new();
    for (layer, key) in [("framework", "framework"), ("app", "app")] {
        for entry in doc[key].as_array().expect("layer") {
            let pkg = entry["package"].as_str().expect("package").to_owned();
            assert!(
                classified.insert(pkg.clone(), layer).is_none(),
                "`{pkg}` is classified twice in docs/specs/framework-layering.json"
            );
        }
    }
    let on_disk = crates_on_disk();
    let missing: Vec<_> = on_disk
        .keys()
        .filter(|k| !classified.contains_key(*k))
        .collect();
    assert!(
        missing.is_empty(),
        "unclassified crates, add each to \"framework\" or \"app\" in docs/specs/framework-layering.json: {missing:?}"
    );
    for key in ["framework", "app"] {
        for entry in doc[key].as_array().expect("layer") {
            let pkg = entry["package"].as_str().expect("package");
            if let Some(rel) = on_disk.get(pkg) {
                assert_eq!(
                    rel,
                    entry["path"].as_str().expect("path"),
                    "`{pkg}` moved: update docs/specs/framework-layering.json"
                );
            }
        }
    }
}

#[test]
fn framework_crates_do_not_know_an_application() {
    let doc = layering();
    let identifiers: Vec<String> = strings(&doc["identifiers"])
        .into_iter()
        .map(|s| s.to_lowercase())
        .collect();
    let exts = strings(&doc["scanExtensions"]);
    let framework: Vec<String> = doc["framework"]
        .as_array()
        .expect("framework")
        .iter()
        .map(|e| e["package"].as_str().expect("package").to_owned())
        .collect();
    let root = root();
    let mut hits = Vec::new();
    for entry in doc["framework"].as_array().expect("framework") {
        let dir = root.join(entry["path"].as_str().expect("path"));
        if !dir.is_dir() {
            continue;
        }
        let mut list = Vec::new();
        files(&dir, &exts, &mut list);
        for file in list {
            let Ok(text) = fs::read_to_string(&file) else {
                continue;
            };
            let lower = text.to_lowercase();
            for id in &identifiers {
                if let Some(at) = lower.find(id.as_str()) {
                    let line = lower[..at].matches('\n').count() + 1;
                    hits.push(format!(
                        "{}:{line} names `{id}`",
                        file.strip_prefix(&root).unwrap_or(&file).display()
                    ));
                }
            }
        }
        for dep in nie_dependencies(&dir.join("Cargo.toml")) {
            if !framework.contains(&dep) {
                hits.push(format!(
                    "{} depends on app crate `{dep}`",
                    entry["package"].as_str().expect("package")
                ));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "framework crates must stay application-neutral:\n{}",
        hits.join("\n")
    );
}

#[test]
fn plans_name_classified_crates() {
    let doc = layering();
    let known: Vec<&str> = ["framework", "app"]
        .iter()
        .flat_map(|k| doc[*k].as_array().expect("layer"))
        .map(|e| e["package"].as_str().expect("package"))
        .collect();
    for plan in doc["plans"].as_array().expect("plans") {
        let pkg = plan["package"].as_str().expect("package");
        assert!(known.contains(&pkg), "plan for unclassified crate `{pkg}`");
    }
}
