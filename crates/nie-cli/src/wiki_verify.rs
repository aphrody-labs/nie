//! `nie wiki verify`: compares what a wiki serves, record by record and field by field, to the
//! raw decode of the VFS entries it names, and every page's stated provenance to the entry it
//! points at. The raw side reads the files with the `nie-formats` cfg.bin parsers directly, not
//! through the wiki's own table layer.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context as _, bail};
use clap::Args;
use futures_util::StreamExt as _;
use nie_formats::cfgbin::{self, CfgEntry, RdbnValue, Value as Cfg};
use nie_formats::vfs::Vfs;
use nie_wiki_vfs::Wiki;
use serde_json::{Map, Value, json};

use crate::serve_cmd::{parse_query, percent_decode};
use crate::serve_routes::match_route;
use crate::serve_wiki;

const PAGE_LIMIT: usize = 500;

#[derive(Debug, Args)]
pub struct VerifyArgs {
    /// Base URL of a running `nie serve` (`https://nie.aphrody.app`); without it the wiki is
    /// built in process over the VFS and the same route handlers answer.
    #[arg(long)]
    pub server: Option<String>,
    /// Requests in flight.
    #[arg(long, default_value_t = 16)]
    pub concurrency: usize,
    /// Skip the HTML pages (JSON routes only).
    #[arg(long)]
    pub no_pages: bool,
    /// Mismatches printed per check.
    #[arg(long, default_value_t = 10)]
    pub show: usize,
}

pub fn run(args: &VerifyArgs, game_dir: Option<&Path>) -> anyhow::Result<()> {
    let source = game_dir.map(|p| p.to_string_lossy().into_owned());
    let vfs =
        Arc::new(nie_formats::vfs::open_source(source.as_deref()).context("mounting the game")?);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let report = runtime.block_on(verify(args, vfs))?;
    report.print(args.show);
    if report.total_mismatches() > 0 {
        bail!("{} mismatch(es)", report.total_mismatches());
    }
    Ok(())
}

// ----- report -----

#[derive(Default)]
struct Check {
    checked: usize,
    mismatches: Vec<String>,
}

#[derive(Default)]
pub(crate) struct Report {
    checks: BTreeMap<String, Check>,
    /// Facts about the installation's data that the wiki states as empty states; they are
    /// reported, never counted as mismatches.
    notes: BTreeMap<String, Vec<String>>,
}

impl Report {
    /// How many comparisons ran under `check`.
    #[cfg(test)]
    pub(crate) fn checked(&self, check: &str) -> usize {
        self.checks.get(check).map_or(0, |c| c.checked)
    }

    fn note(&mut self, check: &str, message: String) {
        self.checks.entry(check.to_owned()).or_default().checked += 1;
        self.notes
            .entry(check.to_owned())
            .or_default()
            .push(message);
    }

    fn pass(&mut self, check: &str) {
        self.checks.entry(check.to_owned()).or_default().checked += 1;
    }

    fn fail(&mut self, check: &str, message: String) {
        let entry = self.checks.entry(check.to_owned()).or_default();
        entry.checked += 1;
        entry.mismatches.push(message);
    }

    pub(crate) fn total_mismatches(&self) -> usize {
        self.checks.values().map(|c| c.mismatches.len()).sum()
    }

    pub(crate) fn print(&self, show: usize) {
        for (name, check) in &self.checks {
            println!(
                "{:<44} checked {:>7}  mismatches {:>6}",
                name,
                check.checked,
                check.mismatches.len()
            );
            for message in check.mismatches.iter().take(show) {
                println!("    {message}");
            }
            if check.mismatches.len() > show {
                println!("    ... {} more", check.mismatches.len() - show);
            }
        }
        for (name, notes) in &self.notes {
            println!(
                "note {name}: {} entr(ies) the wiki shows as empty states",
                notes.len()
            );
            for message in notes.iter().take(show) {
                println!("    {message}");
            }
        }
        println!(
            "total: {} check(s), {} mismatch(es)",
            self.checks.values().map(|c| c.checked).sum::<usize>(),
            self.total_mismatches()
        );
    }

    /// Records the differences of `served` from `expected` under `check`.
    fn compare(&mut self, check: &str, context: &str, served: &Value, expected: &Value) {
        let mut diffs = Vec::new();
        diff("", served, expected, &mut diffs);
        if diffs.is_empty() {
            self.pass(check);
        } else {
            let message = format!("{context}: {}", diffs.join("; "));
            self.fail(check, message);
        }
    }
}

/// Deep comparison; numbers that are not both integers compare as `f32`, the stored width.
fn diff(path: &str, served: &Value, expected: &Value, out: &mut Vec<String>) {
    let at = if path.is_empty() { "." } else { path };
    match (served, expected) {
        (Value::Object(a), Value::Object(b)) => {
            for (key, value) in b {
                match a.get(key) {
                    Some(have) => diff(&format!("{path}.{key}"), have, value, out),
                    None => out.push(format!("missing {path}.{key}")),
                }
            }
            for key in a.keys().filter(|k| !b.contains_key(*k)) {
                out.push(format!("extra {path}.{key}"));
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                out.push(format!("{at}: {} elements, expected {}", a.len(), b.len()));
            }
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                diff(&format!("{path}[{i}]"), x, y, out);
            }
        }
        (Value::Number(a), Value::Number(b)) => {
            let equal = match (a.as_i64(), b.as_i64()) {
                (Some(x), Some(y)) => x == y,
                _ => matches!((a.as_f64(), b.as_f64()), (Some(x), Some(y)) if x as f32 == y as f32),
            };
            if !equal {
                out.push(format!("{at}: {a}, expected {b}"));
            }
        }
        (a, b) if a == b => {}
        (a, b) => out.push(format!("{at}: {}, expected {}", short(a), short(b))),
    }
}

fn short(value: &Value) -> String {
    let text = value.to_string();
    if text.len() > 80 {
        format!(
            "{}...",
            &text[..text.char_indices().nth(77).map_or(text.len(), |(i, _)| i)]
        )
    } else {
        text
    }
}

// ----- raw layer -----

#[derive(Clone, Debug)]
enum Raw {
    Null,
    Bool(bool),
    Int(i64),
    Float(f32),
    Str(String),
    Arr(Vec<Raw>),
}

impl Raw {
    fn json(&self) -> Value {
        match self {
            Raw::Null => Value::Null,
            Raw::Bool(b) => Value::Bool(*b),
            Raw::Int(i) => Value::from(*i),
            Raw::Float(f) => Value::from(f64::from(*f)),
            Raw::Str(s) => Value::from(s.as_str()),
            Raw::Arr(items) => Value::Array(items.iter().map(Raw::json).collect()),
        }
    }

    fn int(&self) -> Option<i64> {
        match self {
            Raw::Int(i) => Some(*i),
            _ => None,
        }
    }
}

fn raw_cfg(value: &Cfg) -> Raw {
    match value {
        Cfg::Int(i) => Raw::Int(i64::from(*i)),
        Cfg::Float(f) => Raw::Float(*f),
        Cfg::String(s) => Raw::Str(s.clone()),
    }
}

fn raw_rdbn(value: &RdbnValue) -> Raw {
    let floats = |f: &[f32; 4]| Raw::Arr(f.iter().map(|x| Raw::Float(*x)).collect());
    match value {
        RdbnValue::Bool(b) => Raw::Bool(*b),
        RdbnValue::Byte(b) => Raw::Int(i64::from(*b)),
        RdbnValue::Short(s) | RdbnValue::ActType(s) => Raw::Int(i64::from(*s)),
        RdbnValue::Int(i) | RdbnValue::Flag(i) => Raw::Int(i64::from(*i)),
        RdbnValue::Float(f) => Raw::Float(*f),
        RdbnValue::Hash(h) => Raw::Str(format!("0x{h:08X}")),
        RdbnValue::Rates(f) | RdbnValue::Position(f) => floats(f),
        RdbnValue::Condition(s) => Raw::Str(s.clone()),
        RdbnValue::ShortTuple(t) => Raw::Arr(t.iter().map(|x| Raw::Int(i64::from(*x))).collect()),
        RdbnValue::Blob(_) | RdbnValue::Invalid => Raw::Null,
    }
}

#[derive(Clone, Debug)]
struct RawNode {
    name: String,
    vars: Vec<Raw>,
    children: Vec<RawNode>,
}

impl RawNode {
    fn from(entry: &CfgEntry) -> Self {
        Self {
            name: entry.name.clone(),
            vars: entry.variables.iter().map(raw_cfg).collect(),
            children: entry.children.iter().map(Self::from).collect(),
        }
    }

    fn json(&self) -> Value {
        json!({
            "name": self.name,
            "variables": self.vars.iter().map(Raw::json).collect::<Vec<_>>(),
            "children": self.children.iter().map(Self::json).collect::<Vec<_>>(),
        })
    }

    fn id(&self, i: usize) -> Option<u32> {
        self.vars.get(i)?.int().map(|v| v as u32)
    }

    fn nonzero(&self, i: usize) -> Option<u32> {
        self.id(i).filter(|v| *v != 0)
    }

