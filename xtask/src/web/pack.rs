//! `web pack`: builds the browser build of the GPUI apps into a content-hashed bundle that `nie serve` embeds.
//!
//! 1. `cargo build --release --target wasm32-unknown-unknown -p nie-apps --lib` in `gui/` (gpui_web, WebGPU
//!    with the WebGL fallback chosen at runtime);
//! 2. `wasm-bindgen --target web` at the version `gui/Cargo.lock` pins;
//! 3. `wasm-opt -Oz` unless `--no-opt`;
//! 4. every file but the entry page is renamed with the first 8 hex digits of its SHA-256;
//! 5. a brotli copy of each file it shrinks, and `manifest.json` in the format `crates/nie-app` reads.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use crate::util::{capture, hex, repo_root};

/// Arguments of `web pack`.
#[derive(clap::Args, Debug)]
pub struct Args {
    /// Bundle directory (replaced).
    #[arg(long, default_value = "gui/dist")]
    pub out: PathBuf,
    /// Cargo target directory (default: `gui/target`).
    #[arg(long)]
    pub target_dir: Option<PathBuf>,
    /// Skip wasm-opt.
    #[arg(long)]
    pub no_opt: bool,
}

/// One file of the bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    pub name: String,
    pub bytes: Vec<u8>,
    pub immutable: bool,
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// `stem.ext` becomes `stem.<8 hex of sha256>.ext`.
pub fn hashed_name(name: &str, bytes: &[u8]) -> String {
    let short = &sha256_hex(bytes)[..8];
    match name.rsplit_once('.') {
        Some((stem, ext)) => format!("{stem}.{short}.{ext}"),
        None => name.to_owned(),
    }
}

pub fn content_type(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "wasm" => "application/wasm",
        "json" => "application/json",
        _ => "application/octet-stream",
    }
}

/// Version `Cargo.lock` text pins for `package`.
pub fn locked_version(lock: &str, package: &str) -> Option<String> {
    let lock = lock.replace("\r\n", "\n");
    let marker = format!("name = \"{package}\"\nversion = \"");
    let rest = &lock[lock.find(&marker)? + marker.len()..];
    Some(rest[..rest.find('"')?].to_owned())
}

/// The entry page that loads the hashed glue module.
pub fn index_page(glue_name: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, height=device-height, initial-scale=1\">\n\
         <title>nie explorer</title>\n<style>html,body{{margin:0;height:100%;background:#000}}</style>\n</head>\n\
         <body>\n<script type=\"module\">import init from \"./{glue_name}\"; await init();</script>\n</body>\n</html>\n"
    )
}

/// The wasm file and glue, renamed bottom-up so the glue hash covers the wasm name; plus the entry page.
pub fn bundle_files(wasm: Vec<u8>, glue: &str, wasm_source_name: &str) -> Result<Vec<File>> {
    let wasm_name = hashed_name(wasm_source_name, &wasm);
    if glue.matches(wasm_source_name).count() != 1 {
        bail!("expected exactly one reference to {wasm_source_name} in the glue");
    }
    let glue_text = glue.replacen(wasm_source_name, &wasm_name, 1);
    let glue_name = hashed_name("nie_gui.js", glue_text.as_bytes());
    let page = index_page(&glue_name);
    Ok(vec![
        File {
            name: wasm_name,
            bytes: wasm,
            immutable: true,
        },
        File {
            name: glue_name,
            bytes: glue_text.into_bytes(),
            immutable: true,
        },
        File {
            name: "index.html".to_owned(),
            bytes: page.into_bytes(),
            immutable: false,
        },
    ])
}

/// One manifest line per file: the format `crates/nie-app/build.rs` reads.
pub fn manifest_line(file: &File, brotli_len: usize, last: bool) -> String {
    format!(
        "{{\"name\":\"{}\",\"type\":\"{}\",\"immutable\":{},\"entry\":{},\"bytes\":{},\"brotli\":{},\"sha256\":\"{}\"}}{}\n",
        file.name,
        content_type(&file.name),
        file.immutable,
        file.name == "index.html",
        file.bytes.len(),
        brotli_len,
        sha256_hex(&file.bytes),
        if last { "" } else { "," }
    )
}

