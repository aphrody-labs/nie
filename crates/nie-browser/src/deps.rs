//! `nie-browser deps`: each vendored engine crate against its newest crates.io release.
//!
//! The vendored tree (`vendor/kitesurf/crates/*`) carries patches as `browser-*` path crates. For
//! every one the report reads the newest non-yanked release from crates.io, then asks of each patch
//! of its series (`scripts/patches/<crate>-<version>/*.patch`) whether the release already contains
//! it: a patch counts as contained when at least [`CONTAINED`] of the lines it adds, outside
//! `Cargo.toml`, appear in the same file of the release. A crate whose patches are all contained
//! goes back to the crates.io version; the others stay path crates with the remaining patch count.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::{DepsArgs, tar};

/// Share of a patch's added lines the release must hold for the patch to count as contained.
pub const CONTAINED: f64 = 0.9;

const PREFIX: &str = "browser-";

/// One crate under `crates/` of the checkout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vendored {
    /// The crates.io name the library is published under.
    pub name: String,
    pub version: Version,
}

/// A published release: its version and the files of its `.crate` archive.
pub struct Release {
    pub version: Version,
    pub files: BTreeMap<String, Vec<u8>>,
}

/// Where releases come from; crates.io in the binary, a fixture in tests.
pub trait Registry {
    /// The newest non-yanked release, `None` when the crate is not on the registry.
    ///
    /// # Errors
    /// Network or archive failure.
    fn latest(&self, name: &str) -> Result<Option<Release>>;
}

/// crates.io over HTTPS.
pub struct CratesIo {
    agent: ureq::Agent,
}

impl CratesIo {
    #[must_use]
    pub fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .user_agent("nie")
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(120)))
            .build()
            .new_agent();
        Self { agent }
    }

    fn get(&self, url: &str) -> Result<Option<Vec<u8>>> {
        let mut res = self.agent.get(url).call().with_context(|| format!("GET {url}"))?;
        if res.status().as_u16() == 404 {
            return Ok(None);
        }
        if !res.status().is_success() {
            bail!("GET {url}: {}", res.status());
        }
        let body = res.body_mut().with_config().limit(256 * 1024 * 1024).read_to_vec()?;
        Ok(Some(body))
    }
}

impl Default for CratesIo {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry for CratesIo {
    fn latest(&self, name: &str) -> Result<Option<Release>> {
        let Some(meta) = self.get(&format!("https://crates.io/api/v1/crates/{name}"))? else {
            return Ok(None);
        };
        let json: serde_json::Value = serde_json::from_slice(&meta)?;
        let newest = json["versions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|v| v["yanked"].as_bool() == Some(false))
            .filter_map(|v| Version::parse(v["num"].as_str()?).ok())
            .max();
        let Some(version) = newest else {
            return Ok(None);
        };
        let url = format!("https://static.crates.io/crates/{name}/{name}-{version}.crate");
        let gz = self.get(&url)?.with_context(|| format!("{url} is missing"))?;
        Ok(Some(Release { version, files: tar::read_crate(&gz)? }))
    }
}

/// The crates of `<root>/crates`, by their published names.
///
/// # Errors
/// An unreadable manifest.
pub fn vendored(root: &Path) -> Result<Vec<Vendored>> {
    let mut out = Vec::new();
    let dir = root.join("crates");
    for entry in fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))? {
        let manifest = entry?.path().join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        let doc: toml::Table = fs::read_to_string(&manifest)
            .with_context(|| format!("read {}", manifest.display()))?
            .parse()?;
        let package = doc.get("package").and_then(toml::Value::as_table).context("no [package]")?;
        let name = package.get("name").and_then(toml::Value::as_str).context("no name")?;
        let version = package.get("version").and_then(toml::Value::as_str).context("no version")?;
        out.push(Vendored {
            name: name.strip_prefix(PREFIX).unwrap_or(name).to_owned(),
            version: Version::parse(version)?,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// The lines each file of a git-format patch adds, trimmed, blank lines and `Cargo.toml` left out.
#[must_use]
pub fn added_lines(patch: &str) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut file: Option<String> = None;
    for line in patch.lines() {
        if let Some(rest) = line.strip_prefix("diff --git a/") {
            file = rest.split(" b/").next().map(str::to_owned);
        } else if line.starts_with("+++") || line.starts_with("---") {
        } else if let (Some(added), Some(f)) = (line.strip_prefix('+'), &file) {
            let t = added.trim();
            if t.len() > 2 && f != "Cargo.toml" {
                out.entry(f.clone()).or_default().push(t.to_owned());
            }
        }
    }
    out
}

/// Whether `files` already hold at least [`CONTAINED`] of what the patch adds.
#[must_use]
pub fn is_contained(patch: &str, files: &BTreeMap<String, Vec<u8>>) -> bool {
    let added = added_lines(patch);
    let total: usize = added.values().map(Vec::len).sum();
    if total == 0 {
        return false;
    }
    let mut found = 0usize;
    for (path, lines) in &added {
        let Some(body) = files.get(path) else { continue };
        let have: BTreeSet<&str> = std::str::from_utf8(body).unwrap_or("").lines().map(str::trim).collect();
        found += lines.iter().filter(|l| have.contains(l.as_str())).count();
    }
    #[allow(clippy::cast_precision_loss)]
    let ratio = found as f64 / total as f64;
    ratio >= CONTAINED
}

/// Patch texts of a crate's series, in filename order.
///
/// # Errors
/// An unreadable patch.
pub fn series(root: &Path, krate: &Vendored) -> Result<Vec<(String, String)>> {
    let base = root.join("scripts/patches");
    let Ok(entries) = fs::read_dir(&base) else { return Ok(Vec::new()) };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_prefix(&format!("{}-", krate.name)))
                .is_some_and(|v| Version::parse(v).is_ok_and(|v| v == krate.version))
        })
        .collect();
    dirs.sort();
    let mut out = Vec::new();
    for dir in dirs {
        let mut files: Vec<PathBuf> = fs::read_dir(&dir)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "patch"))
            .collect();
        files.sort();
        for f in files {
            let name = f.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            out.push((name, fs::read_to_string(&f)?));
        }
    }
    Ok(out)
}