    fn string(&self, i: usize) -> Option<&str> {
        match self.vars.get(i)? {
            Raw::Str(s) => Some(s),
            _ => None,
        }
    }
}

#[derive(Debug)]
struct RawList {
    name: String,
    rows: Vec<Vec<(String, Raw)>>,
}

#[derive(Debug)]
enum RawBody {
    T2b(Vec<RawNode>),
    Rdbn(Vec<RawList>),
}

#[derive(Debug)]
struct RawTable {
    path: String,
    body: RawBody,
    classes: std::sync::OnceLock<Vec<OwnedClass>>,
}

/// A T2B class as computed from the container structure.
struct Computed<'a> {
    name: String,
    nodes: Vec<&'a RawNode>,
    container: Vec<Raw>,
    role: Role,
}

#[derive(Debug)]
struct OwnedClass {
    name: String,
    nodes: Vec<RawNode>,
    container: Vec<Raw>,
    role: Role,
}

/// A T2B class: its nodes in file order, with the role the container structure gives it.
struct T2bClass<'a> {
    name: String,
    nodes: &'a [RawNode],
    container: Vec<Raw>,
    role: Role,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Role {
    Records,
    SortIndex(String),
    Reference(String, String),
}

const CONTAINER_SUFFIXES: [&str; 3] = ["_LIST_BEG", "_BEGIN", "_BEG"];

fn container_main(name: &str) -> Option<&str> {
    CONTAINER_SUFFIXES
        .iter()
        .find_map(|suffix| name.strip_suffix(suffix))
}

fn ref_target(main: &str, tail: &str, classes: &[String]) -> Option<String> {
    let mut cuts: Vec<usize> = main.match_indices('_').map(|(i, _)| i).collect();
    cuts.reverse();
    cuts.into_iter()
        .map(|cut| format!("{}_{tail}", &main[..cut]))
        .chain(std::iter::once(format!("{main}_{tail}")))
        .find(|wanted| classes.contains(wanted))
}

impl RawTable {
    fn compute_classes(&self) -> Vec<Computed<'_>> {
        let RawBody::T2b(roots) = &self.body else {
            return Vec::new();
        };
        let mut order: Vec<String> = Vec::new();
        let mut nodes: HashMap<String, Vec<&RawNode>> = HashMap::new();
        let mut container: HashMap<String, Vec<Raw>> = HashMap::new();
        let mut roles: HashMap<String, Role> = HashMap::new();
        for root in roots {
            let main = container_main(&root.name);
            let members: Vec<&RawNode> = if main.is_some() {
                root.children.iter().collect()
            } else {
                vec![root]
            };
            if let Some(main) = main {
                container.insert(main.to_owned(), root.vars.clone());
            }
            for member in members {
                if !nodes.contains_key(&member.name) {
                    order.push(member.name.clone());
                }
                let role = match main {
                    Some(main) if member.name == "__SORT_INDEX" => Role::SortIndex(main.to_owned()),
                    Some(main) if member.name.starts_with(&format!("{main}_REF_")) => {
                        Role::Reference(main.to_owned(), member.name[main.len() + 5..].to_owned())
                    }
                    _ => Role::Records,
                };
                roles.entry(member.name.clone()).or_insert(role);
                nodes.entry(member.name.clone()).or_default().push(member);
            }
        }
        let names = order.clone();
        let mut final_roles: HashMap<String, Role> = HashMap::new();
        for name in &order {
            let role = roles[name].clone();
            let count = nodes[name].len();
            let resolved = match &role {
                Role::Records => Role::Records,
                Role::SortIndex(main) => {
                    if nodes.get(main).map_or(0, Vec::len) == count && count > 0 {
                        role
                    } else {
                        Role::Records
                    }
                }
                Role::Reference(main, tail) => {
                    let main_count = nodes.get(main).map_or(0, Vec::len);
                    match ref_target(main, tail, &names) {
                        Some(target) if main_count == count && count > 0 => {
                            Role::Reference(main.clone(), target)
                        }
                        _ => Role::Records,
                    }
                }
            };
            final_roles.insert(name.clone(), resolved);
        }
        order
            .into_iter()
            .map(|name| Computed {
                nodes: nodes.remove(&name).unwrap_or_default(),
                container: container.remove(&name).unwrap_or_default(),
                role: final_roles.remove(&name).unwrap_or(Role::Records),
                name,
            })
            .collect()
    }

    fn rdbn_list(&self, name: &str) -> Option<&RawList> {
        match &self.body {
            RawBody::Rdbn(lists) => lists.iter().find(|l| l.name == name),
            RawBody::T2b(_) => None,
        }
    }

    fn cached(&self) -> &[OwnedClass] {
        self.classes.get_or_init(|| {
            self.compute_classes()
                .into_iter()
                .map(|c| OwnedClass {
                    name: c.name,
                    nodes: c.nodes.into_iter().cloned().collect(),
                    container: c.container,
                    role: c.role,
                })
                .collect()
        })
    }

    fn t2b_classes(&self) -> Vec<T2bClass<'_>> {
        self.cached()
            .iter()
            .map(|o| T2bClass {
                name: o.name.clone(),
                nodes: &o.nodes,
                container: o.container.clone(),
                role: o.role.clone(),
            })
            .collect()
    }

    /// Every node of class `name` in file order.
    fn nodes(&self, name: &str) -> &[RawNode] {
        self.cached()
            .iter()
            .find(|c| c.name == name)
            .map_or(&[], |c| &c.nodes)
    }
}

fn row_map(row: &[(String, Raw)]) -> BTreeMap<String, &Raw> {
    row.iter().map(|(k, v)| (k.clone(), v)).collect()
}

fn row_json(row: &[(String, Raw)]) -> Value {
    Value::Object(
        row.iter()
            .map(|(k, v)| (k.clone(), v.json()))
            .collect::<Map<_, _>>(),
    )
}

fn row_id(row: &[(String, Raw)], key: &str) -> Option<u32> {
    match &row_map(row).get(key)? {
        Raw::Str(s) => nie_data::hash::HashId::parse(s).map(|h| h.0),
        Raw::Int(i) => Some(*i as u32),
        _ => None,
    }
    .filter(|id| *id != 0)
}

fn row_int(row: &[(String, Raw)], key: &str) -> Option<i64> {
    row_map(row).get(key)?.int()
}

// ----- registry (an independent statement of what the wiki serves) -----

const REGISTRY: &[&str] = &[
    "chara_param",
    "chara_base",
    "chara_series_config",
    "seasonal_chara_param",
    "chara_param_table_config",
    "skill_config",
    "passive_skill_config",
    "aura_skill_config",
    "change_aura_skill_config",
    "override_skill_config",
    "real_skill_config",
    "belong_team_config",
];

#[derive(Clone, Copy, PartialEq)]
enum Sec {
    Noun,
    Text,
    Any,
}

/// (table, class, selector, text file, section, label): `Ok(var)` or `Err(field)`.
type Ref = (
    &'static str,
    &'static str,
    Result<usize, &'static str>,
    &'static str,
    Sec,
    &'static str,
);

const REFS: &[Ref] = &[
    (
        "chara_base",
        "CHARA_BASE_INFO",
        Ok(3),
        "chara_text",
        Sec::Noun,
        "name",
    ),
    (
        "chara_base",
        "CHARA_BASE_INFO",
        Ok(4),
        "chara_text",
        Sec::Noun,
        "family_name",
    ),
    (
        "chara_base",
        "CHARA_BASE_INFO",
        Ok(5),
        "chara_text",
        Sec::Noun,
        "family_name_variant",
    ),
    (
        "chara_base",
        "CHARA_BASE_INFO",
        Ok(19),
        "chara_description_text",
        Sec::Text,
        "description",
    ),
    (
        "chara_series_config",
        "m_charaSeriesInfoList",
        Err("charaSeriesNameTextId"),
        "chara_add_info_text",
        Sec::Noun,
        "name",
    ),
    (
        "skill_config",
        "m_skillInfoList",
        Err("skillNameId"),
        "skill_text",
        Sec::Noun,
        "name",
    ),
    (
        "skill_config",
        "m_skillInfoList",
        Err("skillDescId"),
        "skill_text",
        Sec::Text,
        "description",
    ),
    (
        "passive_skill_config",
        "PASSIVE_SKILL_INFO",
        Ok(1),
        "skill_text",
        Sec::Noun,
        "name",
    ),
    (
        "aura_skill_config",
        "AURA_CMD_INFO",
        Ok(2),
        "skill_text",
        Sec::Noun,
        "name",
    ),
    (
        "aura_skill_config",
        "AURA_CMD_INFO",
        Ok(3),
        "skill_text",
        Sec::Text,
        "description",
    ),
    (
        "belong_team_config",
        "m_belongTeamInfoList",
        Err("teamNameTextId"),
        "team_text",
        Sec::Any,
        "name",
    ),
];

const RICH: &[(&str, &str)] = &[
    ("chara_param", "CHARA_PARAM_INFO"),
    ("chara_param", "__SORT_INDEX"),
    ("chara_base", "CHARA_BASE_INFO"),
    ("chara_base", "CHARA_BASE_INFO_REF_BATTLE"),
    ("skill_config", "m_skillInfoList"),
    ("belong_team_config", "m_belongTeamInfoList"),
];

