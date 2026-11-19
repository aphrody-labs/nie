//! `cargo xtask app-pack --out <dist dir>`: builds the browser app into the folder that `crates/nie-app`
//! embeds (`NIE_APP_DIST=<dist dir>`, then `cargo build -p nie-cli`).
//!
//! Steps, each one a real tool run on the build host:
//!
//! 1. `cargo build --locked -p nie-wasm --target wasm32-unknown-unknown --release` with the
//!    `webgl` feature (WebGPU viewer plus its WebGL 2 fallback), paths remapped, one codegen unit,
//!    fat LTO;
//! 2. `wasm-bindgen --target web` at exactly the version `Cargo.lock` pins for the crate;
//! 3. `wasm-opt -Oz` (Binaryen), unless `--no-opt`;
//! 4. every file is renamed with the first eight hex digits of its BLAKE3, bottom-up, so a file's
//!    hash covers the names of the files it references;
//! 5. a brotli copy of each file it shrinks;
//! 6. `manifest.json`, one object per line, which `nie-app`'s build script reads.
//!
//! The same commit and the same tool versions give the same bytes: nothing here reads the clock,
//! the user name or an absolute path of the build host (see `docs/serve.md`).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::util::{capture, repo_root};

/// Arguments of `cargo xtask app-pack`.
#[derive(clap::Args)]
pub struct Args {
    /// nie checkout (default: this one).
    #[arg(long)]
    repo: Option<PathBuf>,
    /// Folder that receives the bundle.
    #[arg(long)]
    out: PathBuf,
    /// Cargo target directory (default: `<repo>/target/nie-app`).
    #[arg(long)]
    target_dir: Option<PathBuf>,
    /// Skip wasm-opt.
    #[arg(long)]
    no_opt: bool,
}

/// Version `Cargo.lock` pins for `package`.
pub fn locked_version(lock_path: &Path, package: &str) -> Result<String> {
    let lock = std::fs::read_to_string(lock_path)
        .with_context(|| lock_path.display().to_string())?
        .replace("\r\n", "\n");
    let marker = format!("name = \"{package}\"\nversion = \"");
    let at = lock
        .find(&marker)
        .with_context(|| format!("{package} is not in {}", lock_path.display()))?;
    let rest = &lock[at + marker.len()..];
    Ok(rest[..rest.find('"').context("malformed lock file")?].to_owned())
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

pub fn short_hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex()[..8].to_owned()
}

/// `stem.ext` becomes `stem.<hash>.ext`.
pub fn hashed_name(name: &str, bytes: &[u8]) -> String {
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    format!("{stem}.{}.{ext}", short_hash(bytes))
}

pub fn replace_once(text: &str, from: &str, to: &str) -> Result<String> {
    if text.matches(from).count() != 1 {
        bail!("expected exactly one `{from}` to rewrite");
    }
    Ok(text.replacen(from, to, 1))
}

