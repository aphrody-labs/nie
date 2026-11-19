//! `nie new`: scaffolds of nie applications.
//!
//! A scaffold is a complete cargo package whose only dependency is `nie-plugin`, so it builds and
//! passes its tests with no edit, inside the nie workspace (`apps/<name>`, inheriting the
//! workspace package fields and lints) or anywhere else (a standalone workspace pinned to the
//! framework checkout by path, with the toolchain file of the framework).

mod manifest;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail};
pub use manifest::{Abi, App, AppManifest, Plugins, Targets, valid_name};

/// A template.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Template {
    /// Command line program.
    Cli,
    /// HTTP service.
    Serve,
    /// C ABI library (cdylib, staticlib, rlib) with a contract for `nie bindgen`.
    Lib,
    /// UI app: typed view tree rendered to HTML.
    Ui,
    /// WebAssembly module for `wasm32-unknown-unknown`.
    Wasm,
}

const NATIVE_TARGETS: [&str; 5] = [
    "x86_64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
    "aarch64-apple-darwin",
];

macro_rules! tpl {
    ($dir:literal, $file:literal) => {
        include_str!(concat!("../templates/", $dir, "/", $file))
    };
}

impl Template {
    /// Every template.
    pub const ALL: [Self; 5] = [Self::Cli, Self::Serve, Self::Lib, Self::Ui, Self::Wasm];

    /// The template named `name`.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.id() == name)
    }

    /// Name as written in `nie.toml` and on the command line.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::Serve => "serve",
            Self::Lib => "lib",
            Self::Ui => "ui",
            Self::Wasm => "wasm",
        }
    }

    fn summary(self) -> &'static str {
        match self {
            Self::Cli => "Command line app",
            Self::Serve => "HTTP service",
            Self::Lib => "C ABI library",
            Self::Ui => "UI app",
            Self::Wasm => "WebAssembly module",
        }
    }

    fn targets(self) -> Vec<String> {
        match self {
            Self::Wasm => vec!["wasm32-unknown-unknown".to_owned()],
            _ => NATIVE_TARGETS.map(str::to_owned).to_vec(),
        }
    }

    fn crate_types(self) -> Option<&'static str> {
        match self {
            Self::Lib => Some("[\"cdylib\", \"staticlib\", \"rlib\"]"),
            Self::Wasm => Some("[\"cdylib\", \"rlib\"]"),
            _ => None,
        }
    }

    /// Files of the template relative to the app directory, `@key@` placeholders unexpanded.
    fn files(self) -> Vec<(&'static str, &'static str)> {
        let mut files = vec![
            ("README.md", tpl!("common", "README.md")),
            (".gitignore", tpl!("common", "gitignore")),
        ];
        match self {
            Self::Cli => files.extend([
                ("src/lib.rs", tpl!("cli", "src/lib.rs")),
                ("src/main.rs", tpl!("cli", "src/main.rs")),
                ("tests/cli.rs", tpl!("cli", "tests/cli.rs")),
            ]),
            Self::Serve => files.extend([
                ("src/lib.rs", tpl!("serve", "src/lib.rs")),
                ("src/main.rs", tpl!("serve", "src/main.rs")),
                ("tests/serve.rs", tpl!("serve", "tests/serve.rs")),
            ]),
            Self::Lib => files.extend([
                ("src/lib.rs", tpl!("lib", "src/lib.rs")),
                ("abi.json", tpl!("lib", "abi.json")),
                ("tests/abi.rs", tpl!("lib", "tests/abi.rs")),
            ]),
            Self::Ui => files.extend([
                ("src/lib.rs", tpl!("ui", "src/lib.rs")),
                ("src/main.rs", tpl!("ui", "src/main.rs")),
                ("tests/ui.rs", tpl!("ui", "tests/ui.rs")),
            ]),
            Self::Wasm => files.extend([
                ("src/lib.rs", tpl!("wasm", "src/lib.rs")),
                ("tests/wasm.rs", tpl!("wasm", "tests/wasm.rs")),
            ]),
        }
        files
    }
}

/// Where the app lives relative to the framework.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    /// Inside the nie workspace: inherits the workspace package fields, lints and profiles.
    Workspace,
    /// Anywhere else: a standalone workspace that depends on the framework checkout at this path.
    External {
        /// Root of the nie checkout.
        framework: PathBuf,
    },
}