const RICH_FAMILY: &[(&str, &str, &str)] = &[
    ("chara_param", "CHARA_PARAM_INFO", "characters"),
    ("chara_param", "__SORT_INDEX", "characters"),
    ("chara_base", "CHARA_BASE_INFO", "characters"),
    ("chara_base", "CHARA_BASE_INFO_REF_BATTLE", "characters"),
    ("skill_config", "m_skillInfoList", "skills"),
    ("belong_team_config", "m_belongTeamInfoList", "teams"),
];

const ELEMENT_KEYS: [(i64, &str); 6] = [
    (0, "none"),
    (1, "wind"),
    (2, "forest"),
    (3, "fire"),
    (4, "mountain"),
    (5, "void"),
];

fn tag_of(folder: &str) -> String {
    folder
        .split('_')
        .enumerate()
        .map(|(i, part)| match (i, part.len()) {
            (0, _) => part.to_owned(),
            (_, 4) => part[..1].to_uppercase() + &part[1..],
            _ => part.to_uppercase(),
        })
        .collect::<Vec<_>>()
        .join("-")
}

fn hex(id: u32) -> String {
    format!("0x{id:08X}")
}

// ----- the installation, decoded raw -----

struct TextFile {
    path: String,
    nouns: HashMap<u32, Vec<(i64, String, usize)>>,
    texts: HashMap<u32, Vec<(i64, String, usize)>>,
}

struct Install {
    languages: Vec<String>,
    tables: HashMap<String, RawTable>,
    texts: HashMap<(String, String), Option<TextFile>>,
    listing: Vec<String>,
}

fn find_table(listing: &[String], stem: &str) -> Option<String> {
    listing
        .iter()
        .filter(|p| p.starts_with("data/common/gamedata/"))
        .filter_map(|path| {
            let file = path.rsplit('/').next()?;
            let version = if file.strip_suffix(".cfg.bin") == Some(stem) {
                Vec::new()
            } else {
                file.strip_prefix(stem)?
                    .strip_prefix('_')?
                    .strip_suffix(".cfg.bin")?
                    .split('.')
                    .map(|part| part.parse::<u64>().ok())
                    .collect::<Option<Vec<_>>>()?
            };
            Some((version, path))
        })
        .max()
        .map(|(_, path)| path.clone())
}

fn load_raw(vfs: &Vfs, path: &str) -> anyhow::Result<RawTable> {
    let bytes = vfs.read(path).with_context(|| format!("reading {path}"))?;
    let body = if cfgbin::is_rdbn(&bytes) {
        let parsed = cfgbin::parse(&bytes).with_context(|| format!("parsing {path}"))?;
        RawBody::Rdbn(
            cfgbin::read_values(&parsed, &bytes)
                .into_iter()
                .map(|list| RawList {
                    name: list.name,
                    rows: list
                        .rows
                        .into_iter()
                        .map(|row| {
                            row.fields
                                .iter()
                                .map(|(k, v)| (k.clone(), raw_rdbn(v)))
                                .collect()
                        })
                        .collect(),
                })
                .collect(),
        )
    } else {
        let parsed = cfgbin::parse_t2b(&bytes).with_context(|| format!("parsing {path}"))?;
        RawBody::T2b(parsed.entries.iter().map(RawNode::from).collect())
    };
    Ok(RawTable {
        path: path.to_owned(),
        body,
        classes: std::sync::OnceLock::new(),
    })
}

fn walk<'a>(nodes: &'a [RawNode], out: &mut Vec<&'a RawNode>) {
    for node in nodes {
        out.push(node);
        walk(&node.children, out);
    }
}

fn load_text(vfs: &Vfs, lang: &str, stem: &str) -> anyhow::Result<Option<TextFile>> {
    let path = format!("data/common/text/{lang}/{stem}.cfg.bin");
    if vfs.find_any(&path).is_none() {
        return Ok(None);
    }
    let bytes = vfs.read(&path)?;
    let parsed = cfgbin::parse_t2b(&bytes)?;
    let roots: Vec<RawNode> = parsed.entries.iter().map(RawNode::from).collect();
    let mut all = Vec::new();
    walk(&roots, &mut all);
    let mut file = TextFile {
        path,
        nouns: HashMap::new(),
        texts: HashMap::new(),
    };
    let (mut noun_index, mut text_index) = (0, 0);
    for node in all {
        let (rows, counter, var) = match node.name.as_str() {
            "NOUN_INFO" => (&mut file.nouns, &mut noun_index, 5),
            "TEXT_INFO" => (&mut file.texts, &mut text_index, 2),
            _ => continue,
        };
        let index = *counter;
        *counter += 1;
        let (Some(key), Some(text)) = (node.id(0), node.string(var)) else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        let form = node.vars.get(1).and_then(Raw::int).unwrap_or(0);
        rows.entry(key)
            .or_default()
            .push((form, text.to_owned(), index));
    }
    Ok(Some(file))
}

impl Install {
    fn open(vfs: &Vfs) -> anyhow::Result<Self> {
        let listing: Vec<String> = vfs.iter_all().map(|e| e.path.to_owned()).collect();
        let mut folders: Vec<String> = listing
            .iter()
            .filter_map(|p| {
                let rest = p.strip_prefix("data/common/text/")?;
                let (folder, file) = rest.split_once('/')?;
                (file == "chara_text.cfg.bin").then(|| folder.to_owned())
            })
            .collect();
        folders.sort();
        folders.dedup();
        let mut tables = HashMap::new();
        for stem in REGISTRY {
            let path = find_table(&listing, stem)
                .with_context(|| format!("no gamedata file for table {stem}"))?;
            tables.insert((*stem).to_owned(), load_raw(vfs, &path)?);
        }
        let mut texts = HashMap::new();
        for lang in &folders {
            for stem in [
                "chara_text",
                "chara_text_roma",
                "chara_description_text",
                "system_text",
                "skill_text",
                "team_text",
                "chara_add_info_text",
            ] {
                texts.insert(
                    (lang.clone(), stem.to_owned()),
                    load_text(vfs, lang, stem)?,
                );
            }
        }
        Ok(Self {
            languages: folders,
            tables,
            texts,
            listing,
        })
    }

    fn table(&self, stem: &str) -> &RawTable {
        &self.tables[stem]
    }

    /// The expected `Localized` JSON of `key` in `stem`, `None` when no language holds it.
    fn localized(&self, stem: &str, section: Sec, key: u32) -> Option<Value> {
        let mut texts = Map::new();
        for lang in &self.languages {
            let Some(Some(file)) = self.texts.get(&(lang.clone(), stem.to_owned())) else {
                continue;
            };
            let found = match section {
                Sec::Noun => file.nouns.get(&key).map(|r| ("NOUN_INFO", r)),
                Sec::Text => file.texts.get(&key).map(|r| ("TEXT_INFO", r)),
                Sec::Any => file
                    .nouns
                    .get(&key)
                    .map(|r| ("NOUN_INFO", r))
                    .or_else(|| file.texts.get(&key).map(|r| ("TEXT_INFO", r))),
            };
            let Some((kind, rows)) = found else { continue };
            let forms: Vec<Value> = rows
                .iter()
                .map(|(form, raw, index)| {
                    let text = nie_data::text::sanitize_text(raw);
                    let mut f = Map::new();
                    f.insert("form".into(), Value::from(*form));
                    if text != *raw {
                        f.insert("raw".into(), Value::from(raw.as_str()));
                    }
                    f.insert("text".into(), Value::from(text));
                    f.insert("index".into(), Value::from(*index));
                    Value::Object(f)
                })
                .collect();
            texts.insert(
                lang.clone(),
                json!({ "vfs_path": file.path, "section": kind, "forms": forms }),
            );
        }
        (!texts.is_empty()).then(|| json!({ "id": hex(key), "table": stem, "texts": texts }))
    }

    fn element(&self, value: i64) -> Value {
        let label = ELEMENT_KEYS
            .iter()
            .find(|(code, _)| *code == value)
            .and_then(|(_, key)| {
                self.localized(
                    "system_text",
                    Sec::Noun,
                    crc32fast::hash(format!("element_type_{key}").as_bytes()),
                )
            });
        match label {
            Some(label) => json!({ "value": value, "label": label }),
            None => json!({ "value": value }),
        }
    }
}

fn provenance(path: &str, table: &str, index: usize) -> Value {
    json!({ "vfs_path": path, "table": table, "index": index })
}

fn put(map: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(value) = value {
        map.insert(key.to_owned(), value);
    }
}

// ----- expected records -----

