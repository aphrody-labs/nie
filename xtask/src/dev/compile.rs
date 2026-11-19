//! `cargo xtask compile`: release, static musl, Windows (msvc) and wasm32 builds, and the crlib libraries.

use std::path::Path;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use super::host::{BuildHost, Cmd, sh_quote};
use super::step::{Action, Exec, Runner, Step, run_with};
use crate::util::hex;

/// Arguments of `cargo xtask compile`.
#[derive(clap::Args, Debug, Clone, Copy, Default)]
pub struct Args {
    #[command(flatten)]
    pub exec: Exec,
    /// Release build of the host binaries (`nie`, `nie-mcp`, `nie-model-serve`); the default.
    #[arg(long)]
    pub release: bool,
    /// Static `x86_64-unknown-linux-musl` build of `nie` (cloud profile).
    #[arg(long)]
    pub musl: bool,
    /// `x86_64-pc-windows-msvc` cross build of `nie` (cloud profile), through `cargo xwin`.
    #[arg(long)]
    pub windows: bool,
    /// `wasm32-unknown-unknown` build of `nie-wasm`.
    #[arg(long)]
    pub wasm: bool,
    /// Static and shared libraries of every crlib crate (`crates/nie-abi/crlib.json`), with the
    /// generated headers, collected under `dist/crlib`.
    #[arg(long)]
    pub crlib: bool,
    /// Every target above.
    #[arg(long)]
    pub all: bool,
}

/// A build target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// Native release.
    Native,
    /// Static musl.
    Musl,
    /// Windows msvc.
    Windows,
    /// wasm32.
    Wasm,
    /// crlib libraries.
    Crlib,
}

impl Args {
    /// The targets these flags select, in build order.
    #[must_use]
    pub fn targets(&self) -> Vec<Target> {
        let any = self.release || self.musl || self.windows || self.wasm || self.crlib || self.all;
        let mut out = Vec::new();
        if !any || self.release || self.all {
            out.push(Target::Native);
        }
        for (flag, target) in [
            (self.musl, Target::Musl),
            (self.windows, Target::Windows),
            (self.wasm, Target::Wasm),
            (self.crlib, Target::Crlib),
        ] {
            if flag || self.all {
                out.push(target);
            }
        }
        out
    }
}

/// Artifact paths relative to the cargo target directory.
#[must_use]
pub fn artifacts(target: Target) -> Vec<&'static str> {
    match target {
        Target::Native => vec!["release/nie", "release/nie-mcp", "release/nie-model-serve"],
        Target::Musl => vec!["x86_64-unknown-linux-musl/release/nie"],
        Target::Windows => vec!["x86_64-pc-windows-msvc/release/nie.exe"],
        Target::Wasm => vec!["wasm32-unknown-unknown/release/nie_wasm.wasm"],
        Target::Crlib => Vec::new(),
    }
}

/// The cargo invocation that builds a target.
#[must_use]
pub fn build_cmd(target: Target) -> Option<Cmd> {
    let cloud = [
        "-p",
        "nie-cli",
        "--no-default-features",
        "--features",
        "cloud",
        "--bin",
        "nie",
    ];
    Some(match target {
        Target::Native => Cmd::cargo([
            "build",
            "--locked",
            "--release",
            "-p",
            "nie-cli",
            "-p",
            "nie-model-serve",
            "--bins",
        ]),
        Target::Musl => Cmd::cargo([
            "build",
            "--locked",
            "--release",
            "--target",
            "x86_64-unknown-linux-musl",
        ])
        .args(cloud),
        Target::Windows => Cmd::cargo([
            "xwin",
            "build",
            "--locked",
            "--release",
            "--target",
            "x86_64-pc-windows-msvc",
        ])
        .args(cloud),
        Target::Wasm => Cmd::cargo([
            "build",
            "--locked",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "-p",
            "nie-wasm",
        ]),
        Target::Crlib => return None,
    })
}

/// One line of the artifact listing: digest, size, path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    /// SHA-256, lowercase hex.
    pub sha256: String,
    /// Size in bytes.
    pub bytes: u64,
    /// Path relative to the target directory.
    pub path: String,
}

/// Parses the `sha256 size path` lines printed by the remote listing.
#[must_use]
pub fn parse_listing(text: &str) -> Vec<Artifact> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, ' ');
            let sha256 = parts.next()?.to_owned();
            let bytes = parts.next()?.parse().ok()?;
            let path = parts.next()?.to_owned();
            (sha256.len() == 64).then_some(Artifact {
                sha256,
                bytes,
                path,
            })
        })
        .collect()
}

/// The artifact table.
#[must_use]
pub fn render_artifacts(list: &[Artifact]) -> String {
    let mut out = String::new();
    for a in list {
        out.push_str(&format!("{}  {:>12}  {}\n", a.sha256, a.bytes, a.path));
    }
    out
}

