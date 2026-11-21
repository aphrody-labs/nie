//! `cargo xtask map [--check]`: writes `MAP.tsv` at the repository root, one line per unit with the exact
//! file count and byte size, the tier and the gate command, so an agent reads the structure in one call.
//!
//! The listing is `git ls-files --cached --others --exclude-standard`, sorted, so the output is a pure
//! function of the tree. `--check` fails when the committed `MAP.tsv` differs.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::util::repo_root;

/// Arguments of `cargo xtask map`.
#[derive(clap::Args)]
pub struct Args {
    /// Fail when MAP.tsv is stale instead of rewriting it.
    #[arg(long)]
    check: bool,
}

/// Unit key of a path: the folder an agent reasons about.
fn unit(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    let depth = match parts[0] {
        "plugins" => 3,
        "crates" | "apps" | "tools" => 2,
        _ => 1,
    };
    if parts.len() <= depth {
        return "(root)".to_owned();
    }
    parts[..depth].join("/")
}

/// Framework or app, read from `docs/specs/framework-layering.json` (the `framework` array precedes
/// the `app` array), by the position of the `"package"` entry.
fn layer(layering: &str, package: &str) -> &'static str {
    let key = format!("\"package\": \"{package}\"");
    let (Some(at), Some(app_start)) = (layering.find(&key), layering.find("\"app\": [")) else {
        return "unclassified";
    };
    if at < app_start { "framework" } else { "app" }
}

fn tier_and_gate(unit: &str, layering: &str) -> (String, String) {
    let name = unit.rsplit('/').next().unwrap_or(unit);
    let top = unit.split('/').next().unwrap_or(unit);
    let (tier, gate) = match top {
        "crates" | "apps" => (
            if top == "apps" {
                "app"
            } else {
                layer(layering, name)
            },
            format!("cargo nextest run -p {name}"),
        ),
        "plugins" => ("plugin", format!("cargo nextest run -p {name}")),
        "xtask" => ("tool", "cargo nextest run -p nie-xtask".to_owned()),
        "tools" if name == "nie-mcp-launch" => (
            "tool",
            "cargo test --manifest-path tools/nie-mcp-launch/Cargo.toml".to_owned(),
        ),
        "tools" => ("tool", "none".to_owned()),
        "bindings" => ("generated", "nie bindgen --check".to_owned()),
        "contract" => ("generated", "cargo test -p nie-shell".to_owned()),
        "include" => ("generated", "cargo test -p nie-abi".to_owned()),
        "docs" => ("doc", "none".to_owned()),
        "deploy" => ("deploy", "cargo nextest run -p nie-deploy".to_owned()),
        "gui" => ("gui", "fr1 only: just ci-gui-linux".to_owned()),
        "tests" => ("test", "just ci-bindings".to_owned()),
        _ => ("meta", "none".to_owned()),
    };
    (tier.to_owned(), gate)
}

/// The text of `MAP.tsv` for the tree under `root`.
fn render(root: &Path) -> Result<String> {
    let out = Command::new("git")
        .current_dir(root)
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .output()
        .context("git ls-files")?;
    if !out.status.success() {
        bail!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let listing = String::from_utf8_lossy(&out.stdout).into_owned();
    let layering = std::fs::read_to_string(root.join("docs/specs/framework-layering.json"))
        .unwrap_or_default();
    let mut units: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for path in listing.split('\0').filter(|p| !p.is_empty()) {
        if !seen.insert(path.to_owned()) {
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(root.join(path)) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let entry = units.entry(unit(path)).or_default();
        entry.0 += 1;
        entry.1 += meta.len();
    }
    let mut text = String::from("unit\tfiles\tbytes\ttier\tgate\n");
    for (unit, (files, bytes)) in &units {
        let (tier, gate) = tier_and_gate(unit, &layering);
        let _ = writeln!(text, "{unit}\t{files}\t{bytes}\t{tier}\t{gate}");
    }
    Ok(text)
}

/// Writes or checks `MAP.tsv`.
pub fn run(args: Args) -> Result<()> {
    let root = repo_root();
    let text = render(&root)?;
    let path = root.join("MAP.tsv");
    if args.check {
        let committed = std::fs::read_to_string(&path).unwrap_or_default();
        if committed == text {
            return Ok(());
        }
        bail!("MAP.tsv is stale, run `just map`");
    }
    std::fs::write(&path, &text).with_context(|| format!("write {}", path.display()))?;
    println!(
        "MAP.tsv: {} lines, {} bytes",
        text.lines().count(),
        text.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_follow_the_folder_depth() {
        assert_eq!(unit("crates/nie-cli/src/main.rs"), "crates/nie-cli");
        assert_eq!(unit("plugins/exe/nie-pe/Cargo.toml"), "plugins/exe/nie-pe");
        assert_eq!(unit("xtask/src/main.rs"), "xtask");
        assert_eq!(unit("Cargo.toml"), "(root)");
        assert_eq!(unit("justfile"), "(root)");
    }

    #[test]
    fn layers_come_from_the_position_in_the_layering_json() {
        let json = "{\"framework\": [{\n \"package\": \"a\"}], \"app\": [{\n \"package\": \"b\"}]}";
        assert_eq!(layer(json, "a"), "framework");
        assert_eq!(layer(json, "b"), "app");
        assert_eq!(layer(json, "c"), "unclassified");
    }

    #[test]
    fn gates_name_the_just_recipes() {
        assert_eq!(tier_and_gate("tests", "").1, "just ci-bindings");
        assert_eq!(tier_and_gate("xtask", "").0, "tool");
    }
}