/// What a `skill_config`, `aura_skill_config` or `passive_skill_config` id refers to.
struct SkillRefs {
    /// id -> (family, provenance, name key)
    ids: HashMap<u32, (&'static str, Value, Option<u32>)>,
}

impl Install {
    fn skill_refs(&self) -> SkillRefs {
        let mut ids: HashMap<u32, (&'static str, Value, Option<u32>)> = HashMap::new();
        let skill = self.table("skill_config");
        if let Some(list) = skill.rdbn_list("m_skillInfoList") {
            for (index, row) in list.rows.iter().enumerate() {
                if let Some(id) = row_id(row, "skillID") {
                    ids.entry(id).or_insert_with(|| {
                        (
                            "skill",
                            provenance(&skill.path, "m_skillInfoList", index),
                            row_id(row, "skillNameId"),
                        )
                    });
                }
            }
        }
        for (stem, class, family, name_var) in [
            ("aura_skill_config", "AURA_CMD_INFO", "aura", 2),
            ("passive_skill_config", "PASSIVE_SKILL_INFO", "passive", 1),
        ] {
            let table = self.table(stem);
            for (index, node) in table.nodes(class).iter().enumerate() {
                if let Some(id) = node.nonzero(0) {
                    ids.entry(id).or_insert_with(|| {
                        (
                            family,
                            provenance(&table.path, class, index),
                            node.nonzero(name_var),
                        )
                    });
                }
            }
        }
        SkillRefs { ids }
    }

    fn series(&self) -> HashMap<u32, (Option<i64>, Option<u32>)> {
        let table = self.table("chara_series_config");
        table
            .rdbn_list("m_charaSeriesInfoList")
            .into_iter()
            .flat_map(|l| &l.rows)
            .filter_map(|row| {
                Some((
                    row_id(row, "charaSeriesId")?,
                    (
                        row_int(row, "charaSeriesType"),
                        row_id(row, "charaSeriesNameTextId"),
                    ),
                ))
            })
            .collect()
    }

    fn teams_names(&self) -> HashMap<u32, Option<u32>> {
        self.table("belong_team_config")
            .rdbn_list("m_belongTeamInfoList")
            .into_iter()
            .flat_map(|l| &l.rows)
            .filter_map(|row| Some((row_id(row, "belongTeamId")?, row_id(row, "teamNameTextId"))))
            .collect()
    }
}

/// The character entries the tables yield, in served order: (param index, base index) pairs.
struct CharaEntries {
    entries: Vec<(Option<usize>, Option<usize>, &'static str)>,
}

fn chara_entries(install: &Install) -> CharaEntries {
    let params = install.table("chara_param").nodes("CHARA_PARAM_INFO");
    let bases = install.table("chara_base").nodes("CHARA_BASE_INFO");
    let mut first: HashMap<u32, usize> = HashMap::new();
    for (i, base) in bases.iter().enumerate() {
        if let Some(id) = base.id(0) {
            first.entry(id).or_insert(i);
        }
    }
    let mut used = HashSet::new();
    let mut entries = Vec::new();
    for (i, param) in params.iter().enumerate() {
        let base = param.id(1).and_then(|id| first.get(&id).copied());
        used.extend(base);
        entries.push((
            Some(i),
            base,
            if base.is_some() {
                "param_and_base"
            } else {
                "param_without_base"
            },
        ));
    }
    for i in 0..bases.len() {
        if !used.contains(&i) {
            entries.push((None, Some(i), "base_without_param"));
        }
    }
    CharaEntries { entries }
}

fn coded(value: i64) -> Value {
    json!({ "value": value })
}

impl Install {
    fn character(
        &self,
        entry: (Option<usize>, Option<usize>, &'static str),
        skills: &SkillRefs,
        series: &HashMap<u32, (Option<i64>, Option<u32>)>,
        teams: &HashMap<u32, Option<u32>>,
    ) -> Value {
        let param_table = self.table("chara_param");
        let base_table = self.table("chara_base");
        let params = param_table.nodes("CHARA_PARAM_INFO");
        let bases = base_table.nodes("CHARA_BASE_INFO");
        let sorts = param_table.nodes("__SORT_INDEX");
        let battle_refs = base_table.nodes("CHARA_BASE_INFO_REF_BATTLE");
        let battle = base_table.nodes("CHARA_BASE_BATTLE");
        let (p, b, class) = entry;
        let param = p.map(|i| &params[i]);
        let base = b.map(|i| &bases[i]);
        let mut o = Map::new();
        o.insert("class".into(), class.into());
        put(
            &mut o,
            "chara_param_id",
            param.and_then(|n| n.id(0)).map(|v| Value::from(hex(v))),
        );
        let base_id = match param {
            Some(n) => n.id(1),
            None => base.and_then(|n| n.id(0)),
        };
        put(
            &mut o,
            "chara_base_id",
            base_id.map(|v| Value::from(hex(v))),
        );
        put(
            &mut o,
            "internal_code",
            base.and_then(|n| n.string(1)).map(Value::from),
        );
        let nz = |i: usize| base.and_then(|n| n.nonzero(i));
        let loc =
            |stem: &str, sec: Sec, key: Option<u32>| key.and_then(|k| self.localized(stem, sec, k));
        put(&mut o, "name", loc("chara_text", Sec::Noun, nz(3)));
        put(&mut o, "family_name", loc("chara_text", Sec::Noun, nz(4)));
        put(
            &mut o,
            "family_name_variant",
            loc("chara_text", Sec::Noun, nz(5)),
        );
        put(
            &mut o,
            "name_roma",
            loc("chara_text_roma", Sec::Noun, nz(3)),
        );
        put(
            &mut o,
            "family_name_roma",
            loc("chara_text_roma", Sec::Noun, nz(4)),
        );
        put(
            &mut o,
            "family_name_variant_roma",
            loc("chara_text_roma", Sec::Noun, nz(5)),
        );
        put(
            &mut o,
            "description",
            loc("chara_description_text", Sec::Text, nz(19)),
        );
        put(
            &mut o,
            "gender",
            base.and_then(|n| n.vars.get(11))
                .and_then(Raw::int)
                .map(coded),
        );
        put(
            &mut o,
            "element",
            param
                .and_then(|n| n.vars.get(2))
                .and_then(Raw::int)
                .map(|v| self.element(v)),
        );
        put(
            &mut o,
            "main_position",
            param
                .and_then(|n| n.vars.get(3))
                .and_then(Raw::int)
                .map(coded),
        );
        put(
            &mut o,
            "sub_position",
            param
                .and_then(|n| n.vars.get(4))
                .and_then(Raw::int)
                .map(coded),
        );
        put(
            &mut o,
            "growth_pattern",
            param
                .and_then(|n| n.vars.get(8))
                .and_then(Raw::int)
                .map(Value::from),
        );
        put(
            &mut o,
            "series",
            nz(15).map(|id| {
                let row = series.get(&id);
                let mut s = Map::new();
                s.insert("series_id".into(), hex(id).into());
                put(
                    &mut s,
                    "series_type",
                    row.and_then(|r| r.0).map(Value::from),
                );
                put(
                    &mut s,
                    "name",
                    loc("chara_add_info_text", Sec::Noun, row.and_then(|r| r.1)),
                );
                Value::Object(s)
            }),
        );
        put(
            &mut o,
            "team",
            nz(16).map(|id| {
                let mut t = Map::new();
                t.insert("belong_team_id".into(), hex(id).into());
                put(
                    &mut t,
                    "name",
                    loc("team_text", Sec::Any, teams.get(&id).copied().flatten()),
                );
                Value::Object(t)
            }),
        );
        let mut learned = Vec::new();
        if let Some(param) = param {
            for slot in 0..9 {
                let level_index = 10 + slot * 2;
                let hash_index = level_index + 1;
                if hash_index >= param.vars.len() {
                    break;
                }
                let (Some(id), Some(level)) = (
                    param.nonzero(hash_index),
                    param.vars.get(level_index).and_then(Raw::int),
                ) else {
                    continue;
                };
                if !(0..=99).contains(&level) {
                    continue;
                }
                let mut s = Map::new();
                s.insert("skill_id".into(), hex(id).into());
                s.insert("learn_level".into(), level.into());
                if let Some((family, entry, name_key)) = skills.ids.get(&id) {
                    s.insert("family".into(), (*family).into());
                    s.insert("entry".into(), entry.clone());
                    put(
                        &mut s,
                        "name_text_id",
                        name_key.map(|k| Value::from(hex(k))),
                    );
                    put(&mut s, "name", loc("skill_text", Sec::Noun, *name_key));
                }
                learned.push(Value::Object(s));
            }
        }
        o.insert("skills".into(), Value::Array(learned));
        put(
            &mut o,
            "sort_index",
            p.and_then(|i| sorts.get(i))
                .and_then(|n| n.vars.first())
                .and_then(Raw::int)
                .map(Value::from),
        );
        put(
            &mut o,
            "battle",
            b.and_then(|i| battle_refs.get(i)).and_then(|n| {
                let (offset, count) = (n.vars.first()?.int()?, n.vars.get(1)?.int()?);
                Some(slice_json("CHARA_BASE_BATTLE", battle, offset, count))
            }),
        );
        o.insert(
            "param_raw".into(),
            Value::Array(param.map_or_else(Vec::new, |n| n.vars.iter().map(Raw::json).collect())),
        );
        o.insert(
            "base_raw".into(),
            Value::Array(base.map_or_else(Vec::new, |n| n.vars.iter().map(Raw::json).collect())),
        );
        put(
            &mut o,
            "param_source",
            p.map(|i| provenance(&param_table.path, "CHARA_PARAM_INFO", i)),
        );
        put(
            &mut o,
            "base_source",
            b.map(|i| provenance(&base_table.path, "CHARA_BASE_INFO", i)),
        );
        Value::Object(o)
    }
}

fn slice_json(class: &str, nodes: &[RawNode], offset: i64, count: i64) -> Value {
    let items: Vec<Value> = (|| {
        let start = usize::try_from(offset).ok()?;
        let end = start.checked_add(usize::try_from(count).ok()?)?;
        (end <= nodes.len()).then(|| {
            (start..end)
                .map(|index| {
                    json!({
                        "index": index,
                        "variables": nodes[index].vars.iter().map(Raw::json).collect::<Vec<_>>(),
                    })
                })
                .collect()
        })
    })()
    .unwrap_or_default();
    json!({ "class": class, "offset": offset, "count": count, "items": items })
}

impl Install {
    fn skill(&self, index: usize, row: &[(String, Raw)]) -> Option<Value> {
        let table = self.table("skill_config");
        let id = row_id(row, "skillID")?;
        let code = match row_map(row).get("skillIDStr")? {
            Raw::Str(s) => s.clone(),
            _ => return None,
        };
        let mut o = Map::new();
        o.insert("skill_id".into(), hex(id).into());
        o.insert("skill_id_str".into(), code.into());
        put(
            &mut o,
            "name",
            row_id(row, "skillNameId").and_then(|k| self.localized("skill_text", Sec::Noun, k)),
        );
        put(
            &mut o,
            "description",
            row_id(row, "skillDescId").and_then(|k| self.localized("skill_text", Sec::Text, k)),
        );
        put(
            &mut o,
            "element",
            row_int(row, "element").map(|v| self.element(v)),
        );
        put(&mut o, "category", row_int(row, "category").map(coded));
        o.insert("fields".into(), row_json(row));
        o.insert(
            "source".into(),
            provenance(&table.path, "m_skillInfoList", index),
        );
        Some(Value::Object(o))
    }

