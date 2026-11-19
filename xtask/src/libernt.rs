//! `libernt-closure`: PE import closure of the nie launch chain over a LiberNT build tree.
//!
//! Reads every root binary in place (a local path or `ssh://host/path`, streamed into memory, never
//! stored), hashes it, walks static and delay imports and export forwarders, resolves each DLL
//! first against the game directory, then through the LiberNT apiset table, then against the PE
//! modules a LiberNT build produced, and writes the keep list as JSON.
//!
//! ```text
//! cargo xtask libernt-closure --game ssh://prd/<dir> --build <LiberNT output dir> \
//!     --source <LiberNT checkout> --drivers <name,name,...> --out docs/specs/libernt-closure.json
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::Serialize as _;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::util::hex;

/// Arguments of `libernt-closure`.
#[derive(clap::Args, Debug)]
pub struct Args {
    /// Game installation: a local directory or `ssh://host/path`.
    #[arg(long)]
    pub game: String,
    /// LiberNT output directory (the PE modules a build produced).
    #[arg(long)]
    pub build: PathBuf,
    /// LiberNT checkout (apiset table and hive definitions).
    #[arg(long)]
    pub source: PathBuf,
    /// Extra driver names to keep, comma separated.
    #[arg(long, default_value = "")]
    pub drivers: String,
    /// JSON file to write.
    #[arg(long)]
    pub out: PathBuf,
}

const ROOTS: [&str; 10] = [
    "nie.exe",
    "GameBootstrapper.exe",
    "EACLauncher.exe",
    "steam_api64.dll",
    "sdkencryptedappticket64.dll",
    "libHttpClient.Win32.dll",
    "libcurl.dll",
    "EOSSDK-Win64-Shipping.dll",
    "EasyAntiCheat/EasyAntiCheat_EOS_Setup.exe",
    "EpicOnlineServices/EpicOnlineServicesInstaller.exe",
];
const PE_SUFFIXES: [&str; 9] = [
    ".dll", ".exe", ".sys", ".drv", ".ocx", ".cpl", ".ax", ".acm", ".tlb",
];
const NT_CHAIN: [&str; 9] = [
    "ntoskrnl.exe",
    "hal.dll",
    "kdcom.dll",
    "bootvid.dll",
    "smss.exe",
    "csrss.exe",
    "winlogon.exe",
    "services.exe",
    "lsass.exe",
];
const HIVES: [&str; 3] = ["hivesys.inf", "hivesys_amd64.inf", "hivesft.inf"];
const KERNEL_MODULES: [&str; 6] = [
    "ntoskrnl.exe",
    "hal.dll",
    "ntkrnlmp.exe",
    "win32k.sys",
    "ndis.sys",
    "wdfldr.sys",
];

/// A parsed PE image: the parts the closure needs (sections, data directories, import tables).
struct Pe<'a> {
    data: &'a [u8],
    machine: u16,
    is64: bool,
    dirs: Vec<(u32, u32)>,
    /// `(virtual address, size in memory, raw offset, raw size)`.
    sections: Vec<(u32, u32, u32, u32)>,
}

/// One imported module: whether only the delay-import table names it, and its symbols.
struct Import {
    delay: bool,
    symbols: BTreeSet<String>,
}