fn compress(bytes: &[u8]) -> Result<Vec<u8>> {
    let params = brotli::enc::BrotliEncoderParams {
        quality: 11,
        ..Default::default()
    };
    let mut packed = Vec::new();
    brotli::BrotliCompress(&mut &bytes[..], &mut packed, &params).context("brotli")?;
    Ok(packed)
}

/// Writes the files, their brotli copies when they shrink by more than 10 percent, and the manifest.
pub fn write_bundle(out: &Path, files: &[File]) -> Result<()> {
    let _ = std::fs::remove_dir_all(out);
    std::fs::create_dir_all(out)?;
    let mut manifest = String::from("{\"files\":[\n");
    for (i, file) in files.iter().enumerate() {
        std::fs::write(out.join(&file.name), &file.bytes)?;
        let packed = compress(&file.bytes)?;
        if packed.len() * 10 < file.bytes.len() * 9 {
            std::fs::write(out.join(format!("{}.br", file.name)), &packed)?;
        }
        manifest.push_str(&manifest_line(file, packed.len(), i + 1 == files.len()));
        println!(
            "{}  {} bytes  br {}",
            file.name,
            file.bytes.len(),
            packed.len()
        );
    }
    manifest.push_str("]}\n");
    std::fs::write(out.join("manifest.json"), manifest)?;
    Ok(())
}