    fn emblem_files(&self) -> HashMap<u32, Vec<String>> {
        let wanted: HashSet<u32> = self
            .table("belong_team_config")
            .rdbn_list("m_belongTeamInfoList")
            .into_iter()
            .flat_map(|l| &l.rows)
            .flat_map(|row| {
                row.iter()
                    .filter(|(k, _)| k.starts_with("teamEmblemId"))
                    .filter_map(|(k, _)| row_id(row, k))
            })
            .collect();
        let mut found: HashMap<u32, Vec<String>> = HashMap::new();
        for path in &self.listing {
            let file = path.rsplit('/').next().unwrap_or(path);
            let stem = file.split('.').next().unwrap_or(file);
            let h = crc32fast::hash(stem.as_bytes());
            if wanted.contains(&h) {
                found.entry(h).or_default().push(path.clone());
            }
        }
        for paths in found.values_mut() {
            paths.sort();
        }
        found
    }

    fn team(
        &self,
        index: usize,
        row: &[(String, Raw)],
        files: &HashMap<u32, Vec<String>>,
    ) -> Option<Value> {
        let table = self.table("belong_team_config");
        let id = row_id(row, "belongTeamId")?;
        let mut o = Map::new();
        o.insert("belong_team_id".into(), hex(id).into());
        put(
            &mut o,
            "name",
            row_id(row, "teamNameTextId").and_then(|k| self.localized("team_text", Sec::Any, k)),
        );
        let mut emblems = Map::new();
        for (key, _) in row.iter().filter(|(k, _)| k.starts_with("teamEmblemId")) {
            if let Some(eid) = row_id(row, key) {
                emblems.insert(
                    key.clone(),
                    json!({ "emblem_id": hex(eid), "files": files.get(&eid).cloned().unwrap_or_default() }),
                );
            }
        }
        o.insert("emblems".into(), Value::Object(emblems));
        let bases_table = self.table("chara_base");
        let mut affiliated = Vec::new();
        for (i, base) in bases_table.nodes("CHARA_BASE_INFO").iter().enumerate() {
            if let (Some(team), Some(bid), Some(code)) =
                (base.nonzero(16), base.id(0), base.string(1))
                && team == id {
                    affiliated.push(json!({
                        "chara_base_id": hex(bid),
                        "internal_code": code,
                        "source": provenance(&bases_table.path, "CHARA_BASE_INFO", i),
                    }));
                }
        }
        o.insert("affiliated_characters".into(), Value::Array(affiliated));
        o.insert("fields".into(), row_json(row));
        o.insert(
            "roster".into(),
            json!({ "family": nie_wiki_vfs::UNSOURCED[0].family, "reason": nie_wiki_vfs::UNSOURCED[0].reason }),
        );
        o.insert(
            "source".into(),
            provenance(&table.path, "m_belongTeamInfoList", index),
        );
        Some(Value::Object(o))
    }

    /// Expected labels of a generic record.
    fn labels(
        &self,
        stem: &str,
        class: &str,
        vars: &[Raw],
        row: Option<&[(String, Raw)]>,
    ) -> Map<String, Value> {
        let mut out = Map::new();
        for (table, c, sel, text, sec, label) in REFS {
            if *table != stem || *c != class {
                continue;
            }
            let key = match sel {
                Ok(i) => vars
                    .get(*i)
                    .and_then(Raw::int)
                    .map(|v| v as u32)
                    .filter(|v| *v != 0),
                Err(field) => row.and_then(|r| row_id(r, field)),
            };
            if let Some(l) = key.and_then(|k| self.localized(text, *sec, k)) {
                out.insert((*label).to_owned(), l);
            }
        }
        out
    }

