//! `cargo xtask repro`: reproducible builds, proven.
//!
//! The same commit is exported twice (`git archive`) into two directories of different depth and
//! length, built with a fixed environment, and every release artifact is hashed with BLAKE3. The
//! build is reproducible when both trees yield the same bytes for every artifact; the result is
//! written as a release manifest (toolchain, `Cargo.lock` hash, artifact hashes).
//!
//! The fixed environment:
//! - `SOURCE_DATE_EPOCH` is the commit time;
//! - `--remap-path-prefix` maps the checkout, `CARGO_HOME`, `RUSTUP_HOME` and `$HOME` to fixed names;
//! - `--locked --release`, `CARGO_INCREMENTAL=0`, `TZ=UTC`, `LC_ALL=C`;
//! - every other source of rustflags is cleared, and a `target-cpu=native` anywhere is refused.

use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context as _, bail};
use serde::{Deserialize, Serialize};

use crate::util::capture as run_capture;

/// Arguments of `cargo xtask repro`.
#[derive(Debug, Clone, clap::Args)]
pub struct Args {
    /// Packages to build (default: the framework tools).
    #[arg(short = 'p', long = "package")]
    pub packages: Vec<String>,
    /// Git checkout (default: the checkout above the current directory).
    #[arg(long)]
    pub root: Option<PathBuf>,
    /// Revision to build.
    #[arg(long, default_value = "HEAD")]
    pub rev: String,
    /// Directory that receives the two build trees (default: a temporary directory).
    #[arg(long)]
    pub work: Option<PathBuf>,
    /// Manifest written when the builds agree (relative to the checkout).
    #[arg(long, default_value = "docs/specs/repro-manifest.json")]
    pub manifest: PathBuf,
    /// Build for this target triple.
    #[arg(long)]
    pub target: Option<String>,
    /// Keep the build trees.
    #[arg(long)]
    pub keep: bool,
}

/// Default packages: the framework tools.
pub const DEFAULT_PACKAGES: [&str; 3] = ["nie-bindgen", "nie-scaffold", "hello"];

/// One built artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    /// Cargo package.
    pub package: String,
    /// Cargo target name.
    pub name: String,
    /// `bin`, `cdylib` or `staticlib`.
    pub kind: String,
    /// File name.
    pub file: String,
    /// Size in bytes.
    pub size: u64,
    /// BLAKE3 of the file.
    pub blake3: String,
}

/// The release manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Manifest format.
    pub schema: u32,
    /// Commit built.
    pub commit: String,
    /// `SOURCE_DATE_EPOCH` used.
    pub source_date_epoch: u64,
    /// `rustc -vV` lines.
    pub rustc: Vec<String>,
    /// `cargo -V`.
    pub cargo: String,
    /// BLAKE3 of `Cargo.lock`.
    pub cargo_lock_blake3: String,
    /// Target triple (empty: the host).
    pub target: String,
    /// Rustflags in the form they were passed, paths replaced by placeholders.
    pub rustflags: Vec<String>,
    /// Number of independent builds that produced identical artifacts.
    pub builds_compared: usize,
    /// Artifacts, sorted by package, name and kind.
    pub artifacts: Vec<Artifact>,
}

/// The checkout above `start`.
///
/// # Errors
/// When no `.git` directory is found.
pub fn find_root(start: &Path) -> anyhow::Result<PathBuf> {
    start
        .ancestors()
        .find(|d| d.join(".git").exists() && d.join("Cargo.toml").is_file())
        .map(Path::to_path_buf)
        .context("no git checkout with a Cargo.toml above the current directory")
}

/// Refuses `target-cpu=native` in a config file or the environment: a build that depends on the
/// host CPU cannot be reproduced elsewhere.
///
/// # Errors
/// When one of the inputs asks for native codegen.
pub fn refuse_native(texts: &[(String, String)]) -> anyhow::Result<()> {
    for (origin, text) in texts {
        let squashed: String = text
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '"')
            .collect();
        if squashed.contains("target-cpu=native") {
            bail!("{origin} asks for target-cpu=native, which makes the build host dependent");
        }
    }
    Ok(())
}