pub fn run(args: &Args) -> Result<()> {
    let gui = repo_root().join("gui");
    let out = std::path::absolute(&args.out)?;
    let target_dir = std::path::absolute(
        args.target_dir
            .clone()
            .unwrap_or_else(|| gui.join("target")),
    )?;

    println!("build nie-apps for wasm32");
    crate::util::run(
        Command::new("cargo")
            .current_dir(&gui)
            .args([
                "build",
                "--locked",
                "--release",
                "-p",
                "nie-apps",
                "--lib",
                "--target",
                "wasm32-unknown-unknown",
            ])
            .env("CARGO_TARGET_DIR", &target_dir)
            .env("CARGO_PROFILE_RELEASE_OPT_LEVEL", "s")
            .env("CARGO_PROFILE_RELEASE_LTO", "fat")
            .env("CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "1")
            .env("CARGO_PROFILE_RELEASE_PANIC", "abort")
            .env("CARGO_PROFILE_RELEASE_DEBUG", "0"),
    )?;
    let raw = target_dir.join("wasm32-unknown-unknown/release/nie_apps.wasm");

    let lock = std::fs::read_to_string(gui.join("Cargo.lock")).context("gui/Cargo.lock")?;
    let pinned =
        locked_version(&lock, "wasm-bindgen").context("wasm-bindgen is not in gui/Cargo.lock")?;
    let installed = capture(Command::new("wasm-bindgen").arg("--version"))?;
    if installed.split_whitespace().nth(1) != Some(pinned.as_str()) {
        bail!("wasm-bindgen CLI `{installed}` does not match {pinned} in gui/Cargo.lock");
    }

    let work = out.join(".work");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work)?;
    capture(
        Command::new("wasm-bindgen")
            .args([
                "--target",
                "web",
                "--no-typescript",
                "--out-name",
                "nie_gui",
                "--out-dir",
            ])
            .arg(&work)
            .arg(&raw),
    )?;
    let wasm_path = work.join("nie_gui_bg.wasm");
    if !args.no_opt {
        println!("wasm-opt -Oz");
        let optimised = work.join("opt.wasm");
        capture(
            Command::new("wasm-opt")
                .args(["-Oz", "-all", "--strip-debug", "--strip-producers", "-o"])
                .arg(&optimised)
                .arg(&wasm_path),
        )?;
        std::fs::rename(&optimised, &wasm_path)?;
    }

    let wasm = std::fs::read(&wasm_path)?;
    let glue = std::fs::read_to_string(work.join("nie_gui.js"))?;
    let files = bundle_files(wasm, &glue, "nie_gui_bg.wasm")?;
    std::fs::remove_dir_all(&work)?;
    write_bundle(&out, &files)?;
    println!("bundle in {}", out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_carry_eight_hex_digits_of_the_sha256() {
        // sha256("abc") starts with ba7816bf.
        assert_eq!(hashed_name("nie_gui.js", b"abc"), "nie_gui.ba7816bf.js");
        assert_eq!(hashed_name("noext", b"abc"), "noext");
    }

    #[test]
    fn content_types_by_extension() {
        assert_eq!(content_type("a.wasm"), "application/wasm");
        assert_eq!(content_type("a.html"), "text/html; charset=utf-8");
        assert_eq!(content_type("a.bin"), "application/octet-stream");
    }

    #[test]
    fn the_lock_pins_a_version_for_a_package_with_either_line_ending() {
        let lock = "[[package]]\r\nname = \"wasm-bindgen\"\r\nversion = \"0.2.130\"\r\n";
        assert_eq!(
            locked_version(lock, "wasm-bindgen").as_deref(),
            Some("0.2.130")
        );
        assert_eq!(locked_version(lock, "other"), None);
    }

    #[test]
    fn the_glue_hash_covers_the_wasm_name_and_the_page_names_the_glue() {
        let files = bundle_files(
            b"wasm".to_vec(),
            "load('nie_gui_bg.wasm')",
            "nie_gui_bg.wasm",
        )
        .unwrap();
        assert_eq!(files.len(), 3);
        let wasm_name = &files[0].name;
        assert!(wasm_name.starts_with("nie_gui_bg.") && wasm_name.ends_with(".wasm"));
        let glue = String::from_utf8(files[1].bytes.clone()).unwrap();
        assert!(glue.contains(wasm_name.as_str()));
        assert_eq!(files[1].name, hashed_name("nie_gui.js", glue.as_bytes()));
        let page = String::from_utf8(files[2].bytes.clone()).unwrap();
        assert!(page.contains(&format!("./{}", files[1].name)));
        assert!(!files[2].immutable && files[0].immutable);
        assert!(bundle_files(vec![], "no reference", "nie_gui_bg.wasm").is_err());
        assert!(
            bundle_files(vec![], "nie_gui_bg.wasm nie_gui_bg.wasm", "nie_gui_bg.wasm").is_err()
        );
    }

    #[test]
    fn manifest_lines_are_one_object_each_in_the_format_nie_app_reads() {
        let file = File {
            name: "index.html".into(),
            bytes: b"abc".to_vec(),
            immutable: false,
        };
        let line = manifest_line(&file, 7, false);
        assert_eq!(
            line,
            "{\"name\":\"index.html\",\"type\":\"text/html; charset=utf-8\",\"immutable\":false,\"entry\":true,\"bytes\":3,\"brotli\":7,\"sha256\":\"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad\"},\n"
        );
        assert!(manifest_line(&file, 7, true).ends_with("\"}\n"));
        assert!(
            serde_json::from_str::<serde_json::Value>(line.trim_end().trim_end_matches(','))
                .is_ok()
        );
    }

    #[test]
    fn a_bundle_is_written_with_brotli_only_where_it_shrinks() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("dist");
        let files = vec![
            File {
                name: "big.js".into(),
                bytes: vec![b'a'; 4096],
                immutable: true,
            },
            File {
                name: "index.html".into(),
                bytes: b"<p>x</p>".to_vec(),
                immutable: false,
            },
        ];
        write_bundle(&out, &files).unwrap();
        assert!(out.join("big.js.br").is_file());
        assert!(!out.join("index.html.br").is_file());
        let manifest = std::fs::read_to_string(out.join("manifest.json")).unwrap();
        assert!(manifest.starts_with("{\"files\":[\n"));
        let parsed: serde_json::Value = serde_json::from_str(&manifest).unwrap();
        assert_eq!(parsed["files"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["files"][1]["entry"], true);
    }
}