/// A request to scaffold an app.
#[derive(Debug, Clone)]
pub struct Request {
    /// App name.
    pub name: String,
    /// Template.
    pub template: Template,
    /// App directory (created; must be empty or absent).
    pub dir: PathBuf,
    /// Placement.
    pub placement: Placement,
}

/// Root of a nie checkout: a directory whose `Cargo.toml` declares a workspace and that holds
/// `crates/nie-plugin`. Searches `start` and its ancestors.
#[must_use]
pub fn find_framework(start: &Path) -> Option<PathBuf> {
    start.ancestors().find(|d| is_framework(d)).map(Path::to_path_buf)
}

fn is_framework(dir: &Path) -> bool {
    dir.join("crates/nie-plugin/Cargo.toml").is_file()
        && std::fs::read_to_string(dir.join("Cargo.toml")).is_ok_and(|t| t.contains("[workspace]"))
}

/// The framework root for `nie new`: `explicit`, else `NIE_FRAMEWORK`, else the checkout above
/// `cwd`, else the checkout this binary sits in.
///
/// # Errors
/// When no checkout is found.
pub fn resolve_framework(explicit: Option<&Path>, cwd: &Path) -> anyhow::Result<PathBuf> {
    // The checkout the binary sits in (`target/<profile>/nie-new`); never a path baked in at
    // compile time, so the binary is the same whichever directory built it.
    let built_from = std::env::current_exe().ok().and_then(|exe| find_framework(&exe));
    let candidates = [
        explicit.map(Path::to_path_buf),
        std::env::var_os("NIE_FRAMEWORK").map(PathBuf::from),
        find_framework(cwd),
        built_from,
    ];
    for c in candidates.into_iter().flatten() {
        if is_framework(&c) {
            return Ok(c.canonicalize().unwrap_or(c));
        }
    }
    bail!("no nie checkout found: pass --framework <dir> or set NIE_FRAMEWORK")
}

fn pascal(ident: &str) -> String {
    ident
        .split('_')
        .map(|p| {
            let mut c = p.chars();
            c.next().map_or_else(String::new, |f| f.to_uppercase().collect::<String>() + c.as_str())
        })
        .collect()
}

fn expand(text: &str, vars: &[(&str, String)]) -> String {
    let mut out = text.to_owned();
    for (k, v) in vars {
        out = out.replace(&format!("@{k}@"), v);
    }
    out
}

fn toml_path(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn cargo_toml(req: &Request, ident: &str, framework_rust: Option<&FrameworkFacts>) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "[package]\nname = \"{}\"", req.name);
    match (&req.placement, framework_rust) {
        (Placement::External { .. }, Some(f)) => {
            let _ = writeln!(
                s,
                "version = \"0.1.0\"\nedition = \"2024\"\nrust-version = \"{}\"\npublish = false",
                f.rust_version
            );
        }
        _ => {
            let _ = writeln!(
                s,
                "version.workspace = true\nedition.workspace = true\nrust-version.workspace = true\nlicense.workspace = true\nrepository.workspace = true\nauthors.workspace = true\npublish = false"
            );
        }
    }
    let _ = writeln!(s, "description = \"{} ({})\"", req.template.summary(), req.template.id());
    if let Some(types) = req.template.crate_types() {
        let _ = writeln!(s, "\n[lib]\nname = \"{ident}\"\ncrate-type = {types}");
    }
    let _ = writeln!(s, "\n[dependencies]");
    match &req.placement {
        Placement::Workspace => {
            let _ = writeln!(s, "nie-plugin = {{ workspace = true }}\n\n[lints]\nworkspace = true");
        }
        Placement::External { framework } => {
            let _ = writeln!(
                s,
                "nie-plugin = {{ path = \"{}/crates/nie-plugin\" }}\n\n[lints.rust]\nunsafe_op_in_unsafe_fn = \"deny\"\nunused_must_use = \"deny\"\n\n[lints.clippy]\ntodo = \"deny\"\nunimplemented = \"deny\"\ndbg_macro = \"deny\"\n\n[profile.release]\nlto = \"fat\"\ncodegen-units = 1\npanic = \"abort\"\nstrip = true\n\n[workspace]",
                toml_path(framework)
            );
        }
    }
    s
}