/// The rustflags of a build: remaps from the broadest prefix to the most specific one (rustc
/// applies the last matching rule).
#[must_use]
pub fn remap_flags(home: &Path, cargo_home: &Path, rustup_home: &Path, tree: &Path) -> Vec<String> {
    let mut prefixes = vec![
        (home, "/home/build"),
        (cargo_home, "/cargo"),
        (rustup_home, "/rustup"),
        (tree, "/nie"),
    ];
    prefixes.dedup_by(|a, b| a.0 == b.0);
    prefixes
        .into_iter()
        .map(|(p, to)| format!("--remap-path-prefix={}={to}", p.display()))
        .collect()
}

/// Exports `rev` of `root` into `dir` (created).
///
/// # Errors
/// When git or tar fails.
pub fn export_tree(root: &Path, rev: &str, dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut git = Command::new("git")
        .args(["-C"])
        .arg(root)
        .args(["archive", "--format=tar", rev])
        .stdout(Stdio::piped())
        .spawn()
        .context("git archive")?;
    let tar = Command::new("tar")
        .arg("-x")
        .arg("-C")
        .arg(dir)
        .stdin(git.stdout.take().context("git stdout")?)
        .status()
        .context("tar")?;
    if !git.wait()?.success() || !tar.success() {
        bail!("exporting {rev} into {} failed", dir.display());
    }
    Ok(())
}

struct Build {
    artifacts: Vec<Artifact>,
}

fn hash_file(path: &Path) -> anyhow::Result<(String, u64)> {
    let mut f = std::fs::File::open(path).with_context(|| path.display().to_string())?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut size = 0u64;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        size += n as u64;
    }
    Ok((hasher.finalize().to_hex().to_string(), size))
}

fn build(tree: &Path, args: &Args, epoch: u64, flags: &[String]) -> anyhow::Result<Build> {
    let target_dir = tree.join("target");
    let mut cmd = Command::new("cargo");
    cmd.current_dir(tree)
        .args([
            "build",
            "--release",
            "--locked",
            "--message-format=json-render-diagnostics",
        ])
        .arg("--target-dir")
        .arg(&target_dir);
    if let Some(t) = &args.target {
        cmd.args(["--target", t]);
    }
    for p in &args.packages {
        cmd.args(["-p", p]);
    }
    for (k, _) in std::env::vars_os().filter_map(|(k, v)| Some((k.into_string().ok()?, v))) {
        if k.starts_with("CARGO_TARGET_") && k.ends_with("_RUSTFLAGS")
            || k == "RUSTFLAGS"
            || k == "CARGO_BUILD_RUSTFLAGS"
        {
            cmd.env_remove(k);
        }
    }
    cmd.env("SOURCE_DATE_EPOCH", epoch.to_string())
        .env("CARGO_ENCODED_RUSTFLAGS", flags.join("\x1f"))
        .env("CARGO_INCREMENTAL", "0")
        .env("TZ", "UTC")
        .env("LC_ALL", "C")
        .env_remove("CARGO_TARGET_DIR")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let mut child = cmd.spawn().context("cargo build")?;
    let mut json = String::new();
    child
        .stdout
        .take()
        .context("stdout")?
        .read_to_string(&mut json)?;
    if !child.wait()?.success() {
        bail!("cargo build failed in {}", tree.display());
    }
    let mut artifacts = Vec::new();
    for line in json.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v["reason"] != "compiler-artifact" {
            continue;
        }
        let name = v["package_id"].as_str().unwrap_or_default();
        let Some(package) = args.packages.iter().find(|p| package_matches(name, p)) else {
            continue;
        };
        let kinds: Vec<&str> = v["target"]["kind"]
            .as_array()
            .map(|k| k.iter().filter_map(|k| k.as_str()).collect())
            .unwrap_or_default();
        let wanted: Vec<&str> = kinds
            .iter()
            .copied()
            .filter(|k| matches!(*k, "bin" | "cdylib" | "staticlib"))
            .collect();
        if wanted.is_empty() {
            continue;
        }
        let files: Vec<&str> = v["filenames"]
            .as_array()
            .map(|f| f.iter().filter_map(|f| f.as_str()).collect())
            .unwrap_or_default();
        for file in files {
            let path = Path::new(file);
            let Some(file_name) = path.file_name().and_then(|f| f.to_str()) else {
                continue;
            };
            // Only the final artifacts: not `.d`, `.rmeta` or hard links under `deps/`.
            if path
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|d| d == "deps")
            {
                continue;
            }
            let (blake3, size) = hash_file(path)?;
            artifacts.push(Artifact {
                package: package.clone(),
                name: v["target"]["name"].as_str().unwrap_or_default().to_owned(),
                kind: wanted[0].to_owned(),
                file: file_name.to_owned(),
                size,
                blake3,
            });
        }
    }
    artifacts.sort_by(|a, b| {
        (&a.package, &a.name, &a.kind, &a.file).cmp(&(&b.package, &b.name, &b.kind, &b.file))
    });
    artifacts.dedup();
    Ok(Build { artifacts })
}

