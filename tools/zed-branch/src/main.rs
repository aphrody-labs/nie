//! Regenerates the `nie` branch of the aphrody-labs/zed fork.
//!
//! The branch holds only the GPUI closure of upstream zed main: the workspace crates reachable
//! from `gpui_platform` and `gpui_tokio` on linux, windows and wasm32, all Apache-2.0, with every
//! dependency taken from a crates.io release (wgpu at 30.0.2, the Zed forks at their published
//! `-zed` releases). Nothing of the aphrody commits on the fork is used: each run starts from the
//! upstream ref, writes the tree to a scratch directory and records it as one new commit on top of
//! the previous `nie` tip, so submodule pins stay fetchable and no history is rewritten.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use toml_edit::{DocumentMut, Item, Table, value};

/// Crates that start the closure.
const ROOTS: &[&str] = &["gpui_platform", "gpui_tokio"];
/// Platforms whose dependency graphs are united.
const TARGETS: &[&str] = &[
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
    "wasm32-unknown-unknown",
];
/// Non-crate trees the closure includes next to its crates (fonts embedded by `include_bytes!`).
const EXTRA_TREES: &[&str] = &["assets/fonts", "LICENSE-APACHE"];
/// The wgpu release that replaces the git revision zed pins.
const WGPU_VERSION: &str = "=30.0.2";
/// Registry version for git dependencies that name no version of their own.
const REGISTRY_VERSIONS: &[(&str, &str)] = &[("naga", "=30.0.2"), ("wgpu", WGPU_VERSION)];

/// Source edits that adapt the closure to the crates.io wgpu: file, text to find (exactly once),
/// replacement, reason. A miss fails the run, so upstream drift is noticed instead of ignored.
const SOURCE_EDITS: &[(&str, &str, &str, &str)] = &[
    (
        "crates/gpui_wgpu/src/wgpu_context.rs",
        "                default_queue: wgpu::QueueDescriptor::default(),\n",
        "",
        "DeviceDescriptor::default_queue exists only on wgpu trunk (gfx-rs/wgpu#10109); 30.0.2 has a single default queue",
    ),
    (
        "crates/gpui_wgpu/src/wgpu_atlas.rs",
        "                    default_queue: wgpu::QueueDescriptor::default(),\n",
        "",
        "DeviceDescriptor::default_queue exists only on wgpu trunk (gfx-rs/wgpu#10109); 30.0.2 has a single default queue",
    ),
    (
        "crates/gpui_wgpu/src/wgpu_renderer.rs",
        "!format.has_srgb_suffix()",
        "!format.is_srgb()",
        "TextureFormat::has_srgb_suffix is trunk-only; is_srgb is the same test on 30.0.2",
    ),
];

struct Args {
    repo: PathBuf,
    upstream: String,
    branch: String,
    toolchain: String,
    work: PathBuf,
    report: Option<PathBuf>,
}

impl Args {
    fn parse() -> Result<Self> {
        let mut map: BTreeMap<String, String> = BTreeMap::new();
        let mut it = std::env::args().skip(1);
        while let Some(flag) = it.next() {
            let key = flag.strip_prefix("--").context("flags look like --name value")?;
            map.insert(key.to_owned(), it.next().context("a flag needs a value")?);
        }
        let repo = PathBuf::from(map.remove("repo").context("--repo <zed clone> is required")?);
        let work = map.remove("work").map_or_else(
            || repo.parent().unwrap_or(Path::new(".")).join("work"),
            PathBuf::from,
        );
        Ok(Self {
            repo,
            upstream: map.remove("upstream").unwrap_or_else(|| "upstream/main".into()),
            branch: map.remove("branch").unwrap_or_else(|| "nie".into()),
            toolchain: map.remove("toolchain").context("--toolchain <channel> is required")?,
            work,
            report: map.remove("report").map(PathBuf::from),
        })
    }
}