struct FrameworkFacts {
    rust_version: String,
    version: String,
    toolchain: Option<String>,
}

fn framework_facts(root: &Path) -> anyhow::Result<FrameworkFacts> {
    let manifest: toml::Table = std::fs::read_to_string(root.join("Cargo.toml"))?.parse()?;
    let rust_version = manifest
        .get("workspace")
        .and_then(|w| w.get("package"))
        .and_then(|p| p.get("rust-version"))
        .and_then(toml::Value::as_str)
        .context("workspace.package.rust-version of the framework")?
        .to_owned();
    let version = manifest
        .get("workspace")
        .and_then(|w| w.get("package"))
        .and_then(|p| p.get("version"))
        .and_then(toml::Value::as_str)
        .context("workspace.package.version of the framework")?
        .to_owned();
    let toolchain = std::fs::read_to_string(root.join("rust-toolchain.toml")).ok();
    Ok(FrameworkFacts { rust_version, version, toolchain })
}

/// Writes the scaffold and returns the files written, relative to the app directory.
///
/// # Errors
/// On an invalid name, a non-empty destination or an I/O error.
pub fn generate(req: &Request) -> anyhow::Result<Vec<String>> {
    valid_name(&req.name)?;
    if req.dir.exists() && std::fs::read_dir(&req.dir)?.next().is_some() {
        bail!("{} exists and is not empty", req.dir.display());
    }
    let ident = req.name.replace('-', "_");
    let facts = match &req.placement {
        Placement::External { framework } => Some(framework_facts(framework)?),
        Placement::Workspace => None,
    };
    let plugin = expand(
        include_str!("../templates/common/lib_plugin.rs"),
        &[
            ("Pascal", pascal(&ident)),
            ("id", req.name.clone()),
            ("name", req.name.clone()),
            ("summary", req.template.summary().to_owned()),
        ],
    );
    let vars = [
        ("name", req.name.clone()),
        ("ident", ident.clone()),
        ("Pascal", pascal(&ident)),
        ("ENV", ident.to_uppercase()),
        ("id", req.name.clone()),
        ("template", req.template.id().to_owned()),
        ("plugin", plugin),
    ];
    let mut written = Vec::new();
    let mut put = |rel: &str, text: String| -> anyhow::Result<()> {
        let path = req.dir.join(rel);
        std::fs::create_dir_all(path.parent().context("parent")?)?;
        std::fs::write(&path, text).with_context(|| path.display().to_string())?;
        written.push(rel.to_owned());
        Ok(())
    };
    put("Cargo.toml", cargo_toml(req, &ident, facts.as_ref()))?;
    for (rel, text) in req.template.files() {
        put(rel, expand(text, &vars))?;
    }
    let manifest = AppManifest {
        app: App { name: req.name.clone(), template: req.template.id().to_owned() },
        targets: Targets { list: req.template.targets() },
        plugins: Plugins::default(),
        abi: if req.template == Template::Lib {
            Abi {
                contract: Some("abi.json".to_owned()),
                exports: vec![
                    "nie_ffi_abi_version".to_owned(),
                    "nie_string_free".to_owned(),
                    format!("nie_{ident}_version"),
                    format!("nie_{ident}_add"),
                ],
            }
        } else {
            Abi::default()
        },
    };
    manifest.validate()?;
    put("nie.toml", manifest.render()?)?;
    if let Some(f) = facts {
        // Every dependency is a path dependency, so the lock is fully determined and `--locked`
        // works on the first build.
        let mut blocks = [
            ("nie-plugin".to_owned(), format!("name = \"nie-plugin\"\nversion = \"{}\"\n", f.version)),
            (
                req.name.clone(),
                format!("name = \"{}\"\nversion = \"0.1.0\"\ndependencies = [\n \"nie-plugin\",\n]\n", req.name),
            ),
        ];
        blocks.sort();
        let body: Vec<String> = blocks.into_iter().map(|(_, b)| format!("[[package]]\n{b}")).collect();
        put(
            "Cargo.lock",
            format!(
                "# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n\n{}",
                body.join("\n")
            ),
        )?;
        if let Some(t) = f.toolchain {
            put("rust-toolchain.toml", t)?;
        }
    }
    written.sort();
    Ok(written)
}