/// Whether a cargo package id (`path+file:///x/crates/nie-abi#2.0.0`, `path+file:///x/apps/hello#0.1.0`,
/// or `name 0.1.0 (path+...)`) names package `name`.
fn package_matches(id: &str, name: &str) -> bool {
    if let Some((_, frag)) = id.rsplit_once('#') {
        // `#name@version` or `#version`
        if let Some((n, _)) = frag.split_once('@') {
            return n == name;
        }
        return id.rsplit_once('#').and_then(|(l, _)| l.rsplit('/').next()) == Some(name);
    }
    id.split_whitespace().next() == Some(name)
}

/// Compares two builds by artifact identity.
///
/// Returns the differences as readable lines; empty when identical.
#[must_use]
pub fn diff(a: &[Artifact], b: &[Artifact]) -> Vec<String> {
    let key = |x: &Artifact| {
        (
            x.package.clone(),
            x.name.clone(),
            x.kind.clone(),
            x.file.clone(),
        )
    };
    let ma: BTreeMap<_, _> = a.iter().map(|x| (key(x), x)).collect();
    let mb: BTreeMap<_, _> = b.iter().map(|x| (key(x), x)).collect();
    let mut out = Vec::new();
    for (k, x) in &ma {
        match mb.get(k) {
            None => out.push(format!("{}: only in the first build", k.3)),
            Some(y) if y.blake3 != x.blake3 => {
                out.push(format!("{}: {} != {}", k.3, x.blake3, y.blake3))
            }
            Some(_) => {}
        }
    }
    for k in mb.keys().filter(|k| !ma.contains_key(*k)) {
        out.push(format!("{}: only in the second build", k.3));
    }
    out
}

