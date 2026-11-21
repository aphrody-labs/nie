//! Embeds the bundle that `cargo xtask app-pack` wrote.
//!
//! The bundle is a folder of content-hashed files and a `manifest.json` (name, content type,
//! immutable flag, one line per file). It is read from `$NIE_APP_DIST`, else `dist/` next to this
//! manifest; with neither, the crate embeds an empty bundle and `nie serve` answers `/app` with
//! an explanation. The folder never enters git: it is built from the sources of the same commit
//! on the build host and in CI.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=NIE_APP_DIST");
    let manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let dist = std::env::var_os("NIE_APP_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("dist"));
    println!("cargo:rerun-if-changed={}", dist.display());

    let mut out = String::from("pub(crate) static FILES: &[Asset] = &[\n");
    let mut entry = String::new();
    if let Ok(text) = std::fs::read_to_string(dist.join("manifest.json")) {
        println!(
            "cargo:rerun-if-changed={}",
            dist.join("manifest.json").display()
        );
        for line in text.lines() {
            // One object per line: {"name","type","immutable","entry","bytes","brotli","blake3"}
            let field = |key: &str| -> Option<String> {
                let at = line.find(&format!("\"{key}\":"))? + key.len() + 3;
                let rest = line[at..].trim_start();
                if let Some(quoted) = rest.strip_prefix('"') {
                    Some(quoted[..quoted.find('"')?].to_owned())
                } else {
                    Some(rest[..rest.find([',', '}'])?].trim().to_owned())
                }
            };
            let (Some(name), Some(kind)) = (field("name"), field("type")) else {
                continue;
            };
            let immutable = field("immutable").as_deref() == Some("true");
            let is_entry = field("entry").as_deref() == Some("true");
            let plain = dist.join(&name);
            let packed = dist.join(format!("{name}.br"));
            println!("cargo:rerun-if-changed={}", plain.display());
            let brotli = if packed.is_file() {
                println!("cargo:rerun-if-changed={}", packed.display());
                format!("Some(include_bytes!({:?}))", display(&packed))
            } else {
                "None".to_owned()
            };
            let _ = writeln!(
                out,
                "    Asset {{ name: {name:?}, content_type: {kind:?}, immutable: {immutable}, bytes: include_bytes!({:?}), brotli: {brotli} }},",
                display(&plain)
            );
            if is_entry {
                entry = name;
            }
        }
    }
    out.push_str("];\n");
    let _ = writeln!(out, "pub(crate) const ENTRY: &str = {entry:?};");
    let target = PathBuf::from(std::env::var_os("OUT_DIR").expect("out dir")).join("bundle.rs");
    std::fs::write(target, out).expect("write bundle.rs");
}

/// An absolute path in the form `include_bytes!` accepts on every platform.
fn display(path: &Path) -> String {
    let absolute = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    absolute
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('\\', "/")
}
