//! The checked-in contract files match the generator, and every command names a real binary.

use std::path::{Path, PathBuf};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn checked_in_files_are_current() {
    let root = workspace().join("contract");
    assert_eq!(
        std::fs::read_to_string(root.join("nie-app.json")).unwrap(),
        nie_shell::contract_json(),
        "run `cargo run -p nie-shell -- contract > contract/nie-app.json`"
    );
}

#[test]
fn every_command_binary_exists_in_its_package() {
    for command in nie_shell::contract().commands {
        let manifest = workspace()
            .join("crates")
            .join(command.package)
            .join("Cargo.toml");
        let text = std::fs::read_to_string(&manifest)
            .unwrap_or_else(|error| panic!("{}: {error}", manifest.display()));
        let table: toml::Table = text.parse().unwrap();
        let package = table["package"]["name"].as_str().unwrap();
        let bins: Vec<&str> = table
            .get("bin")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|bin| bin.get("name").and_then(toml::Value::as_str))
            .collect();
        assert!(
            bins.contains(&command.binary) || package == command.binary,
            "{} has no binary {}",
            command.package,
            command.binary
        );
    }
}