/// One row of the report.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub vendored: String,
    pub latest: Option<String>,
    pub patches: usize,
    pub contained: usize,
    pub remaining: usize,
    pub verdict: Verdict,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// Back to the crates.io release: no patch of the series is missing from it.
    CratesIo,
    /// Stays a `browser-*` path crate: patches remain.
    Keep,
    /// Not published on crates.io.
    Unpublished,
}

/// Build the report of every vendored crate.
///
/// # Errors
/// A failure reading the checkout or the registry.
pub fn report(root: &Path, registry: &dyn Registry) -> Result<Vec<Row>> {
    let mut rows = Vec::new();
    for krate in vendored(root)? {
        let patches = series(root, &krate)?;
        let release = registry.latest(&krate.name)?;
        let (latest, contained) = match &release {
            Some(r) => (
                Some(r.version.to_string()),
                patches.iter().filter(|(_, text)| is_contained(text, &r.files)).count(),
            ),
            None => (None, 0),
        };
        let remaining = patches.len() - contained;
        let newest_ok = release.as_ref().is_some_and(|r| r.version >= krate.version);
        let verdict = match (&release, remaining, newest_ok) {
            (None, ..) => Verdict::Unpublished,
            (Some(_), 0, true) => Verdict::CratesIo,
            _ => Verdict::Keep,
        };
        rows.push(Row {
            name: krate.name,
            vendored: krate.version.to_string(),
            latest,
            patches: patches.len(),
            contained,
            remaining,
            verdict,
        });
    }
    Ok(rows)
}

/// Markdown form of the report.
///
/// # Errors
/// Formatting cannot fail in practice.
pub fn markdown(rows: &[Row], commit: &str) -> Result<String> {
    let mut out = String::new();
    writeln!(out, "# kitesurf vendored crates against crates.io\n")?;
    writeln!(out, "Generated by `nie-browser deps`; upstream commit `{commit}` of vendor/kitesurf.\n")?;
    let remaining: usize = rows.iter().map(|r| r.remaining).sum();
    let back = rows.iter().filter(|r| r.verdict == Verdict::CratesIo).count();
    writeln!(
        out,
        "{} crates, {back} can return to crates.io, {remaining} patches remain across the rest.\n",
        rows.len()
    )?;
    writeln!(out, "| crate | vendored | crates.io | patches | contained | remaining | verdict |")?;
    writeln!(out, "| --- | --- | --- | ---: | ---: | ---: | --- |")?;
    for r in rows {
        let verdict = match r.verdict {
            Verdict::CratesIo => "crates.io",
            Verdict::Keep => "keep",
            Verdict::Unpublished => "not on crates.io",
        };
        writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {verdict} |",
            r.name,
            r.vendored,
            r.latest.as_deref().unwrap_or("-"),
            r.patches,
            r.contained,
            r.remaining
        )?;
    }
    Ok(out)
}

/// The commit a checkout (directory `.git` or a submodule's `.git` file) points at.
#[must_use]
pub fn head_commit(root: &Path) -> Option<String> {
    let dot_git = root.join(".git");
    let git_dir = if dot_git.is_file() {
        let text = fs::read_to_string(&dot_git).ok()?;
        let rel = text.trim().strip_prefix("gitdir:")?.trim();
        root.join(rel)
    } else {
        dot_git
    };
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    let Some(reference) = head.strip_prefix("ref: ") else {
        return Some(head.to_owned());
    };
    if let Ok(sha) = fs::read_to_string(git_dir.join(reference)) {
        return Some(sha.trim().to_owned());
    }
    let packed = fs::read_to_string(git_dir.join("packed-refs")).ok()?;
    packed.lines().find_map(|l| {
        let (sha, name) = l.split_once(' ')?;
        (name == reference).then(|| sha.to_owned())
    })
}