fn u16_at(data: &[u8], off: usize) -> Result<u16> {
    let bytes = data
        .get(off..off + 2)
        .with_context(|| format!("truncated PE at {off:#x}"))?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn u32_at(data: &[u8], off: usize) -> Result<u32> {
    let bytes = data
        .get(off..off + 4)
        .with_context(|| format!("truncated PE at {off:#x}"))?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

impl<'a> Pe<'a> {
    fn parse(data: &'a [u8]) -> Result<Self> {
        if data.get(..2) != Some(b"MZ") {
            bail!("not MZ");
        }
        let pe = u32_at(data, 0x3C)? as usize;
        if data.get(pe..pe + 4) != Some(b"PE\0\0") {
            bail!("not PE");
        }
        let machine = u16_at(data, pe + 4)?;
        let nsec = usize::from(u16_at(data, pe + 6)?);
        let optsize = usize::from(u16_at(data, pe + 20)?);
        let opt = pe + 24;
        let is64 = u16_at(data, opt)? == 0x20B;
        let dd = opt + if is64 { 112 } else { 96 };
        let ndd = u32_at(data, dd - 4)? as usize;
        let mut dirs = Vec::new();
        for i in 0..ndd.min(16) {
            dirs.push((u32_at(data, dd + 8 * i)?, u32_at(data, dd + 8 * i + 4)?));
        }
        let sec = opt + optsize;
        let mut sections = Vec::new();
        for i in 0..nsec {
            let at = sec + 40 * i;
            let vsize = u32_at(data, at + 8)?;
            let va = u32_at(data, at + 12)?;
            let rawsize = u32_at(data, at + 16)?;
            let raw = u32_at(data, at + 20)?;
            sections.push((va, vsize.max(rawsize), raw, rawsize));
        }
        Ok(Self {
            data,
            machine,
            is64,
            dirs,
            sections,
        })
    }

    /// File offset of an RVA, if it lies in the raw data of a section (or in the headers).
    fn off(&self, rva: u64) -> Option<usize> {
        for &(va, size, raw, rawsize) in &self.sections {
            let (va, size) = (u64::from(va), u64::from(size));
            if va <= rva && rva < va + size {
                let o = rva - va;
                return (o < u64::from(rawsize) || rawsize == 0)
                    .then(|| (u64::from(raw) + o) as usize);
            }
        }
        (rva < 0x400).then_some(rva as usize)
    }

    /// NUL-terminated Latin-1 string at an RVA.
    fn cstr(&self, rva: u64) -> Option<String> {
        let o = self.off(rva)?;
        let rest = self.data.get(o..).unwrap_or(&[]);
        let end = rest.iter().position(|b| *b == 0).unwrap_or(rest.len());
        Some(rest[..end].iter().map(|b| char::from(*b)).collect())
    }

    /// Symbol names and `#ordinal` entries of an import lookup table.
    fn thunks(&self, rva: u64) -> Vec<String> {
        let size = if self.is64 { 8 } else { 4 };
        let flag = 1u64 << if self.is64 { 63 } else { 31 };
        let mut out = Vec::new();
        let mut o = self.off(rva);
        while let Some(at) = o {
            let slice = self.data.get(at..).unwrap_or(&[]);
            let mut v = 0u64;
            for (i, b) in slice.iter().take(size).enumerate() {
                v |= u64::from(*b) << (8 * i);
            }
            if v == 0 {
                break;
            }
            if v & flag != 0 {
                out.push(format!("#{}", v & 0xFFFF));
            } else if let Some(name) = self.cstr((v & 0x7FFF_FFFF) + 2) {
                out.push(name);
            }
            o = Some(at + size);
        }
        out
    }

    /// Modules named by the import and delay-import tables.
    fn imports(&self) -> Result<BTreeMap<String, Import>> {
        let mut out: BTreeMap<String, Import> = BTreeMap::new();
        let dir = |n: usize| self.dirs.get(n).map_or(0, |d| d.0);
        let rva = dir(1);
        let mut o = (rva != 0).then(|| self.off(u64::from(rva))).flatten();
        while let Some(at) = o {
            let oft = u32_at(self.data, at)?;
            let name = u32_at(self.data, at + 12)?;
            let ft = u32_at(self.data, at + 16)?;
            if name == 0 {
                break;
            }
            let dll = self
                .cstr(u64::from(name))
                .context("import name outside the image")?
                .to_lowercase();
            let entry = out.entry(dll).or_insert_with(|| Import {
                delay: false,
                symbols: BTreeSet::new(),
            });
            entry
                .symbols
                .extend(self.thunks(u64::from(if oft != 0 { oft } else { ft })));
            o = Some(at + 20);
        }
        let rva = dir(13);
        let mut o = (rva != 0).then(|| self.off(u64::from(rva))).flatten();
        while let Some(at) = o {
            let attrs = u32_at(self.data, at)?;
            let name = u32_at(self.data, at + 4)?;
            let int_rva = u32_at(self.data, at + 16)?;
            if name == 0 || attrs & 1 == 0 {
                break;
            }
            let dll = self
                .cstr(u64::from(name))
                .context("delay import name outside the image")?
                .to_lowercase();
            let entry = out.entry(dll).or_insert_with(|| Import {
                delay: true,
                symbols: BTreeSet::new(),
            });
            entry.symbols.extend(self.thunks(u64::from(int_rva)));
            o = Some(at + 32);
        }
        Ok(out)
    }

    /// DLL names the export directory forwards to.
    fn forwarders(&self) -> Result<BTreeSet<String>> {
        let (rva, size) = self.dirs.first().copied().unwrap_or((0, 0));
        let Some(o) = (rva != 0).then(|| self.off(u64::from(rva))).flatten() else {
            return Ok(BTreeSet::new());
        };
        let nfuncs = u32_at(self.data, o + 20)? as usize;
        let funcs = u32_at(self.data, o + 28)?;
        let mut out = BTreeSet::new();
        if nfuncs == 0 {
            return Ok(out);
        }
        let fo = self
            .off(u64::from(funcs))
            .context("export function table outside the image")?;
        for i in 0..nfuncs {
            let f = u32_at(self.data, fo + 4 * i)?;
            if rva <= f
                && u64::from(f) < u64::from(rva) + u64::from(size)
                && let Some(target) = self.cstr(u64::from(f)).filter(|t| !t.is_empty())
                && let Some((module, _)) = target.split_once('.')
            {
                out.insert(format!("{}.dll", module.to_lowercase()));
            }
        }
        Ok(out)
    }
}

/// Where the game files are: a local directory, or a host reached through `ssh`.
struct Source {
    host: Option<String>,
    base: String,
}

impl Source {
    fn new(spec: &str) -> Self {
        match spec.strip_prefix("ssh://") {
            Some(rest) => {
                let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
                Self {
                    host: Some(host.to_owned()),
                    base: format!("/{path}"),
                }
            }
            None => Self {
                host: None,
                base: spec.to_owned(),
            },
        }
    }

    fn read(&self, rel: &str) -> Result<Vec<u8>> {
        let path = format!("{}/{rel}", self.base);
        match &self.host {
            Some(host) => {
                let out = Command::new("ssh")
                    .args([host.as_str(), "cat", &format!("'{path}'")])
                    .output()
                    .context("spawn ssh")?;
                if !out.status.success() {
                    bail!(
                        "ssh {host} cat {path}: {}",
                        String::from_utf8_lossy(&out.stderr).trim()
                    );
                }
                Ok(out.stdout)
            }
            None => std::fs::read(&path).with_context(|| path),
        }
    }

    /// Relative path of every `.dll` and `.exe` under the base, sorted.
    fn names(&self) -> Result<Vec<String>> {
        let mut names: Vec<String> = match &self.host {
            Some(host) => {
                let script = format!(
                    "cd '{}' && find . -type f \\( -iname '*.dll' -o -iname '*.exe' \\) -not -path './data/*' -not -path './_CommonRedist/*'",
                    self.base
                );
                let out = Command::new("ssh")
                    .args([host.as_str(), &script])
                    .output()
                    .context("spawn ssh")?;
                if !out.status.success() {
                    bail!(
                        "ssh {host} find: {}",
                        String::from_utf8_lossy(&out.stderr).trim()
                    );
                }
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .filter(|l| !l.is_empty())
                    .map(|l| l.strip_prefix("./").unwrap_or(l).to_owned())
                    .collect()
            }
            None => {
                let base = Path::new(&self.base);
                let mut found = Vec::new();
                walk(base, &mut |path| {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    if (name.ends_with(".dll") || name.ends_with(".exe"))
                        && let Ok(rel) = path.strip_prefix(base)
                    {
                        found.push(rel.to_string_lossy().replace('\\', "/"));
                    }
                });
                found
            }
        };
        names.sort();
        Ok(names)
    }
}

/// Visits every file below `dir` in a deterministic order (files of a directory first, then its
/// subdirectories, each sorted by name); directories named `CMakeFiles` and symlinked directories
/// are not entered.
fn walk(dir: &Path, visit: &mut dyn FnMut(&Path)) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = read.flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    let mut dirs = Vec::new();
    for entry in entries {
        let path = entry.path();
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            dirs.push(path);
        } else if std::fs::metadata(&path).is_ok_and(|m| m.is_file()) {
            visit(&path);
        }
    }
    for sub in dirs {
        if sub.file_name().is_some_and(|n| n == "CMakeFiles") {
            continue;
        }
        walk(&sub, visit);
    }
}