fn local_listing(target_dir: &Path, paths: &[&str]) -> Vec<Artifact> {
    paths
        .iter()
        .filter_map(|rel| {
            let bytes = std::fs::read(target_dir.join(rel)).ok()?;
            Some(Artifact {
                sha256: hex(&Sha256::digest(&bytes)),
                bytes: bytes.len() as u64,
                path: (*rel).to_owned(),
            })
        })
        .collect()
}

fn listing_step(name: &str, host: &BuildHost, target_dir: &Path, paths: Vec<&'static str>) -> Step {
    let script = format!(
        "cd {} && for f in {}; do if [ -f \"$f\" ]; then printf '%s %s %s\\n' \"$(sha256sum \"$f\" | cut -d' ' -f1)\" \"$(stat -c %s \"$f\")\" \"$f\"; fi; done",
        sh_quote(&host.path("target")),
        paths
            .iter()
            .map(|p| sh_quote(p))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let dir = target_dir.to_path_buf();
    let local_paths = paths.clone();
    let show = |text: &str| -> Result<()> {
        let list = parse_listing(text);
        print!("{}", render_artifacts(&list));
        Ok(())
    };
    let mut step = Step::build(name, Cmd::new("sh").args(["-c", &script])).captured(show);
    step.local = Some(Action::Call(Box::new(move || {
        let list = local_listing(&dir, &local_paths);
        print!("{}", render_artifacts(&list));
        Ok(())
    })));
    step
}

/// A crate of `crates/nie-abi/crlib.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrlibCrate {
    /// Cargo package.
    pub package: String,
    /// Features that switch the C surface on (`crlib` unless the manifest says otherwise).
    pub features: Vec<String>,
}