/// Builds the bundle.
pub fn run(args: Args) -> Result<()> {
    let repo = args
        .repo
        .unwrap_or_else(repo_root)
        .canonicalize()
        .context("--repo")?;
    let target_dir = args
        .target_dir
        .clone()
        .unwrap_or_else(|| repo.join("target").join("nie-app"));
    let out = &args.out;

    // 1. The wasm module.
    let cargo_home = std::env::var("CARGO_HOME")
        .ok()
        .or_else(|| std::env::var("HOME").ok().map(|h| format!("{h}/.cargo")))
        .unwrap_or_default();
    let remap = format!(
        "--remap-path-prefix={}=/nie --remap-path-prefix={cargo_home}=/cargo",
        repo.display()
    );
    println!("build nie-wasm");
    capture(
        Command::new("cargo")
            .current_dir(&repo)
            .env("CARGO_TARGET_DIR", &target_dir)
            .env("RUSTFLAGS", &remap)
            .env("CARGO_PROFILE_RELEASE_OPT_LEVEL", "s")
            .env("CARGO_PROFILE_RELEASE_LTO", "fat")
            .env("CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "1")
            .env("CARGO_PROFILE_RELEASE_PANIC", "abort")
            .env("CARGO_PROFILE_RELEASE_DEBUG", "0")
            .args([
                "build",
                "--locked",
                "--release",
                "-p",
                "nie-wasm",
                "--lib",
                "--target",
                "wasm32-unknown-unknown",
                "--features",
                "webgl",
            ]),
    )?;
    let raw = target_dir.join("wasm32-unknown-unknown/release/nie_wasm.wasm");

    // 2. The glue, at the pinned version.
    let pinned = locked_version(&repo.join("Cargo.lock"), "wasm-bindgen")?;
    let installed = capture(Command::new("wasm-bindgen").arg("--version"))?;
    if installed.split_whitespace().nth(1) != Some(pinned.as_str()) {
        bail!("wasm-bindgen CLI `{installed}` does not match the {pinned} pinned in Cargo.lock");
    }
    let work = out.join(".work");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work)?;
    println!("wasm-bindgen {pinned}");
    capture(
        Command::new("wasm-bindgen")
            .args([
                "--target",
                "web",
                "--no-typescript",
                "--out-name",
                "nie_wasm",
                "--out-dir",
            ])
            .arg(&work)
            .arg(&raw),
    )?;

    // 3. Binaryen.
    let wasm_path = work.join("nie_wasm_bg.wasm");
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

    // 4. Hash bottom-up.
    let web = repo.join("crates/nie-app/web");
    let read = |path: &Path| std::fs::read(path).with_context(|| path.display().to_string());
    let text =
        |path: &Path| std::fs::read_to_string(path).with_context(|| path.display().to_string());
    let mut files: Vec<(String, Vec<u8>, bool)> = Vec::new();

    let wasm = read(&wasm_path)?;
    let wasm_name = hashed_name("nie_wasm_bg.wasm", &wasm);
    files.push((wasm_name.clone(), wasm, true));

    let glue = replace_once(
        &text(&work.join("nie_wasm.js"))?,
        "nie_wasm_bg.wasm",
        &wasm_name,
    )?;
    let glue_name = hashed_name("nie_wasm.js", glue.as_bytes());
    files.push((glue_name.clone(), glue.into_bytes(), true));

    let app = replace_once(
        &text(&web.join("app.js"))?,
        "./nie_wasm.js",
        &format!("./{glue_name}"),
    )?;
    let app_name = hashed_name("app.js", app.as_bytes());
    files.push((app_name.clone(), app.into_bytes(), true));

    let css = read(&web.join("app.css"))?;
    let css_name = hashed_name("app.css", &css);
    files.push((css_name.clone(), css, true));

    let index = text(&web.join("index.html"))?;
    let index = replace_once(&index, "href=\"app.css\"", &format!("href=\"{css_name}\""))?;
    let index = replace_once(&index, "src=\"app.js\"", &format!("src=\"{app_name}\""))?;
    files.push(("index.html".to_owned(), index.into_bytes(), false));

    // 5 and 6. Write, compress, list.
    let _ = std::fs::remove_dir_all(out);
    std::fs::create_dir_all(out)?;
    let mut manifest = String::from("{\"files\":[\n");
    for (position, (name, bytes, immutable)) in files.iter().enumerate() {
        std::fs::write(out.join(name), bytes)?;
        let packed = brotli_pack(bytes)?;
        if packed.len() * 10 < bytes.len() * 9 {
            std::fs::write(out.join(format!("{name}.br")), &packed)?;
        }
        let comma = if position + 1 < files.len() { "," } else { "" };
        let _ = writeln!(
            manifest,
            "{{\"name\":\"{name}\",\"type\":\"{}\",\"immutable\":{immutable},\"entry\":{},\"bytes\":{},\"brotli\":{},\"blake3\":\"{}\"}}{comma}",
            content_type(name),
            name == "index.html",
            bytes.len(),
            packed.len(),
            blake3::hash(bytes).to_hex()
        );
        println!("{name}  {} bytes  br {}", bytes.len(), packed.len());
    }
    manifest.push_str("]}\n");
    std::fs::write(out.join("manifest.json"), manifest)?;
    std::fs::remove_dir_all(&work).ok();
    println!("bundle in {}", out.display());
    Ok(())
}

/// Brotli at quality 11 with a 16 MiB window.
pub fn brotli_pack(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut packed = Vec::new();
    let params = brotli::enc::BrotliEncoderParams {
        quality: 11,
        lgwin: 24,
        ..Default::default()
    };
    brotli::BrotliCompress(&mut &bytes[..], &mut packed, &params).context("brotli")?;
    Ok(packed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_carry_the_blake3_prefix() {
        let name = hashed_name("app.js", b"abc");
        assert!(name.starts_with("app.") && name.ends_with(".js"));
        assert_eq!(name.len(), "app..js".len() + 8);
        assert_eq!(short_hash(b"abc"), blake3::hash(b"abc").to_hex()[..8]);
    }

    #[test]
    fn a_reference_is_rewritten_exactly_once() {
        assert_eq!(replace_once("a b", "a", "c").unwrap(), "c b");
        assert!(replace_once("a a", "a", "c").is_err());
        assert!(replace_once("x", "a", "c").is_err());
    }

    #[test]
    fn the_lock_version_is_read_with_either_line_ending() {
        let dir = tempfile::tempdir().unwrap();
        let lock = dir.path().join("Cargo.lock");
        std::fs::write(
            &lock,
            "[[package]]\r\nname = \"wasm-bindgen\"\r\nversion = \"0.2.130\"\r\n",
        )
        .unwrap();
        assert_eq!(locked_version(&lock, "wasm-bindgen").unwrap(), "0.2.130");
        assert!(locked_version(&lock, "absent").is_err());
    }

    #[test]
    fn brotli_round_trips_in_size() {
        let data = vec![7u8; 4096];
        assert!(brotli_pack(&data).unwrap().len() < data.len());
    }

    #[test]
    fn content_types_cover_the_bundle() {
        assert_eq!(content_type("a.wasm"), "application/wasm");
        assert_eq!(content_type("manifest.json"), "application/json");
    }
}