/// `api-ms-*` / `ext-ms-*` DLL name (lower case, `.dll`) to its host module, from the C table.
fn apiset_hosts_from_text(text: &str) -> Result<BTreeMap<String, String>> {
    let re = Regex::new(r#"L"((?:api|ext)-ms-[^"]+)"\), RTL_CONSTANT_STRING\(L"([^"]+)""#)?;
    Ok(re
        .captures_iter(text)
        .map(|c| (format!("{}.dll", c[1].to_lowercase()), c[2].to_lowercase()))
        .collect())
}

fn apiset_hosts(source: &Path) -> Result<BTreeMap<String, String>> {
    let table = source.join("sdk/lib/apisets/apisets.table.c");
    let bytes = std::fs::read(&table).with_context(|| table.display().to_string())?;
    apiset_hosts_from_text(&String::from_utf8_lossy(&bytes))
}

fn last_component(path: &str) -> String {
    path.rsplit(['\\', '/'])
        .next()
        .unwrap_or(path)
        .to_lowercase()
}

/// Boot and session chain read from the hive definitions: services started at boot, system or auto
/// start, the shell and logon values, the subsystem servers, plus the documented NT chain. The map
/// is module name to the reason it is a root.
fn os_roots_from_text(text: &str) -> Result<BTreeMap<String, String>> {
    let start_re = Regex::new(r#"Services\\([^\\"]+)","Start",0x00010001,0x0000000([0-9])"#)?;
    let image_re = Regex::new(r#"Services\\([^\\"]+)","ImagePath",0x000[0-9a-f]+,"([^"]+)""#)?;
    let dll_re =
        Regex::new(r#"Services\\([^\\"]+)\\Parameters","ServiceDll",0x000[0-9a-f]+,"([^"]+)""#)?;
    let shell_re = Regex::new(r#""(?:Shell|Userinit)",0x000[0-9a-f]+,"([^"]+)""#)?;
    let server_re = Regex::new(r"ServerDll=([A-Za-z0-9_]+)")?;
    let kmode_re = Regex::new(r#"SubSystems","Kmode",0x000[0-9a-f]+,"([^"]+)""#)?;

    let mut start: BTreeMap<String, u32> = BTreeMap::new();
    let mut image: BTreeMap<String, String> = BTreeMap::new();
    let mut dll: BTreeMap<String, String> = BTreeMap::new();
    for m in start_re.captures_iter(text) {
        start.insert(m[1].to_owned(), m[2].parse()?);
    }
    for m in image_re.captures_iter(text) {
        image.insert(m[1].to_owned(), m[2].to_owned());
    }
    for m in dll_re.captures_iter(text) {
        dll.insert(m[1].to_owned(), m[2].to_owned());
    }
    let mut roots: BTreeMap<String, String> = BTreeMap::new();
    let mut set_default = |key: String, why: String| {
        roots.entry(key).or_insert(why);
    };
    for (svc, st) in &start {
        if *st > 2 {
            continue;
        }
        let img = image
            .get(svc)
            .cloned()
            .unwrap_or_else(|| format!("system32\\drivers\\{svc}.sys"));
        let mut base = last_component(img.split(' ').next().unwrap_or(""));
        if base == "svchost.exe"
            && let Some(d) = dll.get(svc)
        {
            base = last_component(d);
        }
        set_default(base, format!("service {svc} Start={st}"));
    }
    for m in shell_re.captures_iter(text) {
        set_default(last_component(&m[1]), "Winlogon shell chain".to_owned());
    }
    for m in server_re.captures_iter(text) {
        set_default(
            format!("{}.dll", m[1].to_lowercase()),
            "csrss ServerDll".to_owned(),
        );
    }
    for m in kmode_re.captures_iter(text) {
        set_default(last_component(&m[1]), "Kmode subsystem".to_owned());
    }
    for n in NT_CHAIN {
        set_default(n.to_owned(), "documented NT startup chain".to_owned());
    }
    Ok(roots)
}

fn os_roots(source: &Path) -> Result<BTreeMap<String, String>> {
    let mut text = String::new();
    for hive in HIVES {
        let path = source.join("boot/bootdata").join(hive);
        let bytes = std::fs::read(&path).with_context(|| path.display().to_string())?;
        // Universal newlines, then line continuations (a backslash before the newline) are joined.
        let content = String::from_utf8_lossy(&bytes)
            .replace("\r\n", "\n")
            .replace('\r', "\n");
        text.push_str(&content.replace("\\\n", ""));
        text.push('\n');
    }
    os_roots_from_text(&text)
}

/// Lower-case file name to path of the first PE module (more than 64 bytes, `MZ`) under `build`.
fn build_index(build: &Path) -> BTreeMap<String, PathBuf> {
    let mut idx = BTreeMap::new();
    walk(build, &mut |path| {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if !PE_SUFFIXES.iter().any(|s| name.ends_with(s)) {
            return;
        }
        let is_pe = std::fs::metadata(path).is_ok_and(|m| m.len() > 64)
            && std::fs::read(path).is_ok_and(|b| b.starts_with(b"MZ"));
        if is_pe {
            idx.entry(name).or_insert_with(|| path.to_path_buf());
        }
    });
    idx
}

fn relpath(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// One module of the closure.
struct Module {
    kind: &'static str,
    path: String,
    sha256: String,
    size: usize,
    machine: u16,
    imports: BTreeMap<String, Import>,
    forwards: BTreeSet<String>,
}

fn module(kind: &'static str, data: &[u8], origin: String) -> Result<Module> {
    let pe = Pe::parse(data).with_context(|| origin.clone())?;
    Ok(Module {
        kind,
        path: origin,
        sha256: hex(&Sha256::digest(data)),
        size: data.len(),
        machine: pe.machine,
        imports: pe.imports()?,
        forwards: pe.forwarders()?,
    })
}

/// JSON object with its keys in sorted order, whatever the `Map` implementation keeps.
fn object(entries: BTreeMap<String, Value>) -> Value {
    let mut map = Map::new();
    for (k, v) in entries {
        map.insert(k, v);
    }
    Value::Object(map)
}

fn string_set(set: &BTreeSet<String>) -> Value {
    Value::Array(set.iter().map(|s| Value::String(s.clone())).collect())
}

fn module_json(m: &Module) -> Value {
    let imports = m
        .imports
        .iter()
        .map(|(dll, e)| {
            let entry = BTreeMap::from([
                ("delay".to_owned(), json!(e.delay)),
                ("symbols".to_owned(), string_set(&e.symbols)),
            ]);
            (dll.clone(), object(entry))
        })
        .collect();
    object(BTreeMap::from([
        ("kind".to_owned(), json!(m.kind)),
        ("path".to_owned(), json!(m.path)),
        ("sha256".to_owned(), json!(m.sha256)),
        ("size".to_owned(), json!(m.size)),
        ("machine".to_owned(), json!(format!("{:#x}", m.machine))),
        ("imports".to_owned(), object(imports)),
        ("forwards".to_owned(), string_set(&m.forwards)),
    ]))
}

type Nodes = BTreeMap<String, Module>;

/// Adds a module once; new keys go on the work stack.
fn add(
    nodes: &mut Nodes,
    queue: &mut Vec<String>,
    kind: &'static str,
    key: &str,
    data: &[u8],
    origin: String,
) -> Result<()> {
    if !nodes.contains_key(key) {
        nodes.insert(key.to_owned(), module(kind, data, origin)?);
        queue.push(key.to_owned());
    }
    Ok(())
}

/// What the closure found, ready to serialise.
fn closure(args: &Args) -> Result<Value> {
    let game = Source::new(&args.game);
    let game_files: BTreeMap<String, String> = game
        .names()?
        .into_iter()
        .map(|n| (n.to_lowercase(), n))
        .collect();
    let index = build_index(&args.build);
    let apisets = apiset_hosts(&args.source)?;

    let mut nodes = Nodes::new();
    let mut queue: Vec<String> = Vec::new();
    for root in ROOTS {
        let key = root.rsplit('/').next().unwrap_or(root).to_lowercase();
        add(
            &mut nodes,
            &mut queue,
            "game",
            &key,
            &game.read(root)?,
            root.to_owned(),
        )?;
    }
    let mut wanted = os_roots(&args.source)?;
    for driver in args.drivers.split(',').filter(|d| !d.is_empty()) {
        wanted.insert(driver.to_lowercase(), "extra driver".to_owned());
    }
    let mut roots_os: BTreeMap<String, String> = BTreeMap::new();
    let mut absent: BTreeMap<String, String> = BTreeMap::new();
    for (name, why) in wanted {
        match index.get(&name) {
            Some(path) => {
                let data = std::fs::read(path).with_context(|| path.display().to_string())?;
                add(
                    &mut nodes,
                    &mut queue,
                    "os",
                    &name,
                    &data,
                    relpath(path, &args.build),
                )?;
                roots_os.insert(name, why);
            }
            None => {
                absent.insert(name, why);
            }
        }
    }

    let mut unresolved: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut resolved_via: BTreeMap<String, String> = BTreeMap::new();
    while let Some(key) = queue.pop() {
        let deps: Vec<String> = match nodes.get(&key) {
            Some(rec) => rec
                .imports
                .keys()
                .chain(rec.forwards.iter())
                .cloned()
                .collect(),
            None => continue,
        };
        for dll in deps {
            let target = match apisets.get(&dll) {
                Some(host) => {
                    resolved_via.insert(dll.clone(), host.clone());
                    host.clone()
                }
                None => dll,
            };
            if nodes.contains_key(&target) {
                continue;
            }
            if let Some(name) = game_files.get(&target) {
                add(
                    &mut nodes,
                    &mut queue,
                    "game",
                    &target,
                    &game.read(name)?,
                    name.clone(),
                )?;
            } else if let Some(path) = index.get(&target) {
                let data = std::fs::read(path).with_context(|| path.display().to_string())?;
                add(
                    &mut nodes,
                    &mut queue,
                    "libernt",
                    &target,
                    &data,
                    relpath(path, &args.build),
                )?;
            } else {
                unresolved.entry(target).or_default().insert(key.clone());
            }
        }
    }

    let ntdll_syscalls: BTreeSet<String> = nodes
        .values()
        .filter_map(|r| r.imports.get("ntdll.dll"))
        .flat_map(|e| e.symbols.iter())
        .filter(|s| s.starts_with("Nt") || s.starts_with("Zw"))
        .cloned()
        .collect();
    let mut kernel_exports: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for rec in nodes.values() {
        for dll in KERNEL_MODULES {
            if let Some(e) = rec.imports.get(dll) {
                kernel_exports
                    .entry(dll.to_owned())
                    .or_default()
                    .extend(e.symbols.iter().cloned());
            }
        }
    }

    let commit = Command::new("git")
        .arg("-C")
        .arg(&args.source)
        .args(["rev-parse", "HEAD"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default();
    let keep: Vec<Value> = nodes
        .iter()
        .filter(|(_, v)| matches!(v.kind, "libernt" | "os"))
        .map(|(k, _)| json!(k))
        .collect();
    let strings = |m: &BTreeMap<String, String>| {
        object(m.iter().map(|(k, v)| (k.clone(), json!(v))).collect())
    };
    let sets = |m: &BTreeMap<String, BTreeSet<String>>| {
        object(m.iter().map(|(k, v)| (k.clone(), string_set(v))).collect())
    };

    println!(
        "modules={} libernt={} unresolved={} syscalls={}",
        nodes.len(),
        keep.len(),
        unresolved.len(),
        ntdll_syscalls.len()
    );
    let not_measured = BTreeMap::from([
        (
            "steam client (steam.exe, steamwebhelper)".to_owned(),
            json!("not part of the game installation read in place"),
        ),
        (
            "dynamic loads".to_owned(),
            json!(
                "LoadLibrary, COM registration and manifest dependencies are not visible in the import tables"
            ),
        ),
        (
            "installer payloads".to_owned(),
            json!(
                "EpicOnlineServicesInstaller.exe and EasyAntiCheat_EOS_Setup.exe are measured by their own import tables only"
            ),
        ),
    ]);
    Ok(object(BTreeMap::from([
        ("schema".to_owned(), json!(1)),
        (
            "method".to_owned(),
            json!(
                "cargo xtask libernt-closure: static plus delay PE imports and export forwarders, apiset names resolved through sdk/lib/apisets/apisets.table.c"
            ),
        ),
        ("libernt_commit".to_owned(), json!(commit)),
        ("roots".to_owned(), json!(ROOTS)),
        ("not_measured".to_owned(), object(not_measured)),
        (
            "modules".to_owned(),
            object(
                nodes
                    .iter()
                    .map(|(k, v)| (k.clone(), module_json(v)))
                    .collect(),
            ),
        ),
        ("keep_modules".to_owned(), Value::Array(keep)),
        ("os_roots".to_owned(), strings(&roots_os)),
        ("os_roots_without_binary".to_owned(), strings(&absent)),
        ("unresolved".to_owned(), sets(&unresolved)),
        ("apiset_resolution".to_owned(), strings(&resolved_via)),
        ("ntdll_syscalls".to_owned(), string_set(&ntdll_syscalls)),
        ("kernel_exports".to_owned(), sets(&kernel_exports)),
    ])))
}

/// Escapes every non-ASCII character as `\uXXXX`, as Python's `json.dump` does by default.
fn escape_non_ascii(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            let mut units = [0u16; 2];
            for unit in c.encode_utf16(&mut units) {
                let _ = write!(out, "\\u{unit:04x}");
            }
        }
    }
    out
}

/// The document as text: sorted keys, one space of indent, a trailing newline.
fn render(doc: &Value) -> Result<String> {
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(
        &mut buf,
        serde_json::ser::PrettyFormatter::with_indent(b" "),
    );
    doc.serialize(&mut ser)?;
    let mut text = escape_non_ascii(&String::from_utf8(buf)?);
    text.push('\n');
    Ok(text)
}

pub fn run(args: Args) -> Result<()> {
    let doc = closure(&args)?;
    std::fs::write(&args.out, render(&doc)?).with_context(|| args.out.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(buf: &mut [u8], at: usize, bytes: &[u8]) {
        buf[at..at + bytes.len()].copy_from_slice(bytes);
    }

    fn put32(buf: &mut [u8], at: usize, v: u32) {
        put(buf, at, &v.to_le_bytes());
    }

    fn put64(buf: &mut [u8], at: usize, v: u64) {
        put(buf, at, &v.to_le_bytes());
    }

    /// A PE32+ with one section (RVA 0x1000, file offset 0x400): a forwarder export, one import
    /// (by name and by ordinal) and one delay import.
    fn synthetic_pe() -> Vec<u8> {
        let mut f = vec![0u8; 0x400 + 0x1000];
        put(&mut f, 0, b"MZ");
        put32(&mut f, 0x3C, 0x40);
        put(&mut f, 0x40, b"PE\0\0");
        put(&mut f, 0x44, &0x8664u16.to_le_bytes());
        put(&mut f, 0x46, &1u16.to_le_bytes());
        put(&mut f, 0x54, &240u16.to_le_bytes());
        let opt = 0x58;
        put(&mut f, opt, &0x20Bu16.to_le_bytes());
        let dd = opt + 112;
        put32(&mut f, dd - 4, 16);
        for (n, rva, size) in [
            (0usize, 0x1000u32, 0x80u32),
            (1, 0x1100, 40),
            (13, 0x1300, 64),
        ] {
            put32(&mut f, dd + 8 * n, rva);
            put32(&mut f, dd + 8 * n + 4, size);
        }
        let sec = opt + 240;
        put(&mut f, sec, b".rdata\0\0");
        put32(&mut f, sec + 8, 0x1000);
        put32(&mut f, sec + 12, 0x1000);
        put32(&mut f, sec + 16, 0x1000);
        put32(&mut f, sec + 20, 0x400);
        let at = |rva: usize| rva - 0x1000 + 0x400;
        // Export directory with one function that forwards to OTHER.Func.
        put32(&mut f, at(0x1000) + 20, 1);
        put32(&mut f, at(0x1000) + 28, 0x1028);
        put32(&mut f, at(0x1028), 0x1040);
        put(&mut f, at(0x1040), b"OTHER.Func\0");
        // Import descriptor: KERNEL32.dll, CreateFileW and ordinal 5.
        put32(&mut f, at(0x1100), 0x1200);
        put32(&mut f, at(0x1100) + 12, 0x1180);
        put32(&mut f, at(0x1100) + 16, 0x1200);
        put(&mut f, at(0x1180), b"KERNEL32.dll\0");
        put64(&mut f, at(0x1200), 0x1240);
        put64(&mut f, at(0x1208), (1 << 63) | 5);
        put(&mut f, at(0x1242), b"CreateFileW\0");
        // Delay import descriptor: ADVAPI32.dll, RegOpenKeyW.
        put32(&mut f, at(0x1300), 1);
        put32(&mut f, at(0x1300) + 4, 0x1380);
        put32(&mut f, at(0x1300) + 16, 0x1400);
        put(&mut f, at(0x1380), b"ADVAPI32.dll\0");
        put64(&mut f, at(0x1400), 0x1440);
        put(&mut f, at(0x1442), b"RegOpenKeyW\0");
        f
    }

    #[test]
    fn synthetic_pe_imports_delay_imports_and_forwarders() {
        let data = synthetic_pe();
        let pe = Pe::parse(&data).unwrap();
        assert_eq!(pe.machine, 0x8664);
        assert!(pe.is64);
        let imports = pe.imports().unwrap();
        assert_eq!(
            imports.keys().map(String::as_str).collect::<Vec<_>>(),
            ["advapi32.dll", "kernel32.dll"]
        );
        let k = &imports["kernel32.dll"];
        assert!(!k.delay);
        assert_eq!(
            k.symbols.iter().map(String::as_str).collect::<Vec<_>>(),
            ["#5", "CreateFileW"]
        );
        let a = &imports["advapi32.dll"];
        assert!(a.delay);
        assert_eq!(
            a.symbols.iter().map(String::as_str).collect::<Vec<_>>(),
            ["RegOpenKeyW"]
        );
        assert_eq!(
            pe.forwarders().unwrap().into_iter().collect::<Vec<_>>(),
            ["other.dll"]
        );
    }

    #[test]
    fn module_record_hashes_and_lists_the_parse() {
        let data = synthetic_pe();
        let m = module("game", &data, "x.dll".into()).unwrap();
        assert_eq!(m.sha256, hex(&Sha256::digest(&data)));
        assert_eq!(m.size, data.len());
        let v = module_json(&m);
        assert_eq!(v["machine"], "0x8664");
        assert_eq!(v["imports"]["advapi32.dll"]["delay"], true);
        assert_eq!(v["forwards"][0], "other.dll");
    }

    #[test]
    fn rva_below_the_headers_maps_to_itself_and_bad_input_is_an_error() {
        let data = synthetic_pe();
        let pe = Pe::parse(&data).unwrap();
        assert_eq!(pe.off(0x80), Some(0x80));
        assert_eq!(pe.off(0x9000), None);
        assert!(Pe::parse(b"not a pe").is_err());
        assert!(Pe::parse(b"MZ").is_err());
    }

    #[test]
    fn hive_text_gives_the_boot_chain() {
        let text = concat!(
            "HKLM,\"SYSTEM\\CurrentControlSet\\Services\\beep\",\"Start\",0x00010001,0x00000001\n",
            "Services\\beep\",\"ImagePath\",0x00020000,\"system32\\drivers\\Beep.sys\"\n",
            "Services\\lanman\",\"Start\",0x00010001,0x00000002\n",
            "Services\\lanman\",\"ImagePath\",0x00020000,\"%SystemRoot%\\system32\\svchost.exe -k netsvcs\"\n",
            "Services\\lanman\\Parameters\",\"ServiceDll\",0x00020000,\"%SystemRoot%\\system32\\srvsvc.dll\"\n",
            "Services\\tcpip\",\"Start\",0x00010001,0x00000003\n",
            "Services\\noimage\",\"Start\",0x00010001,0x00000000\n",
            "\"Shell\",0x00000000,\"explorer.exe\"\n",
            "SharedSection=1024,3072 ServerDll=BASESRV,1 ServerDll=winsrv:UserServerDllInitialization\n",
            "SubSystems\",\"Kmode\",0x00020000,\"\\SystemRoot\\system32\\win32k.sys\"\n",
        );
        let roots = os_roots_from_text(text).unwrap();
        assert_eq!(roots["beep.sys"], "service beep Start=1");
        assert_eq!(roots["srvsvc.dll"], "service lanman Start=2");
        assert!(!roots.contains_key("svchost.exe"));
        assert!(!roots.contains_key("tcpip.sys"));
        assert_eq!(roots["noimage.sys"], "service noimage Start=0");
        assert_eq!(roots["explorer.exe"], "Winlogon shell chain");
        assert_eq!(roots["basesrv.dll"], "csrss ServerDll");
        assert_eq!(roots["winsrv.dll"], "csrss ServerDll");
        assert_eq!(roots["win32k.sys"], "Kmode subsystem");
        assert_eq!(roots["ntoskrnl.exe"], "documented NT startup chain");
    }

    #[test]
    fn apiset_table_rows_map_to_their_host() {
        let text = concat!(
            "{ RTL_CONSTANT_STRING(L\"api-ms-win-core-file-l1-1-0\"), RTL_CONSTANT_STRING(L\"KERNELBASE.dll\") },\n",
            "{ RTL_CONSTANT_STRING(L\"ext-ms-win-ntuser-window-l1-1-0\"), RTL_CONSTANT_STRING(L\"user32.dll\") },\n",
            "{ RTL_CONSTANT_STRING(L\"other-name\"), RTL_CONSTANT_STRING(L\"x.dll\") },\n",
        );
        let hosts = apiset_hosts_from_text(text).unwrap();
        assert_eq!(hosts.len(), 2);
        assert_eq!(hosts["api-ms-win-core-file-l1-1-0.dll"], "kernelbase.dll");
        assert_eq!(hosts["ext-ms-win-ntuser-window-l1-1-0.dll"], "user32.dll");
    }

    #[test]
    fn sources_parse_ssh_and_local_specs() {
        let s = Source::new("ssh://prd/srv/nie/steam/game");
        assert_eq!(
            (s.host.as_deref(), s.base.as_str()),
            (Some("prd"), "/srv/nie/steam/game")
        );
        let l = Source::new("C:/games/ie");
        assert_eq!((l.host, l.base.as_str()), (None, "C:/games/ie"));
    }

    #[test]
    fn json_matches_python_indent_one_with_ascii_escapes() {
        let doc = object(BTreeMap::from([
            ("b".to_owned(), json!([])),
            (
                "a".to_owned(),
                object(BTreeMap::from([("k".to_owned(), json!(["x\u{e9}"]))])),
            ),
        ]));
        assert_eq!(
            render(&doc).unwrap(),
            "{\n \"a\": {\n  \"k\": [\n   \"x\\u00e9\"\n  ]\n },\n \"b\": []\n}\n"
        );
    }

    #[test]
    fn build_index_takes_pe_files_only_and_skips_cmake_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let mut pe = vec![0u8; 80];
        pe[..2].copy_from_slice(b"MZ");
        std::fs::create_dir_all(dir.path().join("a/CMakeFiles")).unwrap();
        std::fs::write(dir.path().join("a/Good.DLL"), &pe).unwrap();
        std::fs::write(dir.path().join("a/CMakeFiles/skip.dll"), &pe).unwrap();
        std::fs::write(dir.path().join("a/small.dll"), b"MZ").unwrap();
        std::fs::write(dir.path().join("a/notpe.dll"), vec![0u8; 80]).unwrap();
        let idx = build_index(dir.path());
        assert_eq!(
            idx.keys().map(String::as_str).collect::<Vec<_>>(),
            ["good.dll"]
        );
    }
}