fn run(cmd: &mut Command) -> Result<String> {
    let out = cmd.output().with_context(|| format!("spawn {cmd:?}"))?;
    if !out.status.success() {
        bail!("{cmd:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn git(repo: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo);
    cmd
}

#[derive(Debug)]
struct Pkg {
    dir: PathBuf,
    license: String,
}

struct Closure {
    ws: BTreeMap<String, Pkg>,
    non_registry: BTreeSet<String>,
    /// Name of every package of the closure, registry ones included.
    all: BTreeSet<String>,
}

/// Workspace crates reachable from the roots on every target, plus any resolved package that does
/// not come from the crates.io registry.
fn closure(root: &Path) -> Result<Closure> {
    let mut ws: BTreeMap<String, Pkg> = BTreeMap::new();
    let mut non_registry = BTreeSet::new();
    let mut all = BTreeSet::new();
    for target in TARGETS {
        let raw = run(Command::new("cargo")
            .args(["metadata", "--format-version", "1", "--filter-platform", target])
            .arg("--manifest-path")
            .arg(root.join("Cargo.toml")))?;
        let meta: Value = serde_json::from_str(&raw)?;
        let packages: BTreeMap<&str, &Value> = meta["packages"]
            .as_array()
            .context("packages")?
            .iter()
            .map(|p| (p["id"].as_str().unwrap_or_default(), p))
            .collect();
        let nodes: BTreeMap<&str, &Value> = meta["resolve"]["nodes"]
            .as_array()
            .context("nodes")?
            .iter()
            .map(|n| (n["id"].as_str().unwrap_or_default(), n))
            .collect();
        let mut stack: Vec<&str> = packages
            .iter()
            .filter(|(_, p)| ROOTS.contains(&p["name"].as_str().unwrap_or_default()))
            .map(|(id, _)| *id)
            .collect();
        let mut seen = BTreeSet::new();
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            for dep in nodes[id]["deps"].as_array().context("deps")? {
                stack.push(dep["pkg"].as_str().context("pkg")?);
            }
        }
        for id in seen {
            let p = packages[id];
            let name = p["name"].as_str().unwrap_or_default().to_owned();
            all.insert(name.clone());
            match p["source"].as_str() {
                None => {
                    let manifest = Path::new(p["manifest_path"].as_str().unwrap_or_default());
                    let dir = manifest.parent().context("manifest dir")?;
                    let rel = dir.strip_prefix(root).context("crate outside the workspace")?;
                    ws.insert(
                        name,
                        Pkg {
                            dir: rel.to_path_buf(),
                            license: p["license"].as_str().unwrap_or_default().to_owned(),
                        },
                    );
                }
                Some(src) if src.starts_with("registry+https://github.com/rust-lang/crates.io-index")
                    || src.starts_with("sparse+https://index.crates.io") => {}
                Some(src) => {
                    non_registry.insert(format!("{name} {src}"));
                }
            }
        }
    }
    Ok(Closure { ws, non_registry, all })
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if entry.file_name() == "target" {
            continue;
        }
        if entry.file_type()?.is_dir() {
            copy_dir(&src, &dst)?;
        } else {
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

/// A git dependency or patch that the branch replaces with a registry release.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Fork {
    name: String,
    git: String,
    rev: String,
    registry: String,
}

fn text(item: &Item, key: &str) -> Option<String> {
    item.get(key).and_then(|v| v.as_str()).map(str::to_owned)
}

/// Turns one dependency specification from git into a crates.io specification.
fn to_registry(key: &str, item: &mut Item, forks: &mut BTreeSet<Fork>) -> Result<()> {
    let Some(git) = text(item, "git") else {
        return Ok(());
    };
    let name = text(item, "package").unwrap_or_else(|| key.to_owned());
    let rev = text(item, "rev").unwrap_or_default();
    let pinned = REGISTRY_VERSIONS.iter().find(|(n, _)| *n == name).map(|(_, v)| *v);
    let version = match (pinned, text(item, "version")) {
        (Some(v), _) => v.to_owned(),
        (None, Some(v)) => v,
        (None, None) => bail!("git dependency `{key}` ({git}) has no registry version mapping"),
    };
    let table = item.as_table_like_mut().context("dependency is a table")?;
    for k in ["git", "rev", "branch", "tag"] {
        table.remove(k);
    }
    table.insert("version", value(version.as_str()));
    forks.insert(Fork { name, git, rev, registry: version });
    Ok(())
}

const PATCHED: &str = "no [patch]: the crates.io release";

const DEP_TABLES: &[&str] = &["dependencies", "dev-dependencies", "build-dependencies"];

/// Applies `f` to every dependency table of a crate manifest, target-specific ones included.
fn for_each_dep_table(doc: &mut DocumentMut, f: &mut dyn FnMut(&mut Table) -> Result<()>) -> Result<()> {
    for name in DEP_TABLES {
        if let Some(t) = doc.get_mut(name).and_then(Item::as_table_mut) {
            f(t)?;
        }
    }
    if let Some(targets) = doc.get_mut("target").and_then(Item::as_table_mut) {
        for (_, cfg) in targets.iter_mut() {
            let Some(cfg) = cfg.as_table_mut() else { continue };
            for name in DEP_TABLES {
                if let Some(t) = cfg.get_mut(name).and_then(Item::as_table_mut) {
                    f(t)?;
                }
            }
        }
    }
    Ok(())
}

/// Drops dependencies on workspace crates that are outside the closure (the macOS and iOS
/// backends) together with the features that name them.
fn drop_outside(doc: &mut DocumentMut, dropped: &BTreeSet<String>) -> Result<()> {
    for_each_dep_table(doc, &mut |t| {
        for key in dropped {
            t.remove(key);
        }
        Ok(())
    })?;
    if let Some(features) = doc.get_mut("features").and_then(Item::as_table_mut) {
        for (_, list) in features.iter_mut() {
            let Some(list) = list.as_array_mut() else { continue };
            list.retain(|v| {
                let name = v.as_str().unwrap_or_default();
                let name = name.strip_prefix("dep:").unwrap_or(name);
                let name = name.split('/').next().unwrap_or(name).trim_end_matches('?');
                !dropped.contains(name)
            });
        }
    }
    Ok(())
}

fn write_tree(base: &Path, out: &Path, args: &Args, c: &Closure) -> Result<BTreeSet<Fork>> {
    if out.exists() {
        fs::remove_dir_all(out)?;
    }
    fs::create_dir_all(out)?;
    let mut forks = BTreeSet::new();
    let mut referenced: BTreeSet<String> = BTreeSet::new();
    let base_root: DocumentMut = fs::read_to_string(base.join("Cargo.toml"))?.parse()?;
    let dropped: BTreeSet<String> = base_root["workspace"]["dependencies"]
        .as_table()
        .context("[workspace.dependencies]")?
        .iter()
        .filter(|(k, v)| text(v, "path").is_some() && !c.ws.contains_key(*k))
        .map(|(k, _)| k.to_owned())
        .collect();
    for pkg in c.ws.values() {
        copy_dir(&base.join(&pkg.dir), &out.join(&pkg.dir))?;
        let manifest = out.join(&pkg.dir).join("Cargo.toml");
        let mut doc: DocumentMut = fs::read_to_string(&manifest)?.parse()?;
        drop_outside(&mut doc, &dropped)?;
        for_each_dep_table(&mut doc, &mut |t| {
            for (key, item) in t.iter_mut() {
                if item.get("workspace").and_then(Item::as_bool) == Some(true) {
                    referenced.insert(key.get().to_owned());
                } else {
                    to_registry(key.get(), item, &mut forks)?;
                }
            }
            Ok(())
        })?;
        fs::write(&manifest, doc.to_string())?;
    }
    for (file, from, to, _) in SOURCE_EDITS {
        let path = out.join(file);
        let source = fs::read_to_string(&path).with_context(|| format!("read {file}"))?;
        if source.matches(from).count() != 1 {
            bail!("source edit for {file}: `{}` is not found exactly once", from.trim());
        }
        fs::write(&path, source.replacen(from, to, 1))?;
    }
    for tree in EXTRA_TREES {
        let from = base.join(tree);
        if from.is_dir() {
            copy_dir(&from, &out.join(tree))?;
        } else if from.is_file() {
            if let Some(parent) = out.join(tree).parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&from, out.join(tree))?;
        }
    }

    let mut root: DocumentMut = fs::read_to_string(base.join("Cargo.toml"))?.parse()?;
    let ws = root["workspace"].as_table_mut().context("[workspace]")?;
    let mut members = toml_edit::Array::new();
    for pkg in c.ws.values() {
        members.push(pkg.dir.to_string_lossy().replace('\\', "/"));
    }
    ws.insert("members", value(members));
    ws.remove("exclude");
    ws.remove("default-members");
    ws.remove("metadata");
    let deps = ws["dependencies"].as_table_mut().context("[workspace.dependencies]")?;
    let keys: Vec<String> = deps.iter().map(|(k, _)| k.to_owned()).collect();
    for key in keys {
        let item = deps.get_mut(&key).context("dependency vanished")?;
        let keep = referenced.contains(&key);
        if !keep {
            deps.remove(&key);
            continue;
        }
        to_registry(&key, item, &mut forks)?;
    }
    if let Some(patch) = root.get("patch").and_then(Item::as_table) {
        for (_, table) in patch.iter() {
            let Some(table) = table.as_table_like() else { continue };
            for (name, spec) in table.iter() {
                if let (Some(git), rev) = (text(spec, "git"), text(spec, "rev")) {
                    forks.insert(Fork {
                        name: name.to_owned(),
                        git,
                        rev: rev.unwrap_or_default(),
                        registry: PATCHED.into(),
                    });
                }
            }
        }
    }
    root.remove("patch");
    forks.retain(|f| f.registry != PATCHED || c.all.contains(&f.name));
    root.remove("profile");
    fs::write(out.join("Cargo.toml"), root.to_string())?;
    fs::write(
        out.join("rust-toolchain.toml"),
        format!(
            "[toolchain]\nchannel = \"{}\"\nprofile = \"minimal\"\ncomponents = [\"rustfmt\", \"clippy\"]\n\
             targets = [\"wasm32-unknown-unknown\"]\n",
            args.toolchain
        ),
    )?;
    Ok(forks)
}

fn main() -> Result<()> {
    let args = Args::parse()?;
    let sha = run(git(&args.repo).args(["rev-parse", &args.upstream]))?;
    let base = args.work.join("base");
    let out = args.work.join("out");
    if base.exists() {
        fs::remove_dir_all(&base)?;
    }
    fs::create_dir_all(&base)?;
    let mut archive = git(&args.repo)
        .args(["archive", &sha, "--", "Cargo.toml", "crates", "tooling", "extensions"])
        .args(EXTRA_TREES)
        .stdout(Stdio::piped())
        .spawn()?;
    let tar = Command::new("tar")
        .arg("-x")
        .arg("-C")
        .arg(&base)
        .stdin(archive.stdout.take().context("archive stdout")?)
        .status()?;
    if !tar.success() || !archive.wait()?.success() {
        bail!("git archive of {sha} failed");
    }

    let mut current = closure(&base)?;
    let mut forks = BTreeSet::new();
    for round in 1..=8 {
        forks = write_tree(&base, &out, &args, &current)?;
        let next = closure(&out)?;
        eprintln!("round {round}: {} -> {} workspace crates", current.ws.len(), next.ws.len());
        let settled = next.ws.keys().eq(current.ws.keys());
        if !next.ws.keys().all(|k| current.ws.contains_key(k)) {
            bail!("the pruned workspace grew: {:?}", next.ws.keys().collect::<Vec<_>>());
        }
        current = next;
        if settled {
            break;
        }
    }
    let bad: Vec<_> = current
        .ws
        .iter()
        .filter(|(_, p)| p.license != "Apache-2.0")
        .map(|(n, p)| format!("{n}: {}", p.license))
        .collect();
    if !bad.is_empty() {
        bail!("crates that are not Apache-2.0: {bad:?}");
    }
    if !current.non_registry.is_empty() {
        bail!("dependencies outside crates.io remain: {:?}", current.non_registry);
    }

    let report = json!({
        "upstream": args.upstream,
        "upstreamSha": sha,
        "toolchain": args.toolchain,
        "wgpu": WGPU_VERSION,
        "targets": TARGETS,
        "roots": ROOTS,
        "crates": current.ws.iter().map(|(n, p)| json!({
            "name": n, "dir": p.dir, "license": p.license,
        })).collect::<Vec<_>>(),
        "sourceEdits": SOURCE_EDITS.iter().map(|(file, _, _, reason)| json!({
            "file": file, "reason": reason,
        })).collect::<Vec<_>>(),
        "wgpuFixesOnlyAtPinnedRev": [
            {"pr": "gfx-rs/wgpu#10230", "title": "Fix issue with surface drop panic (Vulkan)", "mergedAt": "2026-09-12"},
            {"pr": "gfx-rs/wgpu#10348", "title": "Ignore timeouts on wasm (WebGL resize)", "mergedAt": "2026-09-15"},
        ],
        "replacedGit": forks.iter().map(|f| json!({
            "name": f.name, "git": f.git, "rev": f.rev, "registry": f.registry,
        })).collect::<Vec<_>>(),
    });
    let pretty = serde_json::to_string_pretty(&report)? + "\n";
    fs::write(out.join("nie-closure.json"), &pretty)?;
    fs::write(
        out.join("NIE.md"),
        format!(
            "# nie branch\n\nGenerated, never edited by hand, from `{}` at `{sha}` by `tools/zed-branch` of \
             aphrody-labs/nie. It holds the Apache-2.0 GPUI closure listed in `nie-closure.json`; \
             every dependency comes from a crates.io release and there is no `[patch]`.\n",
            args.upstream
        ),
    )?;
    if let Some(path) = &args.report {
        fs::write(path, &pretty)?;
    }
    record(&args, &sha, &out, current.ws.len())
}

/// Commits `out` as the next `nie` commit, on top of the previous tip when there is one.
fn record(args: &Args, sha: &str, out: &Path, crates: usize) -> Result<()> {
    let index = args.work.join("index");
    let _ = fs::remove_file(&index);
    let tree = {
        let mut add = git(&args.repo);
        add.env("GIT_INDEX_FILE", &index)
            .arg("--work-tree")
            .arg(out)
            .args(["add", "-A", "-f", "."]);
        run(&mut add)?;
        let mut wt = git(&args.repo);
        wt.env("GIT_INDEX_FILE", &index).arg("write-tree");
        run(&mut wt)?
    };
    let parent = run(git(&args.repo).args(["rev-parse", "--verify", "-q", &format!("refs/remotes/origin/{}", args.branch)]))
        .or_else(|_| run(git(&args.repo).args(["rev-parse", "--verify", "-q", &format!("refs/heads/{}", args.branch)])))
        .ok();
    if let Some(p) = &parent {
        if run(git(&args.repo).args(["rev-parse", &format!("{p}^{{tree}}")]))? == tree {
            println!("unchanged {p}");
            return Ok(());
        }
    }
    let message = format!(
        "nie: GPUI closure of zed main {sha}\n\n{crates} Apache-2.0 workspace crates, wgpu {WGPU_VERSION} from crates.io, no [patch].\n\
         Generated by tools/zed-branch of aphrody-labs/nie.\n"
    );
    let mut commit = git(&args.repo);
    commit.args(["commit-tree", &tree, "-m", &message]);
    if let Some(p) = &parent {
        commit.args(["-p", p]);
    }
    let id = run(&mut commit)?;
    run(git(&args.repo).args(["update-ref", &format!("refs/heads/{}", args.branch), &id]))?;
    println!("{id}");
    Ok(())
}