/// Entry point of `nie-browser deps`.
///
/// # Errors
/// See [`report`].
pub fn run(args: &DepsArgs) -> Result<()> {
    let rows = report(&args.root, &CratesIo::new())?;
    let commit = args.commit.clone().or_else(|| head_commit(&args.root)).unwrap_or_else(|| "unknown".to_owned());
    let text = if args.json {
        serde_json::to_string_pretty(&serde_json::json!({ "commit": commit, "crates": rows }))?
    } else {
        markdown(&rows, &commit)?
    };
    match &args.out {
        Some(path) => fs::write(path, text + "\n").with_context(|| format!("write {}", path.display()))?,
        None => println!("{text}"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(BTreeMap<String, Release>);

    impl Registry for Fixture {
        fn latest(&self, name: &str) -> Result<Option<Release>> {
            Ok(self.0.get(name).map(|r| Release { version: r.version.clone(), files: r.files.clone() }))
        }
    }

    const PATCH: &str = "From 0\nSubject: [PATCH] add\n---\ndiff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,1 +1,3 @@\n fn old() {}\n+fn added_one() {}\n+fn added_two() {}\ndiff --git a/Cargo.toml b/Cargo.toml\n--- a/Cargo.toml\n+++ b/Cargo.toml\n@@ -1 +1,2 @@\n+thin-vec = \"0.2\"\n";

    fn checkout(dir: &Path) {
        let c = dir.join("crates/alpha");
        fs::create_dir_all(&c).unwrap();
        fs::write(c.join("Cargo.toml"), "[package]\nname = \"browser-alpha\"\nversion = \"1.0.0\"\n").unwrap();
        let p = dir.join("scripts/patches/alpha-1.0.0");
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join("0001-add.patch"), PATCH).unwrap();
        fs::write(p.join("README.txt"), "not a patch").unwrap();
    }

    fn release(version: &str, lib: &str) -> Release {
        let mut files = BTreeMap::new();
        files.insert("src/lib.rs".to_owned(), lib.as_bytes().to_vec());
        Release { version: Version::parse(version).unwrap(), files }
    }

    #[test]
    fn added_lines_skip_manifests_and_headers() {
        let added = added_lines(PATCH);
        assert_eq!(added.len(), 1);
        assert_eq!(added["src/lib.rs"], ["fn added_one() {}", "fn added_two() {}"]);
    }

    #[test]
    fn vendored_strips_the_browser_prefix() {
        let dir = tempfile::tempdir().unwrap();
        checkout(dir.path());
        let v = vendored(dir.path()).unwrap();
        assert_eq!(v, [Vendored { name: "alpha".into(), version: Version::new(1, 0, 0) }]);
    }

    #[test]
    fn a_release_holding_every_patch_goes_back_to_crates_io() {
        let dir = tempfile::tempdir().unwrap();
        checkout(dir.path());
        let reg = Fixture(BTreeMap::from([(
            "alpha".to_owned(),
            release("1.1.0", "fn old() {}\n    fn added_one() {}\nfn added_two() {}\n"),
        )]));
        let rows = report(dir.path(), &reg).unwrap();
        assert_eq!((rows[0].patches, rows[0].contained, rows[0].remaining), (1, 1, 0));
        assert_eq!(rows[0].verdict, Verdict::CratesIo);
    }

    #[test]
    fn a_release_without_the_patch_keeps_the_path_crate() {
        let dir = tempfile::tempdir().unwrap();
        checkout(dir.path());
        let reg = Fixture(BTreeMap::from([("alpha".to_owned(), release("1.1.0", "fn old() {}\n"))]));
        let rows = report(dir.path(), &reg).unwrap();
        assert_eq!((rows[0].contained, rows[0].remaining), (0, 1));
        assert_eq!(rows[0].verdict, Verdict::Keep);
        assert!(markdown(&rows, "abc").unwrap().contains("| alpha | 1.0.0 | 1.1.0 | 1 | 0 | 1 | keep |"));
    }

    #[test]
    fn an_unpublished_crate_is_reported_as_such() {
        let dir = tempfile::tempdir().unwrap();
        checkout(dir.path());
        let rows = report(dir.path(), &Fixture(BTreeMap::new())).unwrap();
        assert_eq!(rows[0].verdict, Verdict::Unpublished);
        assert_eq!(rows[0].latest, None);
    }

    #[test]
    fn head_commit_follows_a_gitdir_file_and_a_ref() {
        let dir = tempfile::tempdir().unwrap();
        let git = dir.path().join("real");
        fs::create_dir_all(git.join("refs/heads")).unwrap();
        fs::write(git.join("HEAD"), "ref: refs/heads/nie\n").unwrap();
        fs::write(git.join("refs/heads/nie"), "0123456789abcdef\n").unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join(".git"), "gitdir: ../real\n").unwrap();
        assert_eq!(head_commit(&sub).as_deref(), Some("0123456789abcdef"));
    }
}