    /// The expected generic records of one class: `(index, record)`.
    fn table_records(&self, stem: &str, class: &str) -> Vec<Value> {
        let table = self.table(stem);
        match &table.body {
            RawBody::Rdbn(_) => table
                .rdbn_list(class)
                .into_iter()
                .flat_map(|l| l.rows.iter().enumerate())
                .map(|(index, row)| {
                    let mut o = Map::new();
                    o.insert("table".into(), stem.into());
                    o.insert("class".into(), class.into());
                    o.insert("index".into(), index.into());
                    o.insert("fields".into(), row_json(row));
                    let labels = self.labels(stem, class, &[], Some(row));
                    if !labels.is_empty() {
                        o.insert("labels".into(), Value::Object(labels));
                    }
                    o.insert("source".into(), provenance(&table.path, class, index));
                    Value::Object(o)
                })
                .collect(),
            RawBody::T2b(_) => {
                let classes = table.t2b_classes();
                let class_names: Vec<String> = classes.iter().map(|c| c.name.clone()).collect();
                let Some(this) = classes.iter().find(|c| c.name == class) else {
                    return Vec::new();
                };
                // Annotation classes of this main class: sort index and references.
                let sorts: Vec<&T2bClass<'_>> = classes
                    .iter()
                    .filter(|c| matches!(&c.role, Role::SortIndex(m) if m == class))
                    .collect();
                let refs: Vec<(&T2bClass<'_>, &String)> = classes
                    .iter()
                    .filter_map(|c| match &c.role {
                        Role::Reference(m, target) if m == class => Some((c, target)),
                        _ => None,
                    })
                    .collect();
                let _ = class_names;
                this.nodes
                    .iter()
                    .enumerate()
                    .map(|(index, node)| {
                        let mut o = Map::new();
                        o.insert("table".into(), stem.into());
                        o.insert("class".into(), class.into());
                        o.insert("index".into(), index.into());
                        o.insert(
                            "variables".into(),
                            Value::Array(node.vars.iter().map(Raw::json).collect()),
                        );
                        if !node.children.is_empty() {
                            o.insert(
                                "children".into(),
                                Value::Array(node.children.iter().map(RawNode::json).collect()),
                            );
                        }
                        if let Some(sort) = sorts.first().and_then(|c| c.nodes.get(index)) {
                            put(
                                &mut o,
                                "sort_index",
                                sort.vars.first().and_then(Raw::int).map(Value::from),
                            );
                        }
                        let mut ref_map = Map::new();
                        for (annotation, target) in &refs {
                            let Some(node) = annotation.nodes.get(index) else {
                                continue;
                            };
                            let (Some(offset), Some(count)) = (
                                node.vars.first().and_then(Raw::int),
                                node.vars.get(1).and_then(Raw::int),
                            ) else {
                                continue;
                            };
                            let target_nodes: &[RawNode] = classes
                                .iter()
                                .find(|c| &c.name == *target)
                                .map_or(&[], |c| c.nodes);
                            ref_map.insert(
                                (*target).clone(),
                                slice_json(target, target_nodes, offset, count),
                            );
                        }
                        if !ref_map.is_empty() {
                            o.insert("refs".into(), Value::Object(ref_map));
                        }
                        let labels = self.labels(stem, class, &node.vars, None);
                        if !labels.is_empty() {
                            o.insert("labels".into(), Value::Object(labels));
                        }
                        o.insert("source".into(), provenance(&table.path, class, index));
                        Value::Object(o)
                    })
                    .collect()
            }
        }
    }

    fn tables_info(&self) -> Vec<Value> {
        REGISTRY
            .iter()
            .map(|stem| {
                let table = self.table(stem);
                let classes: Vec<Value> = match &table.body {
                    RawBody::Rdbn(lists) => lists
                        .iter()
                        .map(|l| {
                            let mut c = Map::new();
                            c.insert("name".into(), l.name.clone().into());
                            c.insert("role".into(), "records".into());
                            c.insert("entries".into(), l.rows.len().into());
                            put(&mut c, "rich_family", rich_family(stem, &l.name));
                            Value::Object(c)
                        })
                        .collect(),
                    RawBody::T2b(_) => table
                        .t2b_classes()
                        .into_iter()
                        .map(|class| {
                            let mut c = Map::new();
                            c.insert("name".into(), class.name.clone().into());
                            let (role, target) = match &class.role {
                                Role::Records => ("records", None),
                                Role::SortIndex(main) => ("sort_index", Some(main.clone())),
                                Role::Reference(_, target) => ("reference", Some(target.clone())),
                            };
                            c.insert("role".into(), role.into());
                            c.insert("entries".into(), class.nodes.len().into());
                            put(&mut c, "target", target.map(Value::from));
                            put(&mut c, "rich_family", rich_family(stem, &class.name));
                            if !class.container.is_empty() {
                                c.insert(
                                    "container_variables".into(),
                                    Value::Array(class.container.iter().map(Raw::json).collect()),
                                );
                            }
                            Value::Object(c)
                        })
                        .collect(),
                };
                json!({
                    "table": stem,
                    "vfs_path": table.path,
                    "format": if matches!(table.body, RawBody::Rdbn(_)) { "rdbn" } else { "t2b" },
                    "classes": classes,
                })
            })
            .collect()
    }

    fn catalog(&self) -> Vec<Value> {
        let decoded: HashSet<&String> = REGISTRY.iter().map(|s| &self.table(s).path).collect();
        let mut paths: Vec<&String> = self
            .listing
            .iter()
            .filter(|p| p.starts_with("data/common/gamedata/") && p.contains(".cfg"))
            .collect();
        paths.sort();
        paths
            .into_iter()
            .map(|p| {
                json!({
                    "vfs_path": p,
                    "status": if decoded.contains(p) { "decoded" } else { "present" },
                })
            })
            .collect()
    }
}

fn rich_family(stem: &str, class: &str) -> Option<Value> {
    RICH_FAMILY
        .iter()
        .find(|(s, c, _)| *s == stem && *c == class)
        .map(|(_, _, f)| Value::from(*f))
}

// ----- fetching -----

#[derive(Clone)]
enum Source {
    Remote(reqwest::Client, String),
    Local(Arc<Wiki>),
}

struct Answer {
    status: u16,
    body: Vec<u8>,
}

impl Source {
    async fn get(&self, target: &str) -> anyhow::Result<Answer> {
        match self {
            Source::Remote(client, base) => {
                let url = format!("{}{target}", base.trim_end_matches('/'));
                let mut last = None;
                for _ in 0..3 {
                    match client.get(&url).send().await {
                        Ok(response) => {
                            let status = response.status().as_u16();
                            match response.bytes().await {
                                Ok(body) => {
                                    return Ok(Answer {
                                        status,
                                        body: body.to_vec(),
                                    });
                                }
                                Err(error) => last = Some(error),
                            }
                        }
                        Err(error) => last = Some(error),
                    }
                }
                Err(anyhow::anyhow!(
                    "GET {url}: {}",
                    last.map_or_else(String::new, |e| e.to_string())
                ))
            }
            Source::Local(wiki) => {
                let wiki = Arc::clone(wiki);
                let target = target.to_owned();
                tokio::task::spawn_blocking(move || {
                    let (path, raw_query) = target.split_once('?').unwrap_or((&target, ""));
                    let query = parse_query(raw_query);
                    let Some((route, params)) = match_route("GET", path) else {
                        return Answer {
                            status: 404,
                            body: Vec::new(),
                        };
                    };
                    let params: Vec<String> = params.iter().map(|p| percent_decode(p)).collect();
                    let response = if route.is_page() {
                        serve_wiki::page_route(route, &params, &query, &wiki)
                    } else if route.is_wiki() {
                        serve_wiki::json_route(route, &params, &query, &wiki)
                    } else {
                        return Answer {
                            status: 404,
                            body: Vec::new(),
                        };
                    };
                    Answer {
                        status: response.status,
                        body: response.body,
                    }
                })
                .await
                .map_err(Into::into)
            }
        }
    }

    async fn json(&self, target: &str) -> anyhow::Result<Value> {
        let answer = self.get(target).await?;
        if answer.status != 200 {
            bail!("GET {target}: HTTP {}", answer.status);
        }
        serde_json::from_slice(&answer.body).with_context(|| format!("GET {target}: not JSON"))
    }