/// Runs the command; fails on a build failure, a refused configuration, or when the two builds differ.
pub fn run(mut args: Args) -> anyhow::Result<()> {
    if args.packages.is_empty() {
        args.packages = DEFAULT_PACKAGES.map(str::to_owned).to_vec();
    }
    let root = match &args.root {
        Some(r) => r.clone(),
        None => find_root(&std::env::current_dir()?)?,
    };
    let commit = run_capture(
        Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["rev-parse", &args.rev]),
    )?;
    let epoch: u64 = run_capture(Command::new("git").arg("-C").arg(&root).args([
        "log",
        "-1",
        "--format=%ct",
        &commit,
    ]))?
    .parse()?;
    let home = PathBuf::from(
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .context("HOME")?,
    );
    let cargo_home =
        std::env::var_os("CARGO_HOME").map_or_else(|| home.join(".cargo"), PathBuf::from);
    let rustup_home =
        std::env::var_os("RUSTUP_HOME").map_or_else(|| home.join(".rustup"), PathBuf::from);

    let work = match &args.work {
        Some(w) => w.clone(),
        None => std::env::temp_dir().join(format!("nie-repro-{}", std::process::id())),
    };
    let trees = [
        work.join("a").join("src"),
        work.join("second-build").join("deeper").join("checkout"),
    ];
    for t in &trees {
        export_tree(&root, &commit, t)?;
    }

    let mut inputs = Vec::new();
    for t in &trees {
        let p = t.join(".cargo/config.toml");
        if let Ok(s) = std::fs::read_to_string(&p) {
            inputs.push((p.display().to_string(), s));
        }
    }
    let user_cfg = cargo_home.join("config.toml");
    if let Ok(s) = std::fs::read_to_string(&user_cfg) {
        inputs.push((user_cfg.display().to_string(), s));
    }
    for k in [
        "RUSTFLAGS",
        "CARGO_BUILD_RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
    ] {
        if let Ok(v) = std::env::var(k) {
            inputs.push((format!("${k}"), v));
        }
    }
    refuse_native(&inputs)?;

    let mut builds = Vec::new();
    for t in &trees {
        let flags = remap_flags(&home, &cargo_home, &rustup_home, t);
        eprintln!("building {} in {}", args.packages.join(" "), t.display());
        builds.push(build(t, &args, epoch, &flags)?);
    }
    let differences = diff(&builds[0].artifacts, &builds[1].artifacts);
    if !differences.is_empty() {
        bail!("not reproducible:\n{}", differences.join("\n"));
    }
    if builds[0].artifacts.is_empty() {
        bail!("no artifact was built for {}", args.packages.join(", "));
    }

    let rustc: Vec<String> = run_capture(Command::new("rustc").current_dir(&trees[0]).arg("-vV"))?
        .lines()
        .map(str::to_owned)
        .collect();
    let cargo = run_capture(Command::new("cargo").current_dir(&trees[0]).arg("-V"))?;
    let lock = std::fs::read(trees[0].join("Cargo.lock")).context("Cargo.lock")?;
    let placeholders = ["$HOME", "$CARGO_HOME", "$RUSTUP_HOME", "$CHECKOUT"];
    let rustflags = remap_flags(
        Path::new(placeholders[0]),
        Path::new(placeholders[1]),
        Path::new(placeholders[2]),
        Path::new(placeholders[3]),
    );
    let manifest = Manifest {
        schema: 1,
        commit,
        source_date_epoch: epoch,
        rustc,
        cargo,
        cargo_lock_blake3: blake3::hash(&lock).to_hex().to_string(),
        target: args.target.clone().unwrap_or_default(),
        rustflags,
        builds_compared: builds.len(),
        artifacts: builds.remove(0).artifacts,
    };
    let path = root.join(&args.manifest);
    std::fs::create_dir_all(path.parent().context("parent")?)?;
    let mut text = serde_json::to_string_pretty(&manifest)?;
    text.push('\n');
    std::fs::write(&path, text)?;
    for a in &manifest.artifacts {
        println!("{}  {}  ({} bytes)", a.blake3, a.file, a.size);
    }
    println!(
        "reproducible: {} artifacts identical across {} builds; manifest {}",
        manifest.artifacts.len(),
        manifest.builds_compared,
        path.display()
    );
    if !args.keep {
        let _ = std::fs::remove_dir_all(&work);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn art(file: &str, hash: &str) -> Artifact {
        Artifact {
            package: "p".into(),
            name: "n".into(),
            kind: "bin".into(),
            file: file.into(),
            size: 1,
            blake3: hash.into(),
        }
    }

    #[test]
    fn native_codegen_is_refused() {
        assert!(
            refuse_native(&[(
                "c".into(),
                "rustflags = [\"-C\", \"target-cpu=native\"]".into()
            )])
            .is_err()
        );
        assert!(refuse_native(&[("e".into(), "-C target-cpu = native".into())]).is_err());
        assert!(refuse_native(&[("c".into(), "target-dir = \"x\"".into())]).is_ok());
    }

    #[test]
    fn remaps_go_from_broad_to_specific() {
        let f = remap_flags(
            Path::new("/home/u"),
            Path::new("/home/u/.cargo"),
            Path::new("/home/u/.rustup"),
            Path::new("/t/src"),
        );
        assert_eq!(
            f.first().unwrap(),
            "--remap-path-prefix=/home/u=/home/build"
        );
        assert_eq!(f.last().unwrap(), "--remap-path-prefix=/t/src=/nie");
        assert_eq!(f.len(), 4);
    }

    #[test]
    fn diff_reports_changes_and_missing_files() {
        let a = [art("x", "1"), art("y", "2")];
        let b = [art("x", "1"), art("y", "3"), art("z", "4")];
        let d = diff(&a, &b);
        assert_eq!(d.len(), 2, "{d:?}");
        assert!(diff(&a, &a).is_empty());
    }

    #[test]
    fn package_ids() {
        assert!(package_matches(
            "path+file:///x/crates/nie-abi#2.0.0",
            "nie-abi"
        ));
        assert!(package_matches(
            "path+file:///x/crates/nie-abi#nie-abi@2.0.0",
            "nie-abi"
        ));
        assert!(package_matches("nie-abi 2.0.0 (path+file:///x)", "nie-abi"));
        assert!(!package_matches(
            "path+file:///x/crates/nie-abi#2.0.0",
            "nie"
        ));
    }
}
