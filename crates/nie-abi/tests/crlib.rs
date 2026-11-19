//! The crlib scheme holds for the whole workspace: every member is listed in `crlib.json`, every
//! crlib builds as `rlib`, `staticlib` and `cdylib` with a `crlib` feature, and the committed
//! headers and Rust layers equal what the generator writes.

use nie_abi::crlib::{self, Status, Surface};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn workspace_packages() -> Vec<Value> {
    let out = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--offline",
        ])
        .current_dir(root())
        .output()
        .expect("cargo metadata");
    assert!(
        out.status.success(),
        "cargo metadata: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let metadata: Value = serde_json::from_slice(&out.stdout).expect("metadata json");
    let members: Vec<String> = metadata["workspace_members"]
        .as_array()
        .expect("members")
        .iter()
        .map(|m| m.as_str().expect("id").to_owned())
        .collect();
    metadata["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .filter(|p| {
            members
                .iter()
                .any(|m| m == p["id"].as_str().unwrap_or_default())
        })
        .cloned()
        .collect()
}

fn library_target(package: &Value) -> Option<&Value> {
    package["targets"].as_array()?.iter().find(|t| {
        t["kind"].as_array().is_some_and(|kinds| {
            kinds
                .iter()
                .any(|k| matches!(k.as_str(), Some("lib" | "rlib" | "cdylib" | "staticlib")))
        })
    })
}

#[test]
fn committed_headers_and_layers_match_the_generator() {
    let manifest = crlib::load_manifest(crlib::MANIFEST_JSON);
    let files = crlib::generated_files(&root(), &manifest).expect("abi.json");
    assert!(!files.is_empty());
    for (rel, text) in files {
        let on_disk = std::fs::read_to_string(root().join(&rel))
            .unwrap_or_else(|e| panic!("{}: {e}", rel.display()));
        assert_eq!(
            on_disk.replace("\r\n", "\n"),
            text,
            "{} drifted: run `cargo run -p nie-abi -- --crlib .`",
            rel.display(),
        );
    }
}

#[test]
fn the_manifest_lists_every_workspace_member_exactly() {
    let manifest = crlib::load_manifest(crlib::MANIFEST_JSON);
    let base = root().canonicalize().expect("root");
    let mut members: Vec<(String, String)> = workspace_packages()
        .iter()
        .map(|p| {
            let manifest_path = PathBuf::from(p["manifest_path"].as_str().expect("path"));
            let dir = manifest_path.parent().expect("dir");
            let rel = dir
                .canonicalize()
                .ok()
                .and_then(|d| d.strip_prefix(&base).map(Path::to_path_buf).ok())
                .or_else(|| dir.strip_prefix(&base).map(Path::to_path_buf).ok())
                .unwrap_or_default();
            (
                p["name"].as_str().expect("name").to_owned(),
                rel.to_string_lossy().replace('\\', "/"),
            )
        })
        .collect();
    members.sort();
    let listed: Vec<(String, String)> = manifest
        .crates
        .iter()
        .map(|e| (e.package.clone(), e.path.clone()))
        .collect();
    for member in &members {
        assert!(
            listed.contains(member),
            "{} ({}) is a workspace member missing from crates/nie-abi/crlib.json: add it as `crlib`, or as `excluded` with a reason",
            member.0,
            member.1
        );
    }
    // A listed crate that is not a member must not exist on disk; one that is not there yet (work
    // landing in another change) is accepted.
    for entry in &listed {
        assert!(
            members.contains(entry) || !root().join(&entry.1).join("Cargo.toml").is_file(),
            "{} ({}) is listed in crlib.json but is not a workspace member",
            entry.0,
            entry.1
        );
    }
}

#[test]
fn every_crlib_builds_as_rlib_staticlib_and_cdylib_with_a_crlib_feature() {
    let manifest = crlib::load_manifest(crlib::MANIFEST_JSON);
    let packages = workspace_packages();
    for entry in manifest.crates.iter().filter(|e| e.status == Status::Crlib) {
        let package = packages
            .iter()
            .find(|p| p["name"] == entry.package.as_str())
            .unwrap_or_else(|| panic!("{} is not a workspace member", entry.package));
        let lib = library_target(package)
            .unwrap_or_else(|| panic!("{} has no library target", entry.package));
        let types: Vec<&str> = lib["crate_types"]
            .as_array()
            .expect("crate_types")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        for want in ["rlib", "staticlib", "cdylib"] {
            assert!(
                types.contains(&want),
                "{}: crate-type must list rlib, staticlib and cdylib, found {types:?}",
                entry.package
            );
        }
        if entry.surface == Surface::Generated {
            assert!(
                package["features"].get("crlib").is_some(),
                "{}: missing the `crlib` feature",
                entry.package
            );
            let lib_rs = std::fs::read_to_string(root().join(&entry.path).join("src/lib.rs"))
                .expect("lib.rs");
            assert!(
                lib_rs.contains("mod abi_generated;"),
                "{}: lib.rs must declare `mod abi_generated;` behind the crlib feature",
                entry.package
            );
            assert!(
                root().join(crlib::header_path(&entry.id)).is_file(),
                "{}: header missing",
                entry.package
            );
        }
    }
}

#[test]
fn symbols_are_unique_across_the_workspace() {
    let manifest = crlib::load_manifest(crlib::MANIFEST_JSON);
    let mut seen = std::collections::BTreeMap::new();
    for entry in manifest
        .crates
        .iter()
        .filter(|e| e.status == Status::Crlib && e.surface == Surface::Generated)
    {
        let text =
            std::fs::read_to_string(root().join(&entry.path).join("abi.json")).expect("abi.json");
        let abi = crlib::load_abi(&text, entry);
        for f in &abi.functions {
            if let Some(other) = seen.insert(f.name.clone(), entry.package.clone()) {
                panic!("{} is exported by {} and {}", f.name, other, entry.package);
            }
        }
    }
}