    /// Every item of a paged list, in order.
    async fn all(&self, base: &str, extra: &str) -> anyhow::Result<(Vec<Value>, Value)> {
        let mut items = Vec::new();
        let mut first = Value::Null;
        let mut offset = 0;
        loop {
            let sep = if base.contains('?') { '&' } else { '?' };
            let page = self
                .json(&format!(
                    "{base}{sep}limit={PAGE_LIMIT}&offset={offset}{extra}"
                ))
                .await?;
            if first.is_null() {
                first = page.clone();
            }
            let batch = page["items"].as_array().cloned().unwrap_or_default();
            let total = page["total"].as_u64().unwrap_or(0) as usize;
            offset += PAGE_LIMIT;
            let done = batch.is_empty() || offset >= total;
            items.extend(batch);
            if done {
                break;
            }
        }
        Ok((items, first))
    }
}

// ----- the checks -----

pub(crate) async fn verify(args: &VerifyArgs, vfs: Arc<Vfs>) -> anyhow::Result<Report> {
    let install = Arc::new(Install::open(&vfs)?);
    let source = match &args.server {
        Some(url) => Source::Remote(
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()?,
            url.clone(),
        ),
        None => Source::Local(Arc::new(Wiki::new(vfs)?)),
    };
    let mut report = Report::default();
    let steps = [
        (
            "languages",
            check_languages(&source, &install, &mut report).await,
        ),
        (
            "catalog",
            check_catalog(&source, &install, &mut report).await,
        ),
        ("tables", check_tables(&source, &install, &mut report).await),
        (
            "characters",
            check_characters(&source, &install, &mut report).await,
        ),
        ("skills", check_skills(&source, &install, &mut report).await),
        ("teams", check_teams(&source, &install, &mut report).await),
    ];
    for (name, outcome) in steps {
        if let Err(error) = outcome {
            report.fail(&format!("{name}.fetch"), format!("{error:#}"));
        }
    }
    check_texts_unreferenced(&install, &mut report);
    if !args.no_pages
        && let Err(error) = check_pages(&source, &install, args.concurrency, &mut report).await
    {
        report.fail("pages.fetch", format!("{error:#}"));
    }
    Ok(report)
}

async fn check_languages(s: &Source, install: &Install, r: &mut Report) -> anyhow::Result<()> {
    let served = s.json("/v1/wiki/languages").await?;
    let expected: Vec<Value> = install
        .languages
        .iter()
        .map(|f| json!({ "folder": f, "tag": tag_of(f) }))
        .collect();
    r.compare(
        "languages",
        "/v1/wiki/languages",
        &served["items"],
        &Value::Array(expected),
    );
    Ok(())
}

async fn check_catalog(s: &Source, install: &Install, r: &mut Report) -> anyhow::Result<()> {
    let served = s.json("/v1/wiki/catalog").await?;
    r.compare(
        "catalog",
        "/v1/wiki/catalog",
        &served["items"],
        &Value::Array(install.catalog()),
    );
    Ok(())
}

async fn check_tables(s: &Source, install: &Install, r: &mut Report) -> anyhow::Result<()> {
    let served = s.json("/v1/wiki/tables").await?;
    r.compare(
        "tables.classes",
        "/v1/wiki/tables",
        &served["items"],
        &Value::Array(install.tables_info()),
    );
    for stem in REGISTRY {
        let table = install.table(stem);
        let class_names: Vec<(String, usize, bool)> = match &table.body {
            RawBody::Rdbn(lists) => lists
                .iter()
                .map(|l| (l.name.clone(), l.rows.len(), true))
                .collect(),
            RawBody::T2b(_) => table
                .t2b_classes()
                .into_iter()
                .map(|c| (c.name.clone(), c.nodes.len(), c.role == Role::Records))
                .collect(),
        };
        // Declared container counts agree with the nodes stored.
        for class in table.t2b_classes() {
            if let Some(Raw::Int(declared)) = class.container.first() {
                let records = class.nodes.iter().filter(|n| n.name == class.name).count();
                if usize::try_from(*declared).ok() == Some(records) {
                    r.pass("tables.declared_count");
                } else {
                    r.note(
                        "tables.declared_count",
                        format!(
                            "{stem}/{}: container declares {declared}, file stores {records}",
                            class.name
                        ),
                    );
                }
            }
        }
        for (class, count, records) in class_names {
            if !records {
                continue;
            }
            let base = format!("/v1/wiki/tables/{stem}/{class}");
            let (items, _) = s.all(&base, "").await?;
            let expected = install.table_records(stem, &class);
            if items.len() == expected.len() && items.len() == count {
                r.pass("tables.coverage");
            } else {
                r.fail(
                    "tables.coverage",
                    format!(
                        "{stem}/{class}: served {}, raw {}, class says {count}",
                        items.len(),
                        expected.len()
                    ),
                );
            }
            for (served, expected) in items.iter().zip(&expected) {
                r.compare(
                    "tables.records",
                    &format!("{stem}/{class}#{}", expected["index"]),
                    served,
                    expected,
                );
            }
        }
    }
    Ok(())
}

async fn check_characters(s: &Source, install: &Install, r: &mut Report) -> anyhow::Result<()> {
    let entries = chara_entries(install);
    let skills = install.skill_refs();
    let series = install.series();
    let teams = install.teams_names();
    let (items, first) = s.all("/v1/wiki/characters", "").await?;
    if items.len() == entries.entries.len()
        && first["total"].as_u64() == Some(entries.entries.len() as u64)
    {
        r.pass("characters.coverage");
    } else {
        r.fail(
            "characters.coverage",
            format!(
                "served {} (total {}), raw entries {}",
                items.len(),
                first["total"],
                entries.entries.len()
            ),
        );
    }
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, _, class) in &entries.entries {
        *counts.entry(class).or_default() += 1;
    }
    for class in first["classes"].as_array().into_iter().flatten() {
        let name = class["class"].as_str().unwrap_or("");
        if class["count"].as_u64() == counts.get(name).map(|c| *c as u64) {
            r.pass("characters.class_counts");
        } else {
            r.fail(
                "characters.class_counts",
                format!(
                    "{name}: served {}, raw {:?}",
                    class["count"],
                    counts.get(name)
                ),
            );
        }
    }
    for (served, entry) in items.iter().zip(&entries.entries) {
        let expected = install.character(*entry, &skills, &series, &teams);
        let id = served["chara_param_id"]
            .as_str()
            .or_else(|| served["base_source"]["index"].as_u64().map(|_| "base"))
            .unwrap_or("?");
        r.compare(
            "characters.records",
            &format!("{id} {:?}", served["internal_code"]),
            served,
            &expected,
        );
    }
    Ok(())
}

async fn check_skills(s: &Source, install: &Install, r: &mut Report) -> anyhow::Result<()> {
    let table = install.table("skill_config");
    let rows = table
        .rdbn_list("m_skillInfoList")
        .map(|l| &l.rows[..])
        .unwrap_or(&[]);
    let expected: Vec<(usize, Value)> = rows
        .iter()
        .enumerate()
        .filter_map(|(i, row)| Some((i, install.skill(i, row)?)))
        .collect();
    let (items, first) = s.all("/v1/wiki/skills", "").await?;
    if items.len() == expected.len() && first["total"].as_u64() == Some(expected.len() as u64) {
        r.pass("skills.coverage");
    } else {
        r.fail(
            "skills.coverage",
            format!(
                "served {} (total {}), raw rows with id and code {}",
                items.len(),
                first["total"],
                expected.len()
            ),
        );
    }
    if expected.len() == rows.len() {
        r.pass("skills.rows_without_id_or_code");
    } else {
        r.fail(
            "skills.rows_without_id_or_code",
            format!(
                "{} of {} rows have no skillID or skillIDStr (served only by /v1/wiki/tables)",
                rows.len() - expected.len(),
                rows.len()
            ),
        );
    }
    for (served, (i, expected)) in items.iter().zip(&expected) {
        r.compare(
            "skills.records",
            &format!("m_skillInfoList#{i}"),
            served,
            expected,
        );
    }
    Ok(())
}

async fn check_teams(s: &Source, install: &Install, r: &mut Report) -> anyhow::Result<()> {
    let table = install.table("belong_team_config");
    let rows = table
        .rdbn_list("m_belongTeamInfoList")
        .map(|l| &l.rows[..])
        .unwrap_or(&[]);
    let files = install.emblem_files();
    let expected: Vec<(usize, Value)> = rows
        .iter()
        .enumerate()
        .filter_map(|(i, row)| Some((i, install.team(i, row, &files)?)))
        .collect();
    let (items, first) = s.all("/v1/wiki/teams", "").await?;
    if items.len() == expected.len() && first["total"].as_u64() == Some(expected.len() as u64) {
        r.pass("teams.coverage");
    } else {
        r.fail(
            "teams.coverage",
            format!(
                "served {} (total {}), raw rows {}",
                items.len(),
                first["total"],
                expected.len()
            ),
        );
    }
    for (served, (i, expected)) in items.iter().zip(&expected) {
        r.compare(
            "teams.records",
            &format!("m_belongTeamInfoList#{i}"),
            served,
            expected,
        );
    }
    Ok(())
}

/// Every text key a table references resolves the same way in the served label and the raw
/// text; keys that resolve in no language are counted, not hidden.
fn check_texts_unreferenced(install: &Install, r: &mut Report) {
    let skills = install.skill_refs();
    for (id, (family, _, name_key)) in &skills.ids {
        match name_key {
            None => r.note(
                "texts.skill_names_absent",
                format!("{} {} has no name text key", family, hex(*id)),
            ),
            Some(key) if install.localized("skill_text", Sec::Noun, *key).is_none() => r.note(
                "texts.skill_names_absent",
                format!(
                    "{} {}: key {} has no row in skill_text in any language",
                    family,
                    hex(*id),
                    hex(*key)
                ),
            ),
            Some(_) => r.pass("texts.skill_names_absent"),
        }
    }
}

// ----- pages -----

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

fn between<'a>(html: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let from = html.find(start)? + start.len();
    let to = html[from..].find(end)? + from;
    Some(&html[from..to])
}

/// `(path, table, index)` of every item of the page's provenance list.
fn page_provenance(html: &str) -> Vec<(String, String, usize)> {
    let Some(list) = between(html, "<h2>Provenance</h2><ul>", "</ul>") else {
        return Vec::new();
    };
    list.split("<li>")
        .skip(1)
        .filter_map(|item| {
            let path = between(item, "<code>", "</code> table <code>")?;
            let rest = &item[item.find("</code> table <code>")? + "</code> table <code>".len()..];
            let table = between(rest, "", "</code> entry ")?;
            let index = rest[rest.find("</code> entry ")? + "</code> entry ".len()..]
                .trim_end_matches("</li>")
                .trim()
                .parse()
                .ok()?;
            Some((unescape(path), unescape(table), index))
        })
        .collect()
}

/// The rows of the page's `Stored fields` table as a JSON object.
fn page_fields(html: &str) -> Option<Value> {
    let table = between(html, "<h2>Stored fields</h2>", "</table>")?;
    let mut map = Map::new();
    for row in table.split("<tr><td><code>").skip(1) {
        let name = between(row, "", "</code></td><td><code>")?;
        let value = between(row, "</code></td><td><code>", "</code></td></tr>")?;
        map.insert(unescape(name), serde_json::from_str(&unescape(value)).ok()?);
    }
    Some(Value::Object(map))
}

fn page_pre(html: &str, summary: &str) -> Option<Value> {
    let marker = format!("<summary>{summary}</summary><pre>");
    let body = between(html, &marker, "</pre>")?;
    serde_json::from_str(&unescape(body)).ok()
}

fn page_variables(html: &str) -> Option<Value> {
    let body = between(html, "positions are as stored.</p><pre>", "</pre>")?;
    serde_json::from_str(&unescape(body)).ok()
}

struct PageJob {
    target: String,
    family: &'static str,
    /// The expected JSON of the record the page renders.
    expected: Value,
    lang: String,
}

async fn check_pages(
    s: &Source,
    install: &Arc<Install>,
    concurrency: usize,
    r: &mut Report,
) -> anyhow::Result<()> {
    // Home and list pages in every language.
    let home = s.get("/wiki").await?;
    let home = String::from_utf8_lossy(&home.body).into_owned();
    for stem in REGISTRY {
        let path = &install.table(stem).path;
        if home.contains(path.as_str()) {
            r.pass("pages.home");
        } else {
            r.fail("pages.home", format!("/wiki does not name {path}"));
        }
    }
    for entry in install.catalog() {
        let path = entry["vfs_path"].as_str().unwrap_or("");
        if home.contains(path) {
            r.pass("pages.home");
        } else {
            r.fail("pages.home", format!("/wiki does not list {path}"));
        }
    }

    let entries = chara_entries(install);
    let skills = install.skill_refs();
    let series = install.series();
    let teams = install.teams_names();
    let files = install.emblem_files();

    let mut jobs: Vec<PageJob> = Vec::new();
    let n_langs = install.languages.len().max(1);
    let mut expected_keys: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for (i, entry) in entries.entries.iter().enumerate() {
        let expected = install.character(*entry, &skills, &series, &teams);
        let key = match (expected["chara_param_id"].as_str(), entry.1) {
            (Some(id), _) => id.to_owned(),
            (None, Some(b)) => format!("base-{b}"),
            _ => continue,
        };
        expected_keys
            .entry("characters")
            .or_default()
            .push(key.clone());
        jobs.push(PageJob {
            target: format!("characters/{key}"),
            family: "characters",
            expected,
            lang: install.languages[i % n_langs].clone(),
        });
    }
    let skill_rows = install
        .table("skill_config")
        .rdbn_list("m_skillInfoList")
        .map(|l| l.rows.clone())
        .unwrap_or_default();
    for (i, row) in skill_rows.iter().enumerate() {
        if let Some(expected) = install.skill(i, row) {
            let key = expected["skill_id"].as_str().unwrap_or("").to_owned();
            expected_keys.entry("skills").or_default().push(key.clone());
            jobs.push(PageJob {
                target: format!("skills/{key}"),
                family: "skills",
                expected,
                lang: install.languages[i % n_langs].clone(),
            });
        }
    }
    let team_rows = install
        .table("belong_team_config")
        .rdbn_list("m_belongTeamInfoList")
        .map(|l| l.rows.clone())
        .unwrap_or_default();
    for (i, row) in team_rows.iter().enumerate() {
        if let Some(expected) = install.team(i, row, &files) {
            let key = expected["belong_team_id"].as_str().unwrap_or("").to_owned();
            expected_keys.entry("teams").or_default().push(key.clone());
            jobs.push(PageJob {
                target: format!("teams/{key}"),
                family: "teams",
                expected,
                lang: install.languages[i % n_langs].clone(),
            });
        }
    }
    let mut n = 0;
    for stem in REGISTRY {
        let table = install.table(stem);
        let classes: Vec<String> = match &table.body {
            RawBody::Rdbn(lists) => lists.iter().map(|l| l.name.clone()).collect(),
            RawBody::T2b(_) => table
                .t2b_classes()
                .into_iter()
                .filter(|c| c.role == Role::Records)
                .map(|c| c.name)
                .collect(),
        };
        for class in classes {
            if RICH.iter().any(|(s, c)| s == stem && *c == class) {
                continue;
            }
            for expected in install.table_records(stem, &class) {
                let index = expected["index"].as_u64().unwrap_or(0);
                jobs.push(PageJob {
                    target: format!("tables/{stem}/{class}/{index}"),
                    family: "tables",
                    expected,
                    lang: install.languages[n % n_langs].clone(),
                });
                n += 1;
            }
        }
    }

    // List pages: every record is linked, in every language.
    for lang in &install.languages {
        let tag = tag_of(lang);
        for (family, keys) in &expected_keys {
            let mut hrefs: HashSet<String> = HashSet::new();
            let mut offset = 0;
            let mut total = 0;
            loop {
                let answer = s
                    .get(&format!(
                        "/wiki/{tag}/{family}?limit={PAGE_LIMIT}&offset={offset}"
                    ))
                    .await?;
                let html = String::from_utf8_lossy(&answer.body).into_owned();
                if answer.status != 200 {
                    r.fail(
                        "pages.lists",
                        format!("/wiki/{tag}/{family}: HTTP {}", answer.status),
                    );
                    break;
                }
                let prefix = format!("href=\"/wiki/{tag}/{family}/");
                for part in html.split(&prefix).skip(1) {
                    if let Some(key) = part.split('"').next() {
                        hrefs.insert(percent_decode(key));
                    }
                }
                total = between(&html, "<p>", " match(es);")
                    .and_then(|t| t.parse::<usize>().ok())
                    .unwrap_or(0);
                offset += PAGE_LIMIT;
                if offset >= total {
                    break;
                }
            }
            let want: HashSet<String> = keys.iter().cloned().collect();
            if hrefs == want && total == keys.len() {
                r.pass("pages.lists");
            } else {
                r.fail(
                    "pages.lists",
                    format!(
                        "/wiki/{tag}/{family}: {} links for {} records (total shown {total}; missing {}, extra {})",
                        hrefs.len(),
                        keys.len(),
                        want.difference(&hrefs).count(),
                        hrefs.difference(&want).count()
                    ),
                );
            }
        }
    }

    // Record pages: stated provenance and displayed content against the raw entry.
    let results: Vec<(String, Vec<String>)> = futures_util::stream::iter(jobs)
        .map(|job| {
            let s = s.clone();
            let install = Arc::clone(install);
            async move {
                let tag = tag_of(&job.lang);
                let path = format!("/wiki/{tag}/{}", job.target);
                let outcome = match s.get(&path).await {
                    Ok(answer) if answer.status == 200 => {
                        let html = String::from_utf8_lossy(&answer.body).into_owned();
                        check_page(&install, &job, &html)
                    }
                    Ok(answer) => vec![format!("HTTP {}", answer.status)],
                    Err(error) => vec![error.to_string()],
                };
                (path, outcome)
            }
        })
        .buffer_unordered(concurrency.max(1))
        .collect()
        .await;
    for (path, problems) in results {
        if problems.is_empty() {
            r.pass("pages.records");
        } else {
            r.fail("pages.records", format!("{path}: {}", problems.join("; ")));
        }
    }
    Ok(())
}

/// Compares what a record page states and shows to the raw entries its provenance names.
fn check_page(install: &Install, job: &PageJob, html: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let stated = page_provenance(html);
    let expected_sources: Vec<Value> = match job.family {
        "characters" => ["param_source", "base_source"]
            .iter()
            .filter_map(|k| job.expected.get(*k).cloned())
            .collect(),
        _ => vec![job.expected["source"].clone()],
    };
    let stated_json: Vec<Value> = stated
        .iter()
        .map(|(p, t, i)| provenance(p, t, *i))
        .collect();
    if stated_json != expected_sources {
        problems.push(format!(
            "provenance {stated_json:?}, expected {expected_sources:?}"
        ));
    }
    // Each stated entry exists and the displayed content equals it.
    for (path, table, index) in &stated {
        let Some(raw) = install.tables.values().find(|t| &t.path == path) else {
            problems.push(format!("provenance path {path} is no registry table"));
            continue;
        };
        let mut shown = Vec::new();
        let mut want = Vec::new();
        match (job.family, table.as_str()) {
            ("characters", "CHARA_PARAM_INFO") | ("characters", "CHARA_BASE_INFO") => {
                let node = raw
                    .nodes(table)
                    .get(*index)
                    .map(|n| n.vars.iter().map(Raw::json).collect::<Vec<_>>());
                let summary = if table == "CHARA_PARAM_INFO" {
                    "chara_param"
                } else {
                    "chara_base"
                };
                shown.push(page_pre(html, summary).unwrap_or(Value::Null));
                want.push(node.map_or(Value::Null, Value::Array));
            }
            ("tables", _) if raw.rdbn_list(table).is_some() => {
                shown.push(page_fields(html).unwrap_or(Value::Null));
                want.push(
                    raw.rdbn_list(table)
                        .and_then(|l| l.rows.get(*index))
                        .map_or(Value::Null, |row| row_json(row)),
                );
            }
            ("tables", _) => {
                shown.push(page_variables(html).unwrap_or(Value::Null));
                want.push(raw.nodes(table).get(*index).map_or(Value::Null, |n| {
                    Value::Array(n.vars.iter().map(Raw::json).collect())
                }));
            }
            (_, list) => {
                shown.push(page_fields(html).unwrap_or(Value::Null));
                want.push(
                    raw.rdbn_list(list)
                        .and_then(|l| l.rows.get(*index))
                        .map_or(Value::Null, |row| row_json(row)),
                );
            }
        }
        for (a, b) in shown.iter().zip(&want) {
            let mut d = Vec::new();
            diff("", a, b, &mut d);
            if !d.is_empty() {
                problems.push(format!(
                    "page shows content differing from {path} {table}#{index}: {}",
                    d.join(", ")
                ));
            }
        }
    }
    // The heading is the record's label in the page language, else its code.
    let heading = between(html, "<h1>", "</h1>")
        .map(unescape)
        .unwrap_or_default();
    let label = |key: &str| {
        job.expected[key]["texts"][&job.lang]["forms"]
            .as_array()
            .and_then(|forms| forms.iter().find(|f| f["form"] == 0).or(forms.first()))
            .and_then(|f| f["text"].as_str())
            .map(str::to_owned)
    };
    let want_heading = match job.family {
        "characters" => label("name")
            .or_else(|| job.expected["internal_code"].as_str().map(str::to_owned))
            .or_else(|| job.expected["chara_base_id"].as_str().map(str::to_owned)),
        "skills" => {
            label("name").or_else(|| job.expected["skill_id_str"].as_str().map(str::to_owned))
        }
        "teams" => {
            label("name").or_else(|| job.expected["belong_team_id"].as_str().map(str::to_owned))
        }
        _ => Some(format!(
            "{} {} #{}",
            job.expected["table"].as_str().unwrap_or(""),
            job.expected["class"].as_str().unwrap_or(""),
            job.expected["index"]
        )),
    };
    if Some(&heading) != want_heading.as_ref() {
        problems.push(format!("heading {heading:?}, expected {want_heading:?}"));
    }
    problems
}