/// Arguments of `nie new`.
#[derive(Debug, clap::Parser)]
#[command(name = "new", about = "Scaffold a nie application: apps/<name> in the workspace, or --path anywhere")]
pub struct Args {
    /// App name (lowercase letters, digits and `-`).
    pub name: String,
    /// Template.
    #[arg(long, value_enum, default_value = "cli")]
    pub template: Template,
    /// Destination directory of the app; default `apps/<name>` of the framework checkout.
    #[arg(long)]
    pub path: Option<PathBuf>,
    /// Root of the nie checkout to depend on (default: `NIE_FRAMEWORK`, the checkout above the
    /// current directory, or the one this binary sits in).
    #[arg(long)]
    pub framework: Option<PathBuf>,
}

/// Runs `nie new`.
///
/// # Errors
/// When no framework checkout is found or the scaffold cannot be written.
pub fn run(args: &Args) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let framework = resolve_framework(args.framework.as_deref(), &cwd)?;
    let (dir, placement) = match &args.path {
        None => (framework.join("apps").join(&args.name), Placement::Workspace),
        Some(p) => {
            let dir = if p.is_absolute() { p.clone() } else { cwd.join(p) };
            let inside = dir.canonicalize().unwrap_or_else(|_| dir.clone()).starts_with(&framework)
                && dir.starts_with(framework.join("apps"));
            (dir, if inside { Placement::Workspace } else { Placement::External { framework } })
        }
    };
    let req = Request { name: args.name.clone(), template: args.template, dir, placement };
    let files = generate(&req)?;
    println!("{} {} ({})", req.template.id(), req.dir.display(), files.join(" "));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_framework(root: &Path) {
        std::fs::create_dir_all(root.join("crates/nie-plugin")).unwrap();
        std::fs::write(root.join("crates/nie-plugin/Cargo.toml"), "").unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\n[workspace.package]\nversion = \"2.0.0\"\nrust-version = \"1.100\"\n",
        )
        .unwrap();
        std::fs::write(root.join("rust-toolchain.toml"), "[toolchain]\nchannel = \"1.100.0\"\n").unwrap();
    }

    #[test]
    fn every_template_expands_without_placeholders() {
        let tmp = tempfile::tempdir().unwrap();
        fake_framework(&tmp.path().join("fw"));
        for t in Template::ALL {
            for placement in [
                Placement::Workspace,
                Placement::External { framework: tmp.path().join("fw") },
            ] {
                let dir = tmp.path().join(format!("{}-{}", t.id(), matches!(placement, Placement::Workspace)));
                let req = Request { name: "demo-app".into(), template: t, dir: dir.clone(), placement };
                let files = generate(&req).unwrap();
                assert!(files.contains(&"nie.toml".to_owned()) && files.contains(&"Cargo.toml".to_owned()));
                for f in files {
                    let text = std::fs::read_to_string(dir.join(&f)).unwrap();
                    assert!(f == "Cargo.lock" || !text.contains('@'), "{f} keeps a placeholder: {text}");
                }
                let m = AppManifest::parse(&std::fs::read_to_string(dir.join("nie.toml")).unwrap()).unwrap();
                assert_eq!(m.app.template, t.id());
            }
        }
    }

    #[test]
    fn refuses_a_non_empty_destination_and_a_bad_name() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("x"), "").unwrap();
        let req = Request { name: "ok".into(), template: Template::Cli, dir: tmp.path().into(), placement: Placement::Workspace };
        assert!(generate(&req).is_err());
        let req = Request { name: "Bad".into(), template: Template::Cli, dir: tmp.path().join("n"), placement: Placement::Workspace };
        assert!(generate(&req).is_err());
    }

    #[test]
    fn framework_is_found_from_a_subdirectory() {
        let tmp = tempfile::tempdir().unwrap();
        fake_framework(tmp.path());
        let sub = tmp.path().join("apps/x");
        std::fs::create_dir_all(&sub).unwrap();
        assert_eq!(find_framework(&sub).unwrap(), tmp.path());
    }
}
