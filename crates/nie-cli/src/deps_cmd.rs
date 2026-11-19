//! `nie deps upstream`: reads an upstream GitHub repository live and decides, for each crate it
//! publishes or depends on, whether nie takes it from crates.io, already has it, only cites its
//! design, or rejects it.
//!
//! Everything runs in process: GitHub (REST API and raw files) and crates.io (sparse index for the
//! versions, JSON API for licence and repository, at most one API request per second) are read over
//! HTTP, answers are cached under the user cache directory, and the result is written to
//! `docs/specs/terminal-sources.json` and `docs/specs/terminal-sources.md`. The name lists and the
//! licence lists come from `docs/specs/terminal-sources.policy.toml`.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Subcommand};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const USER_AGENT: &str = "nie";
/// Minimum gap between two crates.io API requests (policy: at most one per second).
const API_GAP: Duration = Duration::from_millis(1100);
const POLICY_FILE: &str = "terminal-sources.policy.toml";
const JSON_FILE: &str = "terminal-sources.json";
const MD_FILE: &str = "terminal-sources.md";
const DESIGNS_HEADING: &str = "## Designs cited";
const PROBE_ID: &str = "crates_io_probe";

#[derive(Args)]
pub struct DepsArgs {
    #[command(subcommand)]
    cmd: DepsCmd,
}

#[derive(Subcommand)]
enum DepsCmd {
    /// Read upstream GitHub repositories live and decide, per crate, crates-io, already-in-nie,
    /// design-only or rejected.
    Upstream(UpstreamArgs),
    /// Measure nie.exe imports, the kernel, external process calls and Cargo.lock; write the
    /// research log entry.
    Audit(crate::deps_audit::AuditArgs),
}

#[derive(Args)]
struct UpstreamArgs {
    /// GitHub repository `owner/name` (repeatable). Without any `--repo` and `--probe`, every
    /// repository of the committed JSON is read again.
    #[arg(long = "repo", value_name = "OWNER/NAME")]
    repos: Vec<String>,
    /// Also record the crates.io probe (latest versions, `bevy_ratatui` requirement).
    #[arg(long)]
    probe: bool,
    /// Print the computed JSON on stdout instead of the text summary.
    #[arg(long)]
    json: bool,
    /// Output directory (default: `docs/specs` of the nie checkout).
    #[arg(long, value_name = "DIR")]
    out: Option<PathBuf>,
    /// Read again and print the drift against the committed JSON; exit 1 on drift, write nothing.
    #[arg(long)]
    check: bool,
    /// nie checkout holding `Cargo.lock` and `docs/specs` (default: found from the current directory).
    #[arg(long, value_name = "DIR")]
    root: Option<PathBuf>,
    /// Policy file (default: `terminal-sources.policy.toml` next to the output).
    #[arg(long, value_name = "FILE")]
    policy: Option<PathBuf>,
    /// Longest age, in seconds, of a cached HTTP answer.
    #[arg(long, default_value_t = 21_600)]
    max_age_secs: u64,
}

pub fn run(args: DepsArgs) -> Result<()> {
    match args.cmd {
        DepsCmd::Upstream(args) => {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(upstream(args))
        }
        DepsCmd::Audit(args) => crate::deps_audit::run(args),
    }
}

// ---------------------------------------------------------------------------------------------
// Policy
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
struct Policy {
    #[serde(default)]
    licenses: Licenses,
    #[serde(default)]
    lua: NameList,
    #[serde(default)]
    design_cited: NameList,
    #[serde(default)]
    render: Render,
    #[serde(default)]
    repo: BTreeMap<String, RepoPolicy>,
    #[serde(default)]
    probe: ProbePolicy,
}