/// Reads the crlib manifest and returns the crates whose status is `crlib`.
///
/// # Errors
/// When the manifest is not valid JSON of the expected shape.
pub fn parse_crlib_manifest(text: &str) -> Result<Vec<CrlibCrate>> {
    let value: serde_json::Value = serde_json::from_str(text).context("crlib.json")?;
    let crates = value["crates"]
        .as_array()
        .context("crlib.json has no `crates` array")?;
    Ok(crates
        .iter()
        .filter(|c| c["status"].as_str() == Some("crlib"))
        .filter_map(|c| {
            let package = c["package"].as_str()?.to_owned();
            let features = c["features"]
                .as_array()
                .map(|f| {
                    f.iter()
                        .filter_map(|s| s.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_else(|| vec!["crlib".to_owned()]);
            Some(CrlibCrate { package, features })
        })
        .collect())
}

/// `cargo build --release` of every crlib crate with its C surface enabled.
#[must_use]
pub fn crlib_cmd(crates: &[CrlibCrate]) -> Cmd {
    let mut cmd = Cmd::cargo(["build", "--locked", "--release"]);
    for c in crates {
        cmd = cmd.args(["-p", &c.package]);
        for feature in &c.features {
            cmd = cmd.args(["--features", &format!("{}/{feature}", c.package)]);
        }
    }
    cmd
}

fn crlib_steps(runner: &Runner) -> Vec<Step> {
    let manifest = runner.root().join("crates/nie-abi/crlib.json");
    let crates = match std::fs::read_to_string(&manifest) {
        Ok(text) => parse_crlib_manifest(&text),
        Err(e) => Err(anyhow::anyhow!(
            "{}: {e} (the crlib manifest is not there yet)",
            manifest.display()
        )),
    };
    let crates = match crates {
        Ok(c) if !c.is_empty() => c,
        Ok(_) => {
            return vec![Step::call("crlib", || {
                anyhow::bail!("crlib.json lists no crate with status `crlib`")
            })];
        }
        Err(e) => return vec![Step::call("crlib", move || Err(e))],
    };
    let host = runner.host().clone();
    let root = runner.root().to_path_buf();
    let names: Vec<String> = crates.iter().map(|c| c.package.replace('-', "_")).collect();
    let remote_copy = format!(
        "dist={dist} && mkdir -p \"$dist\" && rm -f \"$dist\"/* && for n in {names}; do for e in a so; do cp -f {target}/release/lib$n.$e \"$dist\"/ 2>/dev/null || true; done; done && cp -f include/*.h \"$dist\"/ 2>/dev/null; ls -l \"$dist\"",
        dist = sh_quote(&host.path("dist/crlib")),
        names = names.join(" "),
        target = sh_quote(&host.path("target")),
    );
    let local_names = names.clone();
    vec![
        Step::build("crlib", crlib_cmd(&crates)),
        Step::build("crlib-collect", Cmd::new("sh").args(["-c", &remote_copy])).or_locally(
            Action::Call(Box::new(move || {
                let dist = root.join("dist/crlib");
                std::fs::create_dir_all(&dist)?;
                let mut copied = 0;
                for name in &local_names {
                    for file in [
                        format!("lib{name}.a"),
                        format!("lib{name}.so"),
                        format!("{name}.dll"),
                        format!("{name}.lib"),
                    ] {
                        let from = root.join("target/release").join(&file);
                        if from.is_file() {
                            std::fs::copy(&from, dist.join(&file))?;
                            copied += 1;
                        }
                    }
                }
                if let Ok(headers) = std::fs::read_dir(root.join("include")) {
                    for h in headers
                        .flatten()
                        .filter(|h| h.path().extension().is_some_and(|e| e == "h"))
                    {
                        std::fs::copy(h.path(), dist.join(h.file_name()))?;
                        copied += 1;
                    }
                }
                println!("collected {copied} files in {}", dist.display());
                Ok(())
            })),
        ),
    ]
}

/// The steps for the selected targets.
#[must_use]
pub fn steps(args: Args, runner: &Runner) -> Vec<Step> {
    let mut steps = Vec::new();
    for target in args.targets() {
        if target == Target::Crlib {
            steps.extend(crlib_steps(runner));
            continue;
        }
        let name = format!("{target:?}").to_lowercase();
        if let Some(cmd) = build_cmd(target) {
            steps.push(Step::build(&name, cmd));
        }
        steps.push(listing_step(
            &format!("{name}-artifacts"),
            runner.host(),
            &runner.root().join("target"),
            artifacts(target),
        ));
    }
    steps
}

/// Runs `cargo xtask compile`.
///
/// # Errors
/// When a step fails.
pub fn run(args: Args) -> Result<()> {
    let mut runner = Runner::new(args.exec.mode());
    let steps = steps(args, &runner);
    run_with(&mut runner, steps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_flag_means_the_native_release() {
        assert_eq!(Args::default().targets(), [Target::Native]);
        let args = Args {
            musl: true,
            ..Args::default()
        };
        assert_eq!(args.targets(), [Target::Musl]);
        let all = Args {
            all: true,
            ..Args::default()
        };
        assert_eq!(
            all.targets(),
            [
                Target::Native,
                Target::Musl,
                Target::Windows,
                Target::Wasm,
                Target::Crlib
            ]
        );
    }

    #[test]
    fn musl_uses_the_cloud_profile() {
        let cmd = build_cmd(Target::Musl).unwrap();
        assert_eq!(
            cmd.display(),
            "cargo build --locked --release --target x86_64-unknown-linux-musl -p nie-cli --no-default-features --features cloud --bin nie"
        );
        assert!(
            build_cmd(Target::Windows)
                .unwrap()
                .display()
                .starts_with("cargo xwin build")
        );
        assert!(
            build_cmd(Target::Wasm)
                .unwrap()
                .display()
                .contains("-p nie-wasm")
        );
        assert!(build_cmd(Target::Crlib).is_none());
    }

    #[test]
    fn listings_parse_and_render() {
        let sha = "a".repeat(64);
        let list = parse_listing(&format!("{sha} 12 release/nie\nnoise\n"));
        assert_eq!(
            list,
            [Artifact {
                sha256: sha.clone(),
                bytes: 12,
                path: "release/nie".into()
            }]
        );
        assert!(render_artifacts(&list).contains("release/nie"));
    }

    #[test]
    fn local_listing_hashes_existing_files_only() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("release")).unwrap();
        std::fs::write(dir.path().join("release/nie"), b"abc").unwrap();
        let list = local_listing(dir.path(), &["release/nie", "release/missing"]);
        assert_eq!(list.len(), 1);
        assert_eq!(
            list[0].sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(list[0].bytes, 3);
    }

    #[test]
    fn the_crlib_manifest_selects_crlib_crates() {
        let json = r#"{"crates":[
          {"package":"nie-formats","path":"crates/nie-formats","id":"formats","status":"crlib"},
          {"package":"nie-cli","path":"crates/nie-cli","id":"cli","status":"excluded","reason":"binary"},
          {"package":"nie-x","path":"p","id":"x","status":"crlib","features":["crlib","std"]}
        ]}"#;
        let crates = parse_crlib_manifest(json).unwrap();
        assert_eq!(crates.len(), 2);
        assert_eq!(crates[0].features, ["crlib"]);
        let cmd = crlib_cmd(&crates);
        assert!(
            cmd.display()
                .contains("-p nie-formats --features nie-formats/crlib")
        );
        assert!(cmd.display().contains("--features nie-x/std"));
        assert!(parse_crlib_manifest("{}").is_err());
    }
}