#[derive(Debug, Default, Deserialize)]
struct Licenses {
    #[serde(default)]
    allow: Vec<String>,
    #[serde(default)]
    deny: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct NameList {
    #[serde(default)]
    reason: String,
    #[serde(default)]
    crates: Vec<String>,
}

impl NameList {
    fn contains(&self, name: &str) -> bool {
        let name = canon(name);
        self.crates.iter().any(|c| canon(c) == name)
    }
}

#[derive(Debug, Deserialize)]
struct Render {
    #[serde(default = "default_max_rows")]
    max_rows: usize,
}

impl Default for Render {
    fn default() -> Self {
        Self {
            max_rows: default_max_rows(),
        }
    }
}

fn default_max_rows() -> usize {
    80
}

#[derive(Debug, Default, Deserialize)]
struct RepoPolicy {
    scope: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ProbePolicy {
    #[serde(default)]
    crates: Vec<String>,
    requirement: Option<ProbeRequirementPolicy>,
}

#[derive(Debug, Deserialize)]
struct ProbeRequirementPolicy {
    #[serde(rename = "crate")]
    krate: String,
    dependency: String,
    lock_package: Option<String>,
    #[serde(default)]
    targets: Vec<String>,
}

// ---------------------------------------------------------------------------------------------
// Output documents
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
struct Doc {
    schema: u32,
    #[serde(default)]
    sources: BTreeMap<String, Source>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    crates_io_probe: Option<Probe>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Release {
    tag: String,
    published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Source {
    repo: String,
    url: String,
    license: Option<String>,
    default_branch: String,
    latest_release: Option<Release>,
    #[serde(rename = "ref")]
    git_ref: String,
    ref_kind: String,
    scope: String,
    note: Option<String>,
    members_published: usize,
    members_unpublished: usize,
    counts: BTreeMap<String, usize>,
    members: Vec<Member>,
    decisions: Vec<Row>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Member {
    name: String,
    path: String,
    version: Option<String>,
    publish: bool,
    features: Vec<String>,
    dependencies: usize,
    published: bool,
    crates_io_latest: Option<String>,
    lag: Option<String>,
    license: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Row {
    #[serde(rename = "crate")]
    krate: String,
    kind: String,
    requirement: String,
    decision: String,
    reason: String,
    license: Option<String>,
    latest: Option<String>,
    used_by: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    nie_locked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    compatible: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Probe {
    crates: BTreeMap<String, ProbeCrate>,
    requirement: Option<ProbeRequirement>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ProbeCrate {
    exists: bool,
    latest: Option<String>,
    latest_prerelease: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ProbeRequirement {
    #[serde(rename = "crate")]
    krate: String,
    version: Option<String>,
    dependency: String,
    targets: Vec<ProbeTarget>,
    requirements: Vec<ProbeDep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ProbeTarget {
    source: String,
    version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ProbeDep {
    kind: String,
    requirement: String,
    optional: bool,
    admits: BTreeMap<String, bool>,
}

// ---------------------------------------------------------------------------------------------
// Names, versions, licences
// ---------------------------------------------------------------------------------------------

/// Canonical crate name: crates.io treats `-` and `_` as the same character.
fn canon(name: &str) -> String {
    name.to_ascii_lowercase().replace('-', "_")
}

/// Highest non-yanked stable version, else the highest non-yanked one.
fn latest_of(versions: &[(Version, bool)]) -> Option<Version> {
    let live = || versions.iter().filter(|(_, y)| !y).map(|(v, _)| v);
    live()
        .filter(|v| v.pre.is_empty())
        .max()
        .or_else(|| live().max())
        .cloned()
}

fn req_admits(req: &str, version: &Version) -> Option<bool> {
    VersionReq::parse(req).ok().map(|r| r.matches(version))
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Lic {
    Allowed,
    Unknown,
    Denied,
}

/// Evaluates an SPDX expression against the policy: `OR` takes the best branch, `AND` the worst.
/// Returns the verdict and the first identifier that is not allowed.
fn license_verdict(expr: &str, policy: &Licenses) -> (Lic, Option<String>) {
    let spaced = expr.replace('/', " OR ").replace('(', " ( ").replace(')', " ) ");
    let tokens: Vec<&str> = spaced.split_whitespace().collect();
    let mut parser = SpdxParser {
        tokens: &tokens,
        at: 0,
        policy,
        offender: None,
    };
    let verdict = parser.or();
    (verdict, parser.offender)
}

struct SpdxParser<'a> {
    tokens: &'a [&'a str],
    at: usize,
    policy: &'a Licenses,
    offender: Option<String>,
}

impl<'a> SpdxParser<'a> {
    fn peek(&self) -> Option<&'a str> {
        self.tokens.get(self.at).copied()
    }

    fn or(&mut self) -> Lic {
        let mut best = self.and();
        while self.peek().is_some_and(|t| t.eq_ignore_ascii_case("OR")) {
            self.at += 1;
            let next = self.and();
            best = best.min(next);
        }
        best
    }

    fn and(&mut self) -> Lic {
        let mut worst = self.atom();
        while self.peek().is_some_and(|t| t.eq_ignore_ascii_case("AND")) {
            self.at += 1;
            let next = self.atom();
            worst = worst.max(next);
        }
        worst
    }

    fn atom(&mut self) -> Lic {
        match self.peek() {
            Some("(") => {
                self.at += 1;
                let inner = self.or();
                if self.peek() == Some(")") {
                    self.at += 1;
                }
                inner
            }
            Some(id) => {
                self.at += 1;
                if self.peek().is_some_and(|t| t.eq_ignore_ascii_case("WITH")) {
                    self.at += 2;
                }
                let id = id.trim_end_matches('+');
                let upper = id.to_ascii_uppercase();
                let verdict = if self
                    .policy
                    .deny
                    .iter()
                    .any(|p| upper.starts_with(&p.to_ascii_uppercase()))
                {
                    Lic::Denied
                } else if self
                    .policy
                    .allow
                    .iter()
                    .any(|a| a.eq_ignore_ascii_case(id))
                {
                    Lic::Allowed
                } else {
                    Lic::Unknown
                };
                if verdict != Lic::Allowed && self.offender.is_none() {
                    self.offender = Some(id.to_owned());
                }
                verdict
            }
            None => Lic::Unknown,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Cargo manifests
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Src {
    Registry,
    AltRegistry(String),
    Git(String),
    Path,
}

#[derive(Debug, Clone)]
struct Dep {
    name: String,
    req: Option<String>,
    src: Src,
    /// Repository-relative directory of a path dependency.
    path: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct Ws {
    members: Vec<String>,
    exclude: Vec<String>,
    deps: BTreeMap<String, Dep>,
    version: Option<String>,
    publish: Option<bool>,
    license: Option<String>,
}

#[derive(Debug, Clone)]
struct Manifest {
    dir: String,
    name: Option<String>,
    version: Option<String>,
    publish: bool,
    license: Option<String>,
    features: Vec<String>,
    deps: Vec<Dep>,
    ws: Option<Ws>,
}

/// Joins a repository-relative directory and a relative path, resolving `.` and `..`.
fn join_path(base: &str, rel: &str) -> String {
    let mut parts: Vec<&str> = base.split('/').filter(|s| !s.is_empty() && *s != ".").collect();
    for seg in rel.split(['/', '\\']) {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

fn str_list(v: Option<&toml::Value>) -> Vec<String> {
    v.and_then(toml::Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn publish_value(v: &toml::Value) -> Option<bool> {
    match v {
        toml::Value::Boolean(b) => Some(*b),
        toml::Value::Array(a) => Some(a.iter().any(|x| x.as_str() == Some("crates-io"))),
        _ => None,
    }
}

fn is_workspace_ref(v: &toml::Value) -> bool {
    v.get("workspace").and_then(toml::Value::as_bool) == Some(true)
}

fn parse_dep(key: &str, value: &toml::Value, dir: &str, ws: Option<&Ws>) -> Option<Dep> {
    match value {
        toml::Value::String(req) => Some(Dep {
            name: key.to_owned(),
            req: Some(req.clone()),
            src: Src::Registry,
            path: None,
        }),
        toml::Value::Table(t) => {
            if is_workspace_ref(value) {
                return ws.and_then(|w| w.deps.get(key)).cloned();
            }
            let name = t
                .get("package")
                .and_then(toml::Value::as_str)
                .unwrap_or(key)
                .to_owned();
            let req = t.get("version").and_then(toml::Value::as_str).map(str::to_owned);
            let (src, path) = if let Some(git) = t.get("git").and_then(toml::Value::as_str) {
                (Src::Git(git.to_owned()), None)
            } else if let Some(path) = t.get("path").and_then(toml::Value::as_str) {
                (Src::Path, Some(join_path(dir, path)))
            } else if let Some(reg) = t.get("registry").and_then(toml::Value::as_str) {
                (Src::AltRegistry(reg.to_owned()), None)
            } else {
                (Src::Registry, None)
            };
            Some(Dep { name, req, src, path })
        }
        _ => None,
    }
}

fn dep_tables(doc: &toml::Table) -> Vec<&toml::Table> {
    const KEYS: [&str; 2] = ["dependencies", "build-dependencies"];
    let mut tables = Vec::new();
    for key in KEYS {
        if let Some(t) = doc.get(key).and_then(toml::Value::as_table) {
            tables.push(t);
        }
    }
    if let Some(targets) = doc.get("target").and_then(toml::Value::as_table) {
        for target in targets.values().filter_map(toml::Value::as_table) {
            for key in KEYS {
                if let Some(t) = target.get(key).and_then(toml::Value::as_table) {
                    tables.push(t);
                }
            }
        }
    }
    tables
}

fn parse_ws(table: &toml::Table, dir: &str) -> Ws {
    let mut ws = Ws {
        members: str_list(table.get("members")),
        exclude: str_list(table.get("exclude")),
        ..Ws::default()
    };
    if let Some(pkg) = table.get("package").and_then(toml::Value::as_table) {
        ws.version = pkg.get("version").and_then(toml::Value::as_str).map(str::to_owned);
        ws.publish = pkg.get("publish").and_then(publish_value);
        ws.license = pkg.get("license").and_then(toml::Value::as_str).map(str::to_owned);
    }
    if let Some(deps) = table.get("dependencies").and_then(toml::Value::as_table) {
        for (key, value) in deps {
            if let Some(dep) = parse_dep(key, value, dir, None) {
                ws.deps.insert(key.clone(), dep);
            }
        }
    }
    ws
}

/// Parses one `Cargo.toml`. `inherited` is the workspace of a member; the manifest of a workspace
/// root inherits from its own `[workspace]`.
fn parse_manifest(text: &str, dir: &str, inherited: Option<&Ws>) -> Result<Manifest> {
    let doc: toml::Table = text.parse().context("invalid Cargo.toml")?;
    let own_ws = doc
        .get("workspace")
        .and_then(toml::Value::as_table)
        .map(|t| parse_ws(t, dir));
    let ws_ctx = inherited.or(own_ws.as_ref());
    let pkg = doc.get("package").and_then(toml::Value::as_table);
    let mut manifest = Manifest {
        dir: dir.to_owned(),
        name: None,
        version: None,
        publish: true,
        license: None,
        features: doc
            .get("features")
            .and_then(toml::Value::as_table)
            .map(|t| t.keys().cloned().collect())
            .unwrap_or_default(),
        deps: Vec::new(),
        ws: own_ws.clone(),
    };
    if let Some(pkg) = pkg {
        manifest.name = pkg.get("name").and_then(toml::Value::as_str).map(str::to_owned);
        manifest.version = match pkg.get("version") {
            Some(toml::Value::String(v)) => Some(v.clone()),
            Some(v) if is_workspace_ref(v) => ws_ctx.and_then(|w| w.version.clone()),
            _ => None,
        };
        manifest.publish = match pkg.get("publish") {
            None => true,
            Some(v) if is_workspace_ref(v) => ws_ctx.and_then(|w| w.publish).unwrap_or(true),
            Some(v) => publish_value(v).unwrap_or(true),
        };
        manifest.license = match pkg.get("license") {
            Some(toml::Value::String(v)) => Some(v.clone()),
            Some(v) if is_workspace_ref(v) => ws_ctx.and_then(|w| w.license.clone()),
            _ => None,
        };
    }
    for table in dep_tables(&doc) {
        for (key, value) in table {
            if let Some(dep) = parse_dep(key, value, dir, ws_ctx) {
                manifest.deps.push(dep);
            }
        }
    }
    Ok(manifest)
}

fn seg_match(pat: &[char], text: &[char]) -> bool {
    match (pat.first(), text.first()) {
        (None, None) => true,
        (Some('*'), _) => seg_match(&pat[1..], text) || (!text.is_empty() && seg_match(pat, &text[1..])),
        (Some('?'), Some(_)) => seg_match(&pat[1..], &text[1..]),
        (Some(p), Some(t)) if p == t => seg_match(&pat[1..], &text[1..]),
        _ => false,
    }
}

fn glob_segments(pat: &[&str], path: &[&str]) -> bool {
    match pat.first() {
        None => path.is_empty(),
        Some(&"**") => (0..=path.len()).any(|n| glob_segments(&pat[1..], &path[n..])),
        Some(p) => {
            !path.is_empty()
                && seg_match(&p.chars().collect::<Vec<_>>(), &path[0].chars().collect::<Vec<_>>())
                && glob_segments(&pat[1..], &path[1..])
        }
    }
}

fn glob_match(pattern: &str, path: &str) -> bool {
    let pattern = pattern.trim_start_matches("./").trim_end_matches('/');
    let pat: Vec<&str> = pattern.split('/').collect();
    let segs: Vec<&str> = path.split('/').collect();
    glob_segments(&pat, &segs)
}

/// Workspace member directories: `members` patterns (literal or glob) over the directories that
/// hold a `Cargo.toml`, minus `exclude`.
fn expand_members(patterns: &[String], exclude: &[String], manifest_dirs: &BTreeSet<String>) -> BTreeSet<String> {
    manifest_dirs
        .iter()
        .filter(|d| !d.is_empty())
        .filter(|d| patterns.iter().any(|p| glob_match(p, d)))
        .filter(|d| !exclude.iter().any(|p| glob_match(p, d)))
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------------------------
// nie's own Cargo.lock
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Default)]
struct NieLock {
    versions: BTreeMap<String, Vec<Version>>,
}

impl NieLock {
    fn parse(text: &str) -> Result<Self> {
        let doc: toml::Table = text.parse().context("invalid Cargo.lock")?;
        let mut lock = Self::default();
        let packages = doc
            .get("package")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten();
        for package in packages {
            let (Some(name), Some(version)) = (
                package.get("name").and_then(toml::Value::as_str),
                package.get("version").and_then(toml::Value::as_str),
            ) else {
                continue;
            };
            let registry = package
                .get("source")
                .and_then(toml::Value::as_str)
                .is_some_and(|s| s.starts_with("registry+") || s.starts_with("sparse+"));
            if let (true, Ok(version)) = (registry, Version::parse(version)) {
                lock.versions.entry(canon(name)).or_default().push(version);
            }
        }
        Ok(lock)
    }

    fn get(&self, name: &str) -> Option<&[Version]> {
        self.versions.get(&canon(name)).map(Vec::as_slice)
    }
}

// ---------------------------------------------------------------------------------------------
// crates.io facts
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct Facts {
    exists: bool,
    versions: Vec<(Version, bool)>,
    license: Option<String>,
    repository: Option<String>,
}

impl Facts {
    fn latest(&self) -> Option<Version> {
        latest_of(&self.versions)
    }
}

#[derive(Debug, Deserialize)]
struct IndexEntry {
    vers: String,
    #[serde(default)]
    yanked: bool,
    #[serde(default)]
    deps: Vec<IndexDep>,
}

#[derive(Debug, Deserialize)]
struct IndexDep {
    name: String,
    req: String,
    #[serde(default)]
    optional: bool,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    package: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiCrate {
    #[serde(rename = "crate")]
    krate: ApiCrateMeta,
    #[serde(default)]
    versions: Vec<ApiVersion>,
}

#[derive(Debug, Deserialize)]
struct ApiCrateMeta {
    repository: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiVersion {
    num: String,
    license: Option<String>,
}

fn index_url(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    let path = match n.len() {
        1 => format!("1/{n}"),
        2 => format!("2/{n}"),
        3 => format!("3/{}/{n}", &n[..1]),
        _ => format!("{}/{}/{n}", &n[..2], &n[2..4]),
    };
    format!("https://index.crates.io/{path}")
}

fn parse_index(body: &str) -> Vec<IndexEntry> {
    body.lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

fn percent(s: &str, keep_slash: bool) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) || (keep_slash && b == b'/') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// HTTP with cache
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Host {
    Github,
    Raw,
    Index,
    Crates,
}

struct Net {
    client: reqwest::Client,
    cache_dir: Option<PathBuf>,
    max_age: Duration,
    token: Option<String>,
    last_api: Option<Instant>,
}

impl Net {
    fn new(max_age: Duration) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(90))
            .build()?;
        let cache_dir = dirs::cache_dir().map(|d| d.join("nie").join("deps-upstream"));
        if let Some(dir) = &cache_dir {
            let _ = std::fs::create_dir_all(dir);
        }
        let token = ["GITHUB_TOKEN", "GH_TOKEN"]
            .iter()
            .find_map(|k| std::env::var(k).ok())
            .filter(|t| !t.is_empty());
        Ok(Self {
            client,
            cache_dir,
            max_age,
            token,
            last_api: None,
        })
    }

    fn cache_path(&self, url: &str) -> Option<PathBuf> {
        let dir = self.cache_dir.as_ref()?;
        Some(dir.join(hex::encode(Sha256::digest(url.as_bytes()))))
    }

    fn cache_read(&self, url: &str) -> Option<(u16, String)> {
        let path = self.cache_path(url)?;
        let age = SystemTime::now()
            .duration_since(std::fs::metadata(&path).ok()?.modified().ok()?)
            .unwrap_or(Duration::MAX);
        if age > self.max_age {
            return None;
        }
        let text = std::fs::read_to_string(path).ok()?;
        let (status, body) = text.split_once('\n')?;
        Some((status.parse().ok()?, body.to_owned()))
    }

    fn cache_write(&self, url: &str, status: u16, body: &str) {
        if let Some(path) = self.cache_path(url) {
            let _ = std::fs::write(path, format!("{status}\n{body}"));
        }
    }

    async fn gate(&mut self) {
        if let Some(last) = self.last_api {
            let elapsed = last.elapsed();
            if elapsed < API_GAP {
                tokio::time::sleep(API_GAP - elapsed).await;
            }
        }
        self.last_api = Some(Instant::now());
    }

    /// `None` is a 404 or 410.
    async fn fetch(&mut self, host: Host, url: &str) -> Result<Option<String>> {
        if let Some((status, body)) = self.cache_read(url) {
            return Ok((status == 200).then_some(body));
        }
        let mut last_error = String::new();
        for attempt in 0..4u32 {
            if host == Host::Crates {
                self.gate().await;
            }
            let mut request = self.client.get(url);
            match host {
                Host::Github => {
                    request = request
                        .header("Accept", "application/vnd.github+json")
                        .header("X-GitHub-Api-Version", "2022-11-28");
                    if let Some(token) = &self.token {
                        request = request.bearer_auth(token);
                    }
                }
                Host::Crates => request = request.header("Accept", "application/json"),
                Host::Raw | Host::Index => {}
            }
            let response = match request.send().await {
                Ok(r) => r,
                Err(e) => {
                    last_error = format!("{url}: {e}");
                    tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
                    continue;
                }
            };
            let status = response.status();
            match status.as_u16() {
                200 => {
                    let body = response.text().await.with_context(|| url.to_owned())?;
                    self.cache_write(url, 200, &body);
                    return Ok(Some(body));
                }
                404 | 410 => {
                    self.cache_write(url, 404, "");
                    return Ok(None);
                }
                code => {
                    let remaining = response
                        .headers()
                        .get("x-ratelimit-remaining")
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_owned);
                    let reset = response
                        .headers()
                        .get("x-ratelimit-reset")
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_owned);
                    let retry_after = response
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok());
                    if code == 403 && remaining.as_deref() == Some("0") {
                        bail!(
                            "GitHub rate limit exhausted on {url} (resets at epoch {}); set GITHUB_TOKEN or GH_TOKEN",
                            reset.unwrap_or_default()
                        );
                    }
                    last_error = format!("{url}: HTTP {code}");
                    if code == 429 || code >= 500 {
                        let wait = retry_after.unwrap_or(1 << attempt).min(60);
                        tokio::time::sleep(Duration::from_secs(wait)).await;
                        continue;
                    }
                    bail!("{last_error}");
                }
            }
        }
        bail!("{last_error}")
    }

    async fn github<T: for<'de> Deserialize<'de>>(&mut self, path: &str) -> Result<Option<T>> {
        let url = format!("https://api.github.com{path}");
        match self.fetch(Host::Github, &url).await? {
            Some(body) => Ok(Some(serde_json::from_str(&body).with_context(|| url)?)),
            None => Ok(None),
        }
    }

    /// Index entries of a crate, trying the `-` and `_` spellings crates.io accepts.
    async fn index(&mut self, name: &str) -> Result<Option<(String, Vec<IndexEntry>)>> {
        let mut spellings = vec![name.to_owned(), name.replace('_', "-"), name.replace('-', "_")];
        spellings.dedup();
        for spelling in spellings {
            if let Some(body) = self.fetch(Host::Index, &index_url(&spelling)).await? {
                return Ok(Some((spelling, parse_index(&body))));
            }
        }
        Ok(None)
    }

    async fn facts(&mut self, name: &str) -> Result<Facts> {
        let Some((spelling, entries)) = self.index(name).await? else {
            return Ok(Facts::default());
        };
        let versions: Vec<(Version, bool)> = entries
            .iter()
            .filter_map(|e| Some((Version::parse(&e.vers).ok()?, e.yanked)))
            .collect();
        let mut facts = Facts {
            exists: true,
            versions,
            ..Facts::default()
        };
        let url = format!("https://crates.io/api/v1/crates/{spelling}");
        if let Some(body) = self.fetch(Host::Crates, &url).await? {
            let api: ApiCrate = serde_json::from_str(&body).with_context(|| url)?;
            facts.repository = api.krate.repository.filter(|r| !r.is_empty());
            let pick = facts.latest().map(|v| v.to_string());
            facts.license = api
                .versions
                .iter()
                .find(|v| Some(&v.num) == pick.as_ref())
                .or_else(|| api.versions.first())
                .and_then(|v| v.license.clone())
                .filter(|l| !l.is_empty());
        }
        Ok(facts)
    }
}

// ---------------------------------------------------------------------------------------------
// GitHub
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GhRepo {
    full_name: String,
    html_url: String,
    default_branch: String,
    license: Option<GhLicense>,
}

#[derive(Debug, Deserialize)]
struct GhLicense {
    spdx_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    published_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhTag {
    name: String,
}

#[derive(Debug, Deserialize)]
struct GhTree {
    #[serde(default)]
    tree: Vec<GhTreeEntry>,
    #[serde(default)]
    truncated: bool,
}

#[derive(Debug, Deserialize)]
struct GhTreeEntry {
    path: String,
    #[serde(rename = "type")]
    kind: String,
}

struct RepoMeta {
    full_name: String,
    requested: String,
    url: String,
    license: Option<String>,
    default_branch: String,
    release: Option<Release>,
    git_ref: String,
    ref_kind: String,
}

impl RepoMeta {
    /// Names under which crates.io may link the repository.
    fn aliases(&self) -> Vec<String> {
        let mut names = vec![self.full_name.to_ascii_lowercase(), self.requested.to_ascii_lowercase()];
        names.dedup();
        names
    }
}

fn repo_matches(repository: &str, aliases: &[String]) -> bool {
    let repo = repository
        .to_ascii_lowercase()
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .to_owned();
    aliases.iter().any(|a| repo.ends_with(&format!("github.com/{a}")))
}

async fn raw(net: &mut Net, repo: &str, git_ref: &str, path: &str) -> Result<Option<String>> {
    let url = format!(
        "https://raw.githubusercontent.com/{repo}/{}/{}",
        percent(git_ref, true),
        percent(path, true)
    );
    net.fetch(Host::Raw, &url).await
}

async fn manifest_dirs(net: &mut Net, repo: &str, git_ref: &str) -> Result<(BTreeSet<String>, bool)> {
    let path = format!("/repos/{repo}/git/trees/{}?recursive=1", percent(git_ref, false));
    let tree: GhTree = net
        .github(&path)
        .await?
        .ok_or_else(|| anyhow!("git tree of {repo}@{git_ref} not found"))?;
    let dirs = tree
        .tree
        .iter()
        .filter(|e| e.kind == "blob")
        .filter_map(|e| {
            e.path
                .strip_suffix("/Cargo.toml")
                .map(str::to_owned)
                .or_else(|| (e.path == "Cargo.toml").then(String::new))
        })
        .collect();
    Ok((dirs, tree.truncated))
}

// ---------------------------------------------------------------------------------------------
// Analysis (pure)
// ---------------------------------------------------------------------------------------------

struct Subject {
    name: String,
    reqs: BTreeSet<String>,
    git: Option<String>,
    alt_registry: Option<String>,
    path_only: bool,
    used_by: BTreeSet<String>,
}

struct Verdict {
    decision: &'static str,
    reason: String,
    license: Option<String>,
    latest: Option<String>,
    nie_locked: Option<String>,
    compatible: Option<bool>,
}

struct MemberFacts<'a> {
    manifest: &'a Manifest,
    published: bool,
    why_not: Option<String>,
}

fn lock_status(lock: &NieLock, subject: &Subject) -> (Option<String>, Option<bool>) {
    let Some(versions) = lock.get(&subject.name) else {
        return (None, None);
    };
    let locked = versions
        .iter()
        .map(Version::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let compatible = subject
        .reqs
        .iter()
        .all(|r| versions.iter().any(|v| req_admits(r, v).unwrap_or(true)));
    (Some(locked), Some(compatible))
}

fn classify(
    subject: &Subject,
    member: Option<&MemberFacts<'_>>,
    facts: &Facts,
    lock: &NieLock,
    policy: &Policy,
) -> Verdict {
    let latest = facts.latest().map(|v| v.to_string());
    let (nie_locked, compatible) = lock_status(lock, subject);
    let done = |decision: &'static str, reason: String| Verdict {
        decision,
        reason,
        license: facts.license.clone(),
        latest: latest.clone(),
        nie_locked: nie_locked.clone(),
        compatible,
    };

    if policy.lua.contains(&subject.name) {
        return done("rejected", policy.lua.reason.clone());
    }
    if let Some(member) = member {
        if !member.published {
            let why = member
                .why_not
                .clone()
                .unwrap_or_else(|| "workspace member not on crates.io".to_owned());
            return done("design-only", why);
        }
    } else {
        if let Some(registry) = &subject.alt_registry {
            return done("rejected", format!("alternate registry `{registry}`"));
        }
        if !facts.exists {
            return match &subject.git {
                Some(url) => done("rejected", format!("git-only source {url}")),
                None if subject.path_only => {
                    done("design-only", "path dependency outside the workspace members".to_owned())
                }
                None => done("rejected", "not found on crates.io".to_owned()),
            };
        }
    }
    if !facts.versions.is_empty() && facts.versions.iter().all(|(_, yanked)| *yanked) {
        return done("rejected", "every published version is yanked".to_owned());
    }
    if let Some(locked) = &nie_locked {
        let state = if compatible == Some(true) {
            "requirement compatible"
        } else {
            "requirement not satisfied by the locked version"
        };
        return done("already-in-nie", format!("nie locks {locked}; {state}"));
    }
    let Some(license) = facts.license.clone() else {
        return done("rejected", "no SPDX licence recorded on crates.io".to_owned());
    };
    let (verdict, offender) = license_verdict(&license, &policy.licenses);
    let offender = offender.unwrap_or_default();
    match verdict {
        Lic::Allowed => {
            let mut reason = format!("published, licence {license}, not yanked");
            if let Some(url) = &subject.git {
                reason.push_str(&format!("; upstream builds from git {url}"));
            }
            if let Some(member) = member {
                if let (Some(repo), Some(now)) = (&member.manifest.version, &latest) {
                    reason.push_str(&format!("; crates.io {now}, repository {repo} ({})", lag(repo, now)));
                }
            }
            done("crates-io", reason)
        }
        Lic::Denied if policy.design_cited.contains(&subject.name) => {
            done("design-only", format!("{}: {offender}", policy.design_cited.reason))
        }
        Lic::Denied => done(
            "rejected",
            format!("licence {license} ({offender}) is incompatible with MIT OR Apache-2.0"),
        ),
        Lic::Unknown => done(
            "rejected",
            format!("licence {license} ({offender}) is not in the policy allow list"),
        ),
    }
}

/// `behind`, `current` or `ahead`: the repository version compared with the crates.io one.
fn lag(repo: &str, crates_io: &str) -> &'static str {
    match (Version::parse(repo), Version::parse(crates_io)) {
        (Ok(r), Ok(c)) if r > c => "crates.io behind",
        (Ok(r), Ok(c)) if r < c => "crates.io ahead",
        (Ok(_), Ok(_)) => "current",
        _ => "unknown",
    }
}

struct Input<'a> {
    meta: &'a RepoMeta,
    scope: &'a str,
    note: Option<String>,
    manifests: &'a [Manifest],
    facts: &'a HashMap<String, Facts>,
    lock: &'a NieLock,
    policy: &'a Policy,
}

fn member_name(m: &Manifest) -> Option<&str> {
    m.name.as_deref()
}

/// Crate names whose crates.io facts the analysis needs.
fn needed_crates(manifests: &[Manifest], scope: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let members: BTreeSet<String> = manifests.iter().filter_map(member_name).map(canon).collect();
    for m in manifests {
        if let Some(name) = member_name(m) {
            if m.publish {
                names.insert(name.to_owned());
            }
        }
        if scope == "members" {
            continue;
        }
        for dep in &m.deps {
            if !members.contains(&canon(&dep.name)) {
                names.insert(dep.name.clone());
            }
        }
    }
    names
}

fn analyze(input: &Input<'_>) -> Source {
    let Input {
        meta,
        scope,
        manifests,
        facts,
        lock,
        policy,
        ..
    } = input;
    let empty = Facts::default();
    let aliases = meta.aliases();
    let members: Vec<&Manifest> = manifests.iter().filter(|m| m.name.is_some()).collect();
    let member_names: BTreeSet<String> = members.iter().filter_map(|m| member_name(m)).map(canon).collect();

    // Direct dependencies aggregated by crate, members included (their `used_by`).
    let mut subjects: BTreeMap<String, Subject> = BTreeMap::new();
    for m in &members {
        let user = m.name.clone().unwrap_or_default();
        for dep in &m.deps {
            let s = subjects.entry(canon(&dep.name)).or_insert_with(|| Subject {
                name: dep.name.clone(),
                reqs: BTreeSet::new(),
                git: None,
                alt_registry: None,
                path_only: false,
                used_by: BTreeSet::new(),
            });
            if let Some(req) = &dep.req {
                s.reqs.insert(req.clone());
            }
            match &dep.src {
                Src::Git(url) => s.git = Some(url.clone()),
                Src::AltRegistry(r) => s.alt_registry = Some(r.clone()),
                Src::Path => s.path_only = true,
                Src::Registry => s.path_only = false,
            }
            s.used_by.insert(user.clone());
        }
    }

    let mut rows = Vec::new();
    let mut member_rows = Vec::new();
    for m in &members {
        let name = m.name.clone().unwrap_or_default();
        let key = canon(&name);
        let f = facts.get(&key).unwrap_or(&empty);
        let (published, why_not) = if !m.publish {
            (false, Some("workspace member with publish = false".to_owned()))
        } else if !f.exists {
            (false, Some("workspace member not on crates.io".to_owned()))
        } else if let Some(repository) = f.repository.as_ref().filter(|r| !repo_matches(r, &aliases)) {
            (false, Some(format!("crates.io name belongs to {repository}")))
        } else {
            (true, None)
        };
        let mf = MemberFacts {
            manifest: m,
            published,
            why_not,
        };
        let subject = subjects.remove(&key).unwrap_or_else(|| Subject {
            name: name.clone(),
            reqs: BTreeSet::new(),
            git: None,
            alt_registry: None,
            path_only: false,
            used_by: BTreeSet::new(),
        });
        let latest = published.then(|| f.latest().map(|v| v.to_string())).flatten();
        let mut verdict = classify(&subject, Some(&mf), f, lock, policy);
        if !published {
            verdict.latest = None;
        }
        member_rows.push(Member {
            name: name.clone(),
            path: if m.dir.is_empty() { ".".to_owned() } else { m.dir.clone() },
            version: m.version.clone(),
            publish: m.publish,
            features: m.features.clone(),
            dependencies: m.deps.len(),
            published,
            crates_io_latest: latest.clone(),
            lag: match (&m.version, &latest) {
                (Some(repo), Some(now)) => Some(lag(repo, now).to_owned()),
                _ => None,
            },
            license: m.license.clone().or_else(|| f.license.clone()),
        });
        rows.push(row(
            &name,
            "member",
            m.version.clone().unwrap_or_else(|| "-".to_owned()),
            verdict,
            &subject.used_by,
        ));
    }
    if *scope != "members" {
        for (key, subject) in &subjects {
            if member_names.contains(key) {
                continue;
            }
            let f = facts.get(key).unwrap_or(&empty);
            let verdict = classify(subject, None, f, lock, policy);
            let requirement = if subject.reqs.is_empty() {
                "-".to_owned()
            } else {
                subject.reqs.iter().cloned().collect::<Vec<_>>().join(", ")
            };
            rows.push(row(&subject.name, "dependency", requirement, verdict, &subject.used_by));
        }
    }
    rows.sort_by(|a, b| (canon(&a.krate), &a.kind).cmp(&(canon(&b.krate), &b.kind)));
    member_rows.sort_by(|a, b| a.name.cmp(&b.name));
    let mut counts = BTreeMap::new();
    for r in &rows {
        *counts.entry(r.decision.clone()).or_insert(0) += 1;
    }
    let published = member_rows.iter().filter(|m| m.published).count();
    Source {
        repo: meta.full_name.clone(),
        url: meta.url.clone(),
        license: meta.license.clone(),
        default_branch: meta.default_branch.clone(),
        latest_release: meta.release.clone(),
        git_ref: meta.git_ref.clone(),
        ref_kind: meta.ref_kind.clone(),
        scope: (*scope).to_owned(),
        note: input.note.clone(),
        members_published: published,
        members_unpublished: member_rows.len() - published,
        counts,
        members: member_rows,
        decisions: rows,
    }
}

fn row(name: &str, kind: &str, requirement: String, v: Verdict, used_by: &BTreeSet<String>) -> Row {
    Row {
        krate: name.to_owned(),
        kind: kind.to_owned(),
        requirement,
        decision: v.decision.to_owned(),
        reason: v.reason,
        license: v.license,
        latest: v.latest,
        used_by: used_by.iter().cloned().collect(),
        nie_locked: v.nie_locked,
        compatible: v.compatible,
    }
}

/// A repository without a Cargo workspace: only its design or protocol specification can be cited.
fn design_only_source(meta: &RepoMeta, note: String) -> Source {
    let mut counts = BTreeMap::new();
    counts.insert("design-only".to_owned(), 1);
    Source {
        repo: meta.full_name.clone(),
        url: meta.url.clone(),
        license: meta.license.clone(),
        default_branch: meta.default_branch.clone(),
        latest_release: meta.release.clone(),
        git_ref: meta.git_ref.clone(),
        ref_kind: meta.ref_kind.clone(),
        scope: "design-only".to_owned(),
        note: Some(note.clone()),
        members_published: 0,
        members_unpublished: 0,
        counts,
        members: Vec::new(),
        decisions: vec![Row {
            krate: meta.full_name.clone(),
            kind: "spec".to_owned(),
            requirement: "-".to_owned(),
            decision: "design-only".to_owned(),
            reason: note,
            license: meta.license.clone(),
            latest: meta.release.as_ref().map(|r| r.tag.clone()),
            used_by: Vec::new(),
            nie_locked: None,
            compatible: None,
        }],
    }
}

// ---------------------------------------------------------------------------------------------
// Reading one repository
// ---------------------------------------------------------------------------------------------

async fn read_repo(net: &mut Net, policy: &Policy, lock: &NieLock, requested: &str) -> Result<Source> {
    let repo: GhRepo = net
        .github(&format!("/repos/{requested}"))
        .await?
        .ok_or_else(|| anyhow!("GitHub repository {requested} not found"))?;
    let full = repo.full_name.clone();
    let release = match net.github::<GhRelease>(&format!("/repos/{full}/releases/latest")).await? {
        Some(r) => Some(Release {
            tag: r.tag_name,
            published_at: r.published_at,
        }),
        None => net
            .github::<Vec<GhTag>>(&format!("/repos/{full}/tags?per_page=1"))
            .await?
            .and_then(|t| t.into_iter().next())
            .map(|t| Release {
                tag: t.name,
                published_at: None,
            }),
    };
    let scope = policy
        .repo
        .get(&full.to_ascii_lowercase())
        .and_then(|p| p.scope.clone())
        .unwrap_or_else(|| "full".to_owned());

    // Root manifest at the latest release tag, else at the default branch.
    let mut git_ref = release.as_ref().map_or(repo.default_branch.clone(), |r| r.tag.clone());
    let mut ref_kind = if release.is_some() { "release" } else { "default-branch" }.to_owned();
    let mut root = raw(net, &full, &git_ref, "Cargo.toml").await?;
    if root.is_none() && ref_kind == "release" {
        git_ref = repo.default_branch.clone();
        ref_kind = "default-branch".to_owned();
        root = raw(net, &full, &git_ref, "Cargo.toml").await?;
    }
    let meta = RepoMeta {
        full_name: full.clone(),
        requested: requested.to_owned(),
        url: repo.html_url.clone(),
        license: repo
            .license
            .and_then(|l| l.spdx_id)
            .filter(|l| l != "NOASSERTION"),
        default_branch: repo.default_branch.clone(),
        release,
        git_ref: git_ref.clone(),
        ref_kind,
    };
    let Some(root) = root.filter(|_| scope != "design-only") else {
        let note = if scope == "design-only" {
            "scope set to design-only by the policy: no code is taken".to_owned()
        } else {
            "no Cargo workspace: only the design or protocol specification can be cited, no code is taken".to_owned()
        };
        return Ok(design_only_source(&meta, note));
    };

    let root_manifest = parse_manifest(&root, "", None).with_context(|| format!("{full}: root Cargo.toml"))?;
    let (dirs, truncated) = manifest_dirs(net, &full, &git_ref).await?;
    let ws = root_manifest.ws.clone();
    let mut member_dirs = ws
        .as_ref()
        .map(|w| expand_members(&w.members, &w.exclude, &dirs))
        .unwrap_or_default();
    let mut manifests = Vec::new();
    if root_manifest.name.is_some() {
        manifests.push(root_manifest);
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut note = truncated.then(|| "GitHub truncated the git tree: some members may be missing".to_owned());
    while let Some(dir) = member_dirs.iter().find(|d| !seen.contains(*d)).cloned() {
        seen.insert(dir.clone());
        let Some(text) = raw(net, &full, &git_ref, &format!("{dir}/Cargo.toml")).await? else {
            continue;
        };
        match parse_manifest(&text, &dir, ws.as_ref()) {
            Ok(m) => {
                for dep in &m.deps {
                    if let (Src::Path, Some(path)) = (&dep.src, &dep.path) {
                        if dirs.contains(path) && !path.is_empty() {
                            member_dirs.insert(path.clone());
                        }
                    }
                }
                manifests.push(m);
            }
            Err(e) => {
                note = Some(format!("{dir}/Cargo.toml skipped: {e:#}"));
            }
        }
    }

    let wanted = needed_crates(&manifests, &scope);
    let mut facts = HashMap::new();
    for (i, name) in wanted.iter().enumerate() {
        if i % 25 == 0 {
            eprintln!("{full}: crates.io {i}/{}", wanted.len());
        }
        facts.insert(canon(name), net.facts(name).await?);
    }
    Ok(analyze(&Input {
        meta: &meta,
        scope: &scope,
        note,
        manifests: &manifests,
        facts: &facts,
        lock,
        policy,
    }))
}

// ---------------------------------------------------------------------------------------------
// crates.io probe
// ---------------------------------------------------------------------------------------------

async fn read_probe(net: &mut Net, policy: &Policy, lock: &NieLock) -> Result<Probe> {
    let mut crates = BTreeMap::new();
    for name in &policy.probe.crates {
        let probe = match net.index(name).await? {
            Some((_, entries)) => {
                let versions: Vec<(Version, bool)> = entries
                    .iter()
                    .filter_map(|e| Some((Version::parse(&e.vers).ok()?, e.yanked)))
                    .collect();
                let latest = latest_of(&versions);
                let prerelease = versions
                    .iter()
                    .filter(|(v, y)| !y && !v.pre.is_empty())
                    .map(|(v, _)| v)
                    .max()
                    .filter(|v| latest.as_ref().is_none_or(|l| *v > l))
                    .cloned();
                ProbeCrate {
                    exists: true,
                    latest: latest.map(|v| v.to_string()),
                    latest_prerelease: prerelease.map(|v| v.to_string()),
                }
            }
            None => ProbeCrate {
                exists: false,
                latest: None,
                latest_prerelease: None,
            },
        };
        crates.insert(name.clone(), probe);
    }
    let requirement = match &policy.probe.requirement {
        Some(p) => Some(read_requirement(net, lock, p).await?),
        None => None,
    };
    Ok(Probe { crates, requirement })
}

async fn read_requirement(net: &mut Net, lock: &NieLock, policy: &ProbeRequirementPolicy) -> Result<ProbeRequirement> {
    let mut targets: Vec<ProbeTarget> = policy
        .targets
        .iter()
        .map(|t| ProbeTarget {
            source: "policy".to_owned(),
            version: t.clone(),
        })
        .collect();
    if let Some(version) = policy
        .lock_package
        .as_deref()
        .and_then(|p| lock.get(p))
        .and_then(|v| v.iter().max())
    {
        targets.push(ProbeTarget {
            source: "nie Cargo.lock".to_owned(),
            version: version.to_string(),
        });
    }
    if let Some((_, entries)) = net.index(&policy.dependency).await? {
        let versions: Vec<(Version, bool)> = entries
            .iter()
            .filter_map(|e| Some((Version::parse(&e.vers).ok()?, e.yanked)))
            .collect();
        if let Some(latest) = latest_of(&versions) {
            targets.push(ProbeTarget {
                source: "crates.io latest".to_owned(),
                version: latest.to_string(),
            });
        }
    }
    targets.dedup_by(|a, b| a.version == b.version && a.source == b.source);

    let mut out = ProbeRequirement {
        krate: policy.krate.clone(),
        version: None,
        dependency: policy.dependency.clone(),
        targets,
        requirements: Vec::new(),
    };
    let Some((_, entries)) = net.index(&policy.krate).await? else {
        return Ok(out);
    };
    let versions: Vec<(Version, bool)> = entries
        .iter()
        .filter_map(|e| Some((Version::parse(&e.vers).ok()?, e.yanked)))
        .collect();
    let Some(latest) = latest_of(&versions) else {
        return Ok(out);
    };
    out.version = Some(latest.to_string());
    let published = entries.iter().find(|e| e.vers == latest.to_string());
    for dep in published.map(|e| e.deps.as_slice()).unwrap_or_default() {
        if canon(dep.package.as_deref().unwrap_or(&dep.name)) != canon(&policy.dependency) {
            continue;
        }
        let mut admits = BTreeMap::new();
        for target in &out.targets {
            if let Ok(v) = Version::parse(&target.version) {
                admits.insert(target.version.clone(), req_admits(&dep.req, &v).unwrap_or(false));
            }
        }
        out.requirements.push(ProbeDep {
            kind: dep.kind.clone().unwrap_or_else(|| "normal".to_owned()),
            requirement: dep.req.clone(),
            optional: dep.optional,
            admits,
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Markdown
// ---------------------------------------------------------------------------------------------

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn opt(s: &Option<String>) -> String {
    s.as_deref().map_or("-".to_owned(), cell)
}

fn short_list(items: &[String], max: usize) -> String {
    if items.len() <= max {
        items.join(", ")
    } else {
        format!("{} +{}", items[..max].join(", "), items.len() - max)
    }
}

fn marker(id: &str) -> (String, String) {
    (format!("<!-- generated:{id} -->"), format!("<!-- /generated:{id} -->"))
}

fn decision_table(rows: &[&Row], max_rows: usize) -> String {
    let mut out = String::from(
        "| crate | requirement | decision | reason | licence | latest | used by |\n|---|---|---|---|---|---|---|\n",
    );
    for r in rows.iter().take(max_rows) {
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} |\n",
            cell(&r.krate),
            cell(&r.requirement),
            r.decision,
            cell(&r.reason),
            opt(&r.license),
            opt(&r.latest),
            cell(&short_list(&r.used_by, 4)),
        ));
    }
    if rows.len() > max_rows {
        out.push_str(&format!(
            "\n{} more rows, see `{JSON_FILE}`.\n",
            rows.len() - max_rows
        ));
    }
    out
}

fn render_source(src: &Source, max_rows: usize) -> String {
    let (open, close) = marker(&src.repo);
    let mut md = format!("{open}\n## {}\n\n", src.repo);
    md.push_str(&format!("- Repository: <{}>\n", src.url));
    md.push_str(&format!("- Licence: {}\n", src.license.as_deref().unwrap_or("none detected")));
    md.push_str(&format!("- Default branch: {}\n", src.default_branch));
    match &src.latest_release {
        Some(r) => md.push_str(&format!(
            "- Latest release: {}{}\n",
            r.tag,
            r.published_at.as_ref().map_or(String::new(), |d| format!(" ({d})"))
        )),
        None => md.push_str("- Latest release: none\n"),
    }
    md.push_str(&format!("- Read at: {} ({}), scope {}\n", src.git_ref, src.ref_kind, src.scope));
    if let Some(note) = &src.note {
        md.push_str(&format!("- Note: {note}\n"));
    }
    if src.scope != "design-only" {
        md.push_str(&format!(
            "- Workspace members: {} published on crates.io, {} not published\n",
            src.members_published, src.members_unpublished
        ));
    }
    let counts = src
        .counts
        .iter()
        .map(|(k, v)| format!("{k} {v}"))
        .collect::<Vec<_>>()
        .join(", ");
    md.push_str(&format!("- Decisions: {counts}\n"));

    if src.scope == "design-only" {
        md.push('\n');
        let refs: Vec<&Row> = src.decisions.iter().collect();
        md.push_str(&decision_table(&refs, max_rows));
    } else {
        // Members: for the members scope only the crates nie already locks.
        let locked: BTreeSet<String> = src
            .decisions
            .iter()
            .filter(|r| r.nie_locked.is_some())
            .map(|r| canon(&r.krate))
            .collect();
        let members: Vec<&Member> = src
            .members
            .iter()
            .filter(|m| src.scope != "members" || locked.contains(&canon(&m.name)))
            .collect();
        md.push_str(&format!(
            "\n### Members{}\n\n| member | repository | crates.io | state |\n|---|---|---|---|\n",
            if src.scope == "members" { " nie locks" } else { "" }
        ));
        for m in members.iter().take(max_rows) {
            let state = match (&m.lag, m.published) {
                (Some(l), _) => l.clone(),
                (None, true) => "published".to_owned(),
                (None, false) if !m.publish => "publish = false".to_owned(),
                (None, false) => "not published".to_owned(),
            };
            md.push_str(&format!(
                "| `{}` | {} | {} | {} |\n",
                cell(&m.name),
                opt(&m.version),
                opt(&m.crates_io_latest),
                cell(&state)
            ));
        }
        if members.len() > max_rows {
            md.push_str(&format!("\n{} more members, see `{JSON_FILE}`.\n", members.len() - max_rows));
        }
        let taken: Vec<&Row> = src
            .decisions
            .iter()
            .filter(|r| matches!(r.decision.as_str(), "crates-io" | "already-in-nie"))
            .filter(|r| src.scope != "members" || r.nie_locked.is_some())
            .collect();
        if !taken.is_empty() {
            md.push_str("\n### Crates nie takes\n\n");
            md.push_str(&decision_table(&taken, max_rows));
        }
        let rejected: Vec<&Row> = src.decisions.iter().filter(|r| r.decision == "rejected").collect();
        if !rejected.is_empty() {
            md.push_str("\n### Rejected\n\n");
            md.push_str(&decision_table(&rejected, max_rows));
        }
        let design: Vec<String> = src
            .decisions
            .iter()
            .filter(|r| r.decision == "design-only")
            .map(|r| format!("`{}`", r.krate))
            .collect();
        if !design.is_empty() {
            md.push_str(&format!(
                "\n### Design-only ({})\n\n{}\n",
                design.len(),
                short_list(&design, max_rows)
            ));
        }
    }
    md.push_str(&format!("\n{close}\n"));
    md
}

fn render_probe(probe: &Probe) -> String {
    let (open, close) = marker(PROBE_ID);
    let mut md = format!(
        "{open}\n## crates.io probe\n\n| crate | on crates.io | latest | latest prerelease |\n|---|---|---|---|\n"
    );
    for (name, p) in &probe.crates {
        md.push_str(&format!(
            "| `{name}` | {} | {} | {} |\n",
            if p.exists { "yes" } else { "no" },
            opt(&p.latest),
            opt(&p.latest_prerelease)
        ));
    }
    if let Some(req) = &probe.requirement {
        md.push_str(&format!(
            "\n### `{}` {} on `{}`\n\n",
            req.krate,
            req.version.as_deref().unwrap_or("(not published)"),
            req.dependency
        ));
        if req.requirements.is_empty() {
            md.push_str("No such dependency is published.\n");
        } else {
            let targets: Vec<String> = req
                .targets
                .iter()
                .map(|t| format!("{} ({})", t.version, t.source))
                .collect();
            md.push_str(&format!(
                "| kind | requirement | optional | admits {} |\n|---|---|---|---|\n",
                targets.join(", ")
            ));
            for d in &req.requirements {
                let admits: Vec<String> = d.admits.iter().map(|(v, ok)| format!("{v}: {ok}")).collect();
                md.push_str(&format!(
                    "| {} | `{}` | {} | {} |\n",
                    d.kind,
                    cell(&d.requirement),
                    d.optional,
                    admits.join(", ")
                ));
            }
        }
    }
    md.push_str(&format!("\n{close}\n"));
    md
}

const MD_HEADER: &str = "# Terminal sources\n\nGenerated by `nie deps upstream` from the live GitHub and crates.io APIs; the name and licence lists live in `terminal-sources.policy.toml`. Text between `<!-- generated:... -->` markers is rewritten on every run, everything else is kept.\n\n";
const MD_DESIGNS: &str = "## Designs cited\n\nFilled by hand: one entry per design taken from a source (no code), with the upstream file and the nie module it informs.\n";

/// Replaces the block `id` of `md`, or inserts it before the `Designs cited` section.
fn upsert_block(md: &str, id: &str, block: &str) -> String {
    let (open, close) = marker(id);
    if let (Some(start), Some(end)) = (md.find(&open), md.find(&close)) {
        if start < end {
            let end = end + close.len();
            let tail = md[end..].strip_prefix('\n').unwrap_or(&md[end..]);
            return format!("{}{}{}", &md[..start], block.trim_end_matches('\n'), if tail.is_empty() { "\n".to_owned() } else { format!("\n{tail}") });
        }
    }
    match md.find(DESIGNS_HEADING) {
        Some(at) => format!("{}{}\n{}", &md[..at], block.trim_end_matches('\n'), format!("\n{}", &md[at..])),
        None => format!("{}\n{}\n", md.trim_end(), block.trim_end_matches('\n')),
    }
}

// ---------------------------------------------------------------------------------------------
// Drift
// ---------------------------------------------------------------------------------------------

fn item_key(v: &serde_json::Value) -> Option<String> {
    let name = v.get("crate").or_else(|| v.get("name"))?.as_str()?;
    Some(match v.get("kind").and_then(serde_json::Value::as_str) {
        Some(kind) => format!("{name}/{kind}"),
        None => name.to_owned(),
    })
}

fn diff(path: &str, committed: &serde_json::Value, fresh: &serde_json::Value, out: &mut Vec<String>) {
    use serde_json::Value;
    match (committed, fresh) {
        (Value::Object(a), Value::Object(b)) => {
            let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
            for k in keys {
                let p = format!("{path}.{k}");
                match (a.get(k), b.get(k)) {
                    (Some(x), Some(y)) => diff(&p, x, y, out),
                    (Some(_), None) => out.push(format!("- {p}")),
                    (None, Some(_)) => out.push(format!("+ {p}")),
                    (None, None) => {}
                }
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            let keyed = |v: &[Value]| -> Option<BTreeMap<String, Value>> {
                v.iter().map(|i| Some((item_key(i)?, i.clone()))).collect()
            };
            match (keyed(a), keyed(b)) {
                (Some(x), Some(y)) => {
                    let keys: BTreeSet<&String> = x.keys().chain(y.keys()).collect();
                    for k in keys {
                        let p = format!("{path}[{k}]");
                        match (x.get(k), y.get(k)) {
                            (Some(i), Some(j)) => diff(&p, i, j, out),
                            (Some(_), None) => out.push(format!("- {p}")),
                            (None, Some(_)) => out.push(format!("+ {p}")),
                            (None, None) => {}
                        }
                    }
                }
                _ if a != b => out.push(format!("~ {path}: list changed")),
                _ => {}
            }
        }
        _ if committed != fresh => out.push(format!("~ {path}: {committed} -> {fresh}")),
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Command
// ---------------------------------------------------------------------------------------------

fn find_root(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(root) = explicit {
        return Ok(root.to_owned());
    }
    let cwd = std::env::current_dir()?;
    cwd.ancestors()
        .find(|d| d.join("Cargo.lock").is_file() && d.join("docs").join("specs").is_dir())
        .map(Path::to_owned)
        .ok_or_else(|| anyhow!("no nie checkout (Cargo.lock and docs/specs) above {}; pass --root", cwd.display()))
}

async fn upstream(args: UpstreamArgs) -> Result<()> {
    let root = find_root(args.root.as_deref())?;
    let out_dir = args.out.clone().unwrap_or_else(|| root.join("docs").join("specs"));
    let policy_path = args.policy.clone().unwrap_or_else(|| out_dir.join(POLICY_FILE));
    let policy: Policy = toml::from_str(
        &std::fs::read_to_string(&policy_path).with_context(|| format!("policy {}", policy_path.display()))?,
    )
    .with_context(|| format!("policy {}", policy_path.display()))?;
    let lock = NieLock::parse(
        &std::fs::read_to_string(root.join("Cargo.lock")).context("nie Cargo.lock")?,
    )?;
    let json_path = out_dir.join(JSON_FILE);
    let mut doc: Doc = match std::fs::read_to_string(&json_path) {
        Ok(text) => serde_json::from_str(&text).with_context(|| json_path.display().to_string())?,
        Err(_) => Doc {
            schema: 1,
            ..Doc::default()
        },
    };
    let committed = doc.clone();

    let mut repos = args.repos.clone();
    let mut probe = args.probe;
    if repos.is_empty() && !probe {
        repos = doc.sources.keys().cloned().collect();
        probe = doc.crates_io_probe.is_some();
        if repos.is_empty() && !probe {
            bail!("nothing to read: pass --repo <owner/name> or --probe");
        }
    }

    let mut max_age = Duration::from_secs(args.max_age_secs);
    if args.check {
        max_age = max_age.min(Duration::from_secs(3600));
    }
    let mut net = Net::new(max_age)?;
    let mut fresh_sources = BTreeMap::new();
    for repo in &repos {
        let source = read_repo(&mut net, &policy, &lock, repo).await?;
        fresh_sources.insert(source.repo.clone(), source);
    }
    let fresh_probe = if probe {
        Some(read_probe(&mut net, &policy, &lock).await?)
    } else {
        None
    };

    if args.check {
        let mut drift = Vec::new();
        for (name, source) in &fresh_sources {
            match committed.sources.get(name) {
                Some(old) => diff(
                    name,
                    &serde_json::to_value(old)?,
                    &serde_json::to_value(source)?,
                    &mut drift,
                ),
                None => drift.push(format!("+ {name}: not in the committed JSON")),
            }
        }
        if let Some(fresh) = &fresh_probe {
            match &committed.crates_io_probe {
                Some(old) => diff(
                    PROBE_ID,
                    &serde_json::to_value(old)?,
                    &serde_json::to_value(fresh)?,
                    &mut drift,
                ),
                None => drift.push(format!("+ {PROBE_ID}: not in the committed JSON")),
            }
        }
        if drift.is_empty() {
            println!("no drift against {}", json_path.display());
            return Ok(());
        }
        for line in &drift {
            println!("{line}");
        }
        bail!("{} differences against {}", drift.len(), json_path.display());
    }

    let max_rows = policy.render.max_rows;
    let md_path = out_dir.join(MD_FILE);
    let mut md = std::fs::read_to_string(&md_path)
        .unwrap_or_else(|_| format!("{MD_HEADER}{MD_DESIGNS}"));
    for (name, source) in &fresh_sources {
        md = upsert_block(&md, name, &render_source(source, max_rows));
        doc.sources.insert(name.clone(), source.clone());
    }
    if let Some(p) = &fresh_probe {
        md = upsert_block(&md, PROBE_ID, &render_probe(p));
        doc.crates_io_probe = Some(p.clone());
    }
    doc.schema = 1;
    std::fs::create_dir_all(&out_dir)?;
    std::fs::write(&json_path, format!("{}\n", serde_json::to_string_pretty(&doc)?))?;
    std::fs::write(&md_path, md)?;

    if args.json {
        let shown = Doc {
            schema: 1,
            sources: fresh_sources,
            crates_io_probe: fresh_probe,
        };
        println!("{}", serde_json::to_string_pretty(&shown)?);
    } else {
        for source in fresh_sources.values() {
            let counts = source
                .counts
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(" ");
            println!(
                "{} ref={} members={}/{} published {}",
                source.repo,
                source.git_ref,
                source.members_published,
                source.members_published + source.members_unpublished,
                counts
            );
        }
        if let Some(p) = &fresh_probe {
            println!("probe: {} crates", p.crates.len());
        }
        println!("wrote {} and {}", json_path.display(), md_path.display());
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Tests (synthetic data, no network)
// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const POLICY: &str = r#"
        [licenses]
        allow = ["MIT", "Apache-2.0", "MPL-2.0", "Unicode-3.0"]
        deny = ["GPL", "AGPL", "LGPL"]
        [lua]
        reason = "Lua VM"
        crates = ["mlua", "yazi-plugin"]
        [design_cited]
        reason = "design cited"
        crates = ["gpl-design"]
    "#;

    const ROOT: &str = r#"
        [workspace]
        members = ["crates/*", "tools/one"]
        exclude = ["crates/skip"]
        [workspace.package]
        version = "2.0.0"
        license = "MIT"
        [workspace.dependencies]
        serde = "1.0"
        mlua = { version = "0.11", features = ["vendored"] }
        gpl-crate = "3"
        gpl-design = "1"
        gitdep = { git = "https://example.com/gitdep", rev = "abc" }
        gitpub = { git = "https://example.com/gitpub", version = "0.4" }
        old = "1"
        permissive = "0.5"
        a-core = { path = "crates/a-core", version = "2.0.0" }
    "#;

    const A_CORE: &str = r#"
        [package]
        name = "a-core"
        version.workspace = true
        license.workspace = true
        [features]
        default = []
        extra = []
        [dependencies]
        serde = { workspace = true }
        permissive.workspace = true
    "#;

    const A_CLI: &str = r#"
        [package]
        name = "a-cli"
        version = "2.0.0"
        publish = false
        [dependencies]
        a-core = { workspace = true }
        mlua = { workspace = true }
        gpl-crate = { workspace = true }
        gpl-design = { workspace = true }
        gitdep = { workspace = true }
        gitpub = { workspace = true }
        old = { workspace = true }
        renamed = { package = "permissive", version = "0.5" }
        [target.'cfg(windows)'.dependencies]
        permissive = "0.5.1"
        [dev-dependencies]
        only-dev = "9"
    "#;

    fn facts(license: &str, versions: &[(&str, bool)]) -> Facts {
        Facts {
            exists: true,
            versions: versions
                .iter()
                .map(|(v, y)| (Version::parse(v).unwrap(), *y))
                .collect(),
            license: Some(license.to_owned()),
            repository: None,
        }
    }

    fn meta() -> RepoMeta {
        RepoMeta {
            full_name: "owner/proj".to_owned(),
            requested: "owner/proj".to_owned(),
            url: "https://github.com/owner/proj".to_owned(),
            license: Some("MIT".to_owned()),
            default_branch: "main".to_owned(),
            release: None,
            git_ref: "main".to_owned(),
            ref_kind: "default-branch".to_owned(),
        }
    }

    fn manifests() -> Vec<Manifest> {
        let root = parse_manifest(ROOT, "", None).unwrap();
        let ws = root.ws.clone().unwrap();
        vec![
            parse_manifest(A_CORE, "crates/a-core", Some(&ws)).unwrap(),
            parse_manifest(A_CLI, "crates/a-cli", Some(&ws)).unwrap(),
        ]
    }

    #[test]
    fn workspace_inheritance_and_dependency_sources() {
        let m = manifests();
        assert_eq!(m[0].version.as_deref(), Some("2.0.0"));
        assert_eq!(m[0].license.as_deref(), Some("MIT"));
        assert_eq!(m[0].features, ["default", "extra"]);
        assert!(!m[1].publish);
        let mlua = m[1].deps.iter().find(|d| d.name == "mlua").unwrap();
        assert_eq!(mlua.req.as_deref(), Some("0.11"));
        let git = m[1].deps.iter().find(|d| d.name == "gitdep").unwrap();
        assert_eq!(git.src, Src::Git("https://example.com/gitdep".to_owned()));
        let core = m[1].deps.iter().find(|d| d.name == "a-core").unwrap();
        assert_eq!((core.src.clone(), core.path.as_deref()), (Src::Path, Some("crates/a-core")));
        assert!(m[1].deps.iter().any(|d| d.name == "permissive" && d.req.as_deref() == Some("0.5.1")));
        assert!(m[1].deps.iter().all(|d| d.name != "only-dev"));
    }

    #[test]
    fn members_expand_globs_and_excludes() {
        let dirs: BTreeSet<String> = ["", "crates/a", "crates/skip", "tools/one", "tools/two", "crates/deep/x"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let got = expand_members(
            &["crates/*".to_owned(), "tools/one".to_owned()],
            &["crates/skip".to_owned()],
            &dirs,
        );
        assert_eq!(
            got.into_iter().collect::<Vec<_>>(),
            ["crates/a", "tools/one"]
        );
        assert!(glob_match("crates/**", "crates/deep/x"));
        assert!(glob_match("yazi-*", "yazi-fs"));
        assert!(!glob_match("yazi-*", "other"));
    }

    #[test]
    fn decisions_follow_the_policy() {
        let policy: Policy = toml::from_str(POLICY).unwrap();
        let lock = NieLock::parse(
            "[[package]]\nname = \"serde\"\nversion = \"1.0.200\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n\
             [[package]]\nname = \"mlua\"\nversion = \"0.12.2\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n\
             [[package]]\nname = \"old\"\nversion = \"2.0.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n",
        )
        .unwrap();
        let mut f = HashMap::new();
        f.insert("a_core".to_owned(), facts("MIT", &[("1.0.0", false)]));
        f.insert("serde".to_owned(), facts("MIT OR Apache-2.0", &[("1.0.200", false)]));
        f.insert("mlua".to_owned(), facts("MIT", &[("0.12.2", false)]));
        f.insert("gpl_crate".to_owned(), facts("GPL-3.0-only", &[("3.0.0", false)]));
        f.insert("gpl_design".to_owned(), facts("AGPL-3.0", &[("1.0.0", false)]));
        f.insert("gitpub".to_owned(), facts("MIT", &[("0.4.1", false)]));
        f.insert("old".to_owned(), facts("MIT", &[("1.0.0", true)]));
        f.insert("permissive".to_owned(), facts("(MIT OR Apache-2.0) AND Unicode-3.0", &[("0.5.2", false), ("0.6.0-rc.1", false)]));
        f.insert("a_cli".to_owned(), Facts::default());
        let m = manifests();
        let meta = meta();
        let src = analyze(&Input {
            meta: &meta,
            scope: "full",
            note: None,
            manifests: &m,
            facts: &f,
            lock: &lock,
            policy: &policy,
        });
        let get = |name: &str, kind: &str| src.decisions.iter().find(|r| r.krate == name && r.kind == kind).unwrap();

        let core = get("a-core", "member");
        assert_eq!(core.decision, "crates-io");
        assert!(core.reason.contains("crates.io 1.0.0, repository 2.0.0 (crates.io behind)"), "{}", core.reason);
        assert_eq!(core.used_by, ["a-cli"]);
        assert_eq!(get("a-cli", "member").decision, "design-only");
        assert_eq!(get("mlua", "dependency").decision, "rejected");
        assert_eq!(get("mlua", "dependency").reason, "Lua VM");
        assert_eq!(get("mlua", "dependency").nie_locked.as_deref(), Some("0.12.2"));
        assert_eq!(get("serde", "dependency").decision, "already-in-nie");
        assert_eq!(get("serde", "dependency").compatible, Some(true));
        assert_eq!(get("serde", "dependency").used_by, ["a-core"]);
        assert_eq!(get("gpl-crate", "dependency").decision, "rejected");
        assert!(get("gpl-crate", "dependency").reason.contains("incompatible"));
        assert_eq!(get("gpl-design", "dependency").decision, "design-only");
        assert_eq!(get("gitdep", "dependency").decision, "rejected");
        assert!(get("gitdep", "dependency").reason.starts_with("git-only source"));
        assert_eq!(get("gitpub", "dependency").decision, "crates-io");
        assert!(get("gitpub", "dependency").reason.contains("git https://example.com/gitpub"));
        // `old` is locked in nie but every crates.io version is yanked: yanked wins.
        assert_eq!(get("old", "dependency").decision, "rejected");
        assert_eq!(get("old", "dependency").reason, "every published version is yanked");
        let permissive = get("permissive", "dependency");
        assert_eq!(permissive.decision, "crates-io");
        assert_eq!(permissive.latest.as_deref(), Some("0.5.2"));
        assert_eq!(permissive.requirement, "0.5, 0.5.1");
        assert_eq!(permissive.used_by, ["a-cli", "a-core"]);
        assert!(src.decisions.iter().all(|r| r.krate != "only-dev"));
        assert_eq!(src.counts.get("rejected"), Some(&4));
        assert_eq!((src.members_published, src.members_unpublished), (1, 1));
        let keys: Vec<_> = src.decisions.iter().map(|r| canon(&r.krate)).collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
    }

    #[test]
    fn incompatible_requirement_is_recorded() {
        let policy: Policy = toml::from_str(POLICY).unwrap();
        let lock = NieLock::parse(
            "[[package]]\nname = \"serde\"\nversion = \"1.0.200\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n",
        )
        .unwrap();
        let subject = Subject {
            name: "serde".to_owned(),
            reqs: ["2".to_owned()].into(),
            git: None,
            alt_registry: None,
            path_only: false,
            used_by: BTreeSet::new(),
        };
        let v = classify(&subject, None, &facts("MIT", &[("1.0.200", false)]), &lock, &policy);
        assert_eq!((v.decision, v.compatible), ("already-in-nie", Some(false)));
    }

    #[test]
    fn member_owned_by_another_repository_is_unpublished() {
        let policy: Policy = toml::from_str(POLICY).unwrap();
        let m = manifests();
        let mut f = HashMap::new();
        let mut taken = facts("MIT", &[("0.1.0", false)]);
        taken.repository = Some("https://github.com/someone/else".to_owned());
        f.insert("a_core".to_owned(), taken);
        let meta = meta();
        let src = analyze(&Input {
            meta: &meta,
            scope: "members",
            note: None,
            manifests: &m,
            facts: &f,
            lock: &NieLock::default(),
            policy: &policy,
        });
        let core = src.decisions.iter().find(|r| r.krate == "a-core").unwrap();
        assert_eq!(core.decision, "design-only");
        assert!(core.reason.contains("someone/else"));
        // the members scope carries no dependency rows
        assert!(src.decisions.iter().all(|r| r.kind == "member"));
        assert!(repo_matches("https://github.com/Owner/Proj.git/", &["owner/proj".to_owned()]));
    }

    #[test]
    fn license_expressions() {
        let p: Policy = toml::from_str(POLICY).unwrap();
        let verdict = |e: &str| license_verdict(e, &p.licenses).0;
        assert_eq!(verdict("MIT OR Apache-2.0"), Lic::Allowed);
        assert_eq!(verdict("MIT/Apache-2.0"), Lic::Allowed);
        assert_eq!(verdict("Apache-2.0 WITH LLVM-exception"), Lic::Allowed);
        assert_eq!(verdict("GPL-3.0-or-later OR MIT"), Lic::Allowed);
        assert_eq!(verdict("MIT AND GPL-2.0"), Lic::Denied);
        assert_eq!(verdict("LGPL-2.1+"), Lic::Denied);
        assert_eq!(verdict("BUSL-1.1"), Lic::Unknown);
        assert_eq!(verdict("(MIT OR Apache-2.0) AND Unicode-3.0"), Lic::Allowed);
        assert_eq!(license_verdict("MIT AND Weird", &p.licenses).1.as_deref(), Some("Weird"));
    }

    #[test]
    fn versions_and_index_paths() {
        let v = |s: &str| Version::parse(s).unwrap();
        assert_eq!(
            latest_of(&[(v("1.0.0"), false), (v("1.2.0"), true), (v("2.0.0-rc.1"), false)]),
            Some(v("1.0.0"))
        );
        assert_eq!(latest_of(&[(v("0.1.0-alpha.1"), false)]), Some(v("0.1.0-alpha.1")));
        assert_eq!(index_url("ratatui"), "https://index.crates.io/ra/ta/ratatui");
        assert_eq!(index_url("lru"), "https://index.crates.io/3/l/lru");
        assert_eq!(index_url("ab"), "https://index.crates.io/2/ab");
        assert_eq!(lag("26.9.1", "26.5.6"), "crates.io behind");
        assert_eq!(req_admits("^0.20.0", &v("0.20.1")), Some(true));
        assert_eq!(req_admits("^0.19", &v("0.20.1")), Some(false));
        assert_eq!(join_path("crates/a", "../b/./c"), "crates/b/c");
    }

    #[test]
    fn index_lines_parse_with_dependency_requirements() {
        let body = r#"{"name":"x","vers":"0.1.0","deps":[{"name":"bevy","req":"^0.20.0","features":[],"optional":false,"default_features":false,"target":null,"kind":"normal"}],"yanked":false}
{"name":"x","vers":"0.2.0","deps":[],"yanked":true}"#;
        let entries = parse_index(body);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].deps[0].req, "^0.20.0");
        assert!(entries[1].yanked);
    }

    #[test]
    fn markdown_blocks_accumulate_and_replace() {
        let base = format!("{MD_HEADER}{MD_DESIGNS}");
        let a = upsert_block(&base, "x/a", "<!-- generated:x/a -->\nfirst\n<!-- /generated:x/a -->\n");
        let ab = upsert_block(&a, "x/b", "<!-- generated:x/b -->\nsecond\n<!-- /generated:x/b -->\n");
        assert!(ab.find("first").unwrap() < ab.find("second").unwrap());
        assert!(ab.find("second").unwrap() < ab.find(DESIGNS_HEADING).unwrap());
        let hand = ab.replace("Filled by hand", "Filled by hand: MY NOTES");
        let again = upsert_block(&hand, "x/a", "<!-- generated:x/a -->\nreplaced\n<!-- /generated:x/a -->\n");
        assert!(again.contains("replaced") && !again.contains("first"));
        assert!(again.contains("second") && again.contains("MY NOTES"));
        assert_eq!(again.matches("<!-- generated:x/a -->").count(), 1);
        assert_eq!(again.matches("<!-- /generated:x/a -->").count(), 1);
        // idempotent
        let twice = upsert_block(&again, "x/a", "<!-- generated:x/a -->\nreplaced\n<!-- /generated:x/a -->\n");
        assert_eq!(again, twice);
    }

    #[test]
    fn design_only_repository_has_one_row() {
        let src = design_only_source(&meta(), "no Cargo workspace".to_owned());
        assert_eq!(src.counts.get("design-only"), Some(&1));
        assert_eq!(src.decisions[0].kind, "spec");
        let md = render_source(&src, 80);
        assert!(md.starts_with("<!-- generated:owner/proj -->"));
        assert!(md.trim_end().ends_with("<!-- /generated:owner/proj -->"));
    }

    #[test]
    fn drift_reports_changed_added_and_removed_rows() {
        let old = serde_json::json!({"latest": "1.0", "rows": [{"crate": "a", "kind": "dependency", "decision": "crates-io"}, {"crate": "b", "kind": "member"}]});
        let new = serde_json::json!({"latest": "1.1", "rows": [{"crate": "a", "kind": "dependency", "decision": "rejected"}, {"crate": "c", "kind": "member"}]});
        let mut out = Vec::new();
        diff("s", &old, &new, &mut out);
        out.sort();
        assert_eq!(
            out,
            [
                "+ s.rows[c/member]",
                "- s.rows[b/member]",
                "~ s.latest: \"1.0\" -> \"1.1\"",
                "~ s.rows[a/dependency].decision: \"crates-io\" -> \"rejected\"",
            ]
        );
    }
}
