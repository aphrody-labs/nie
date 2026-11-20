//! `nie deps audit`: measures what nie depends on and records it as a research log entry.
//!
//! Four inputs, all read in process: the game binary (`nie.exe`: PE import table, interface
//! strings, launch chain files), the kernel configuration (`/proc/config.gz` or a file or SSH
//! path), the Rust sources (every `Command::new` call site from the `syn` AST), and `Cargo.lock`.
//! The result is written as `<date>-dependency-audit.md` and `.json` under `docs/research/log`,
//! with the environment fingerprint and the content hashes the research protocol asks for.
//! `--check` re-reads only the code part (call sites and the versions of the tracked crates) and fails on drift against the
//! committed JSON, which is what CI runs.
//!
//! Which tools are known and kept, which crates are tracked and which kernel options are read comes
//! from `docs/specs/dependency-audit.policy.toml`; nothing of that is written in this file.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use clap::Args;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use syn::spanned::Spanned;
use syn::visit::Visit;

const POLICY_FILE: &str = "dependency-audit.policy.toml";

#[derive(Args)]
pub struct AuditArgs {
    /// Game installation: local path, `ssh://`, `sftp://`, `r2://`, `https://`, ... (default:
    /// `NIE_GAME_SOURCE`, else automatic resolution). Needed unless `--code-only`.
    #[arg(long, value_name = "URI", env = "NIE_GAME_SOURCE")]
    game_source: Option<String>,
    /// Kernel configuration: a path or URI of `config.gz` or a plain config (default: this host's
    /// `/proc/config.gz` when it exists).
    #[arg(long, value_name = "PATH_OR_URI")]
    kernel_config: Option<String>,
    /// Audit only the code (call sites and `Cargo.lock`), without the game and the kernel.
    #[arg(long)]
    code_only: bool,
    /// Print the computed JSON on stdout instead of the text summary; write nothing.
    #[arg(long)]
    json: bool,
    /// Compare the code part with the committed JSON of `--out`; exit 1 on drift, write nothing.
    #[arg(long)]
    check: bool,
    /// Output directory (default: `docs/research/log` of the checkout).
    #[arg(long, value_name = "DIR")]
    out: Option<PathBuf>,
    /// Entry date `YYYY-MM-DD` (default: today, UTC).
    #[arg(long, value_name = "DATE")]
    date: Option<String>,
    /// nie checkout holding `Cargo.lock` and `docs` (default: found from the current directory).
    #[arg(long, value_name = "DIR")]
    root: Option<PathBuf>,
    /// Policy file (default: `docs/specs/dependency-audit.policy.toml`).
    #[arg(long, value_name = "FILE")]
    policy: Option<PathBuf>,
    /// Trace the workloads of `docs/specs/asterinas-syscalls.policy.toml` with strace, diff the
    /// calls against the Asterinas SCML tables and write `docs/specs/asterinas-syscalls.json`
    /// (Linux only; reads `--game-source`).
    #[arg(long)]
    syscalls: bool,
    /// Asterinas checkout whose book holds the SCML tables (default: `../fw-nt/third_party/asterinas`).
    #[arg(long, value_name = "DIR", requires = "syscalls")]
    asterinas: Option<PathBuf>,
}

// ---------------------------------------------------------------------------------------------
// Policy
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
struct Policy {
    #[serde(default)]
    code: CodePolicy,
    #[serde(default)]
    lock: LockPolicy,
    #[serde(default)]
    kernel: KernelPolicy,
    #[serde(default)]
    game: GamePolicy,
}

#[derive(Debug, Default, Deserialize)]
struct CodePolicy {
    /// Directories of the checkout holding Rust sources.
    #[serde(default)]
    roots: Vec<String>,
    /// Program name (or `dynamic(<ident>)`) to the reason the call is kept.
    #[serde(default)]
    kept: BTreeMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
struct LockPolicy {
    #[serde(default)]
    tracked: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct KernelPolicy {
    /// Option names without the `CONFIG_` prefix.
    #[serde(default)]
    options: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct GamePolicy {
    /// Files of the launch chain, relative to the game directory.
    #[serde(default)]
    chain: Vec<String>,
    /// Steam app id, for the manifest that carries the build id.
    #[serde(default)]
    app_id: String,
}

// ---------------------------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Default)]
struct Report {
    fingerprint: Fingerprint,
    #[serde(skip_serializing_if = "Option::is_none")]
    game: Option<Game>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kernel: Option<Kernel>,
    code: Code,
    lock: Lock,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct Fingerprint {
    generated_utc: String,
    nie_version: String,
    git_commit: Option<String>,
    toolchain: Option<String>,
    cargo_lock_sha256: String,
    host: String,
    os: String,
    arch: String,
    kernel_release: Option<String>,
    cpu_model: Option<String>,
    memory_kib: Option<u64>,
    cpus: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct Game {
    source: String,
    build_id: Option<String>,
    nie_exe: Option<Binary>,
    cpk_list_sha256: Option<String>,
    chain: Vec<ChainFile>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Binary {
    size: u64,
    sha256: String,
    image_base: u64,
    dlls: Vec<DllImports>,
    dll_count: usize,
    imports_total: usize,
    named_functions: usize,
    ordinal_imports: usize,
    eos_families: Vec<Count>,
    steam_interfaces: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DllImports {
    dll: String,
    functions: usize,
    by_ordinal: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct Count {
    name: String,
    count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChainFile {
    path: String,
    present: bool,
    size: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Kernel {
    source: String,
    sha256: String,
    banner: Option<String>,
    options: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct Code {
    files_scanned: usize,
    sites: Vec<Site>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Site {
    file: String,
    line: usize,
    program: String,
    function: String,
    test: bool,
    /// Why the call is kept, from the policy; `None` means it is not reviewed yet.
    kept: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct Lock {
    packages: usize,
    registry: usize,
    git: usize,
    path: usize,
    tracked: Vec<Tracked>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Tracked {
    name: String,
    versions: Vec<String>,
}

// ---------------------------------------------------------------------------------------------
// Entry
// ---------------------------------------------------------------------------------------------

pub fn run(args: AuditArgs) -> Result<()> {
    let root = find_root(args.root.as_deref())?;
    if args.syscalls {
        return audit_syscalls(&args, &root);
    }
    let policy_path = args
        .policy
        .clone()
        .unwrap_or_else(|| root.join("docs/specs").join(POLICY_FILE));
    let policy: Policy = toml::from_str(
        &std::fs::read_to_string(&policy_path)
            .with_context(|| format!("read {}", policy_path.display()))?,
    )
    .with_context(|| format!("parse {}", policy_path.display()))?;
    let out = args
        .out
        .clone()
        .unwrap_or_else(|| root.join("docs/research/log"));
    let date = match &args.date {
        Some(d) => d.clone(),
        None => today_utc(),
    };
    let json_path = if args.check && args.date.is_none() {
        newest_entry(&out)?
    } else {
        out.join(format!("{date}-dependency-audit.json"))
    };

    let lock_bytes = std::fs::read(root.join("Cargo.lock")).context("read Cargo.lock")?;
    let mut report = Report {
        code: scan_code(&root, &policy.code)?,
        lock: read_lock(&lock_bytes, &policy.lock)?,
        fingerprint: fingerprint(&root, &lock_bytes),
        ..Report::default()
    };

    if args.check {
        let committed: Report = serde_json::from_slice(
            &std::fs::read(&json_path).with_context(|| format!("read {}", json_path.display()))?,
        )
        .with_context(|| format!("parse {}", json_path.display()))?;
        let mut drift = Vec::new();
        if committed.code.sites != report.code.sites {
            let old: BTreeSet<_> = committed.code.sites.iter().collect();
            let new: BTreeSet<_> = report.code.sites.iter().collect();
            for s in new.difference(&old) {
                drift.push(format!("new call site {}:{} {}", s.file, s.line, s.program));
            }
            for s in old.difference(&new) {
                drift.push(format!(
                    "gone call site {}:{} {}",
                    s.file, s.line, s.program
                ));
            }
        }
        for now in &report.lock.tracked {
            let before = committed.lock.tracked.iter().find(|t| t.name == now.name);
            if before.is_none_or(|t| t.versions != now.versions) {
                drift.push(format!(
                    "tracked crate {} is now {:?}",
                    now.name, now.versions
                ));
            }
        }
        if drift.is_empty() {
            println!(
                "dependency audit current: {} sites, {} packages",
                report.code.sites.len(),
                report.lock.packages
            );
            return Ok(());
        }
        for line in &drift {
            println!("{line}");
        }
        bail!("dependency audit drift: {} differences", drift.len());
    }

    if !args.code_only {
        let source = match args.game_source.clone() {
            Some(s) => s,
            None => {
                nie_formats::game_source::resolve(None).map_err(|e| anyhow!("game source: {e}"))?
            }
        };
        report.game = Some(audit_game(&source, &policy.game)?);
        report.kernel = audit_kernel(args.kernel_config.as_deref())?;
        if let Some(kernel) = &report.kernel {
            report.fingerprint.kernel_release = kernel_release(kernel);
        }
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    std::fs::create_dir_all(&out)?;
    let md_path = out.join(format!("{date}-dependency-audit.md"));
    std::fs::write(&json_path, serde_json::to_string_pretty(&report)? + "\n")?;
    std::fs::write(&md_path, render(&report, &date, &policy))?;
    println!(
        "wrote {} and {}: {} call sites ({} unreviewed), {} packages",
        md_path.display(),
        json_path.display(),
        report.code.sites.len(),
        report
            .code
            .sites
            .iter()
            .filter(|s| !s.test && s.kept.is_none())
            .count(),
        report.lock.packages
    );
    Ok(())
}

#[cfg(unix)]
fn audit_syscalls(args: &AuditArgs, root: &Path) -> Result<()> {
    const SCML_DIR: &str = "book/src/kernel/linux-compatibility/syscall-flag-coverage";
    let asterinas = args
        .asterinas
        .clone()
        .unwrap_or_else(|| root.join("../fw-nt/third_party/asterinas"));
    let policy_path = args
        .policy
        .clone()
        .unwrap_or_else(|| root.join("docs/specs/asterinas-syscalls.policy.toml"));
    let policy = nie_syscalls::Policy::parse(
        &std::fs::read_to_string(&policy_path)
            .with_context(|| format!("read {}", policy_path.display()))?,
    )
    .with_context(|| policy_path.display().to_string())?;
    let game = match args.game_source.clone() {
        Some(s) => s,
        None => nie_formats::game_source::resolve(None).map_err(|e| anyhow!("game source: {e}"))?,
    };
    let ctx = nie_syscalls::Context {
        nie: std::env::current_exe()?,
        game,
    };
    let report = nie_syscalls::audit(&asterinas.join(SCML_DIR), &asterinas, &policy, &ctx)?;
    let json = report.to_json()?;
    if args.json {
        print!("{json}");
        return Ok(());
    }
    let path = args
        .out
        .clone()
        .unwrap_or_else(|| root.join("docs/specs"))
        .join("asterinas-syscalls.json");
    std::fs::write(&path, json).with_context(|| format!("write {}", path.display()))?;
    println!(
        "wrote {}: {} workloads, {} syscalls unsupported by the SCML tables",
        path.display(),
        report.workloads.len(),
        report.unsupported.len()
    );
    for gap in &report.unsupported {
        println!("  {:>6}  {}", gap.count, gap.syscall);
    }
    Ok(())
}

#[cfg(not(unix))]
fn audit_syscalls(_args: &AuditArgs, _root: &Path) -> Result<()> {
    bail!("--syscalls needs strace and runs on Linux only")
}

/// The most recent `*-dependency-audit.json` of the log directory (names start with the date).
fn newest_entry(dir: &Path) -> Result<PathBuf> {
    std::fs::read_dir(dir)
        .with_context(|| format!("read {}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with("-dependency-audit.json"))
        })
        .max()
        .ok_or_else(|| anyhow!("no *-dependency-audit.json in {}", dir.display()))
}

fn find_root(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(dir) = explicit {
        return Ok(dir.to_path_buf());
    }
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join("Cargo.lock").is_file() && dir.join("docs/specs").is_dir() {
            return Ok(dir);
        }
        if !dir.pop() {
            bail!("no nie checkout above the current directory; pass --root");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Time and host
// ---------------------------------------------------------------------------------------------

fn civil(secs: u64) -> (i64, u32, u32, u64) {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day, rem)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn today_utc() -> String {
    let (y, m, d, _) = civil(now_secs());
    format!("{y:04}-{m:02}-{d:02}")
}

fn timestamp_utc() -> String {
    let (y, m, d, rem) = civil(now_secs());
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn read_trim(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// The commit `HEAD` points at, read from the `.git` directory without running git.
fn git_commit(root: &Path) -> Option<String> {
    let git = root.join(".git");
    let head = std::fs::read_to_string(git.join("HEAD")).ok()?;
    let head = head.trim();
    let Some(reference) = head.strip_prefix("ref: ") else {
        return Some(head.to_owned());
    };
    if let Ok(sha) = std::fs::read_to_string(git.join(reference)) {
        return Some(sha.trim().to_owned());
    }
    std::fs::read_to_string(git.join("packed-refs"))
        .ok()?
        .lines()
        .find_map(|l| l.strip_suffix(reference).map(|s| s.trim().to_owned()))
}

fn fingerprint(root: &Path, lock: &[u8]) -> Fingerprint {
    let toolchain = std::fs::read_to_string(root.join("rust-toolchain.toml"))
        .ok()
        .and_then(|t| toml::from_str::<toml::Table>(&t).ok())
        .and_then(|t| {
            t.get("toolchain")?
                .get("channel")?
                .as_str()
                .map(str::to_owned)
        });
    let cpu_model = std::fs::read_to_string("/proc/cpuinfo").ok().and_then(|t| {
        t.lines()
            .find_map(|l| l.strip_prefix("model name").and_then(|r| r.split_once(':')))
            .map(|(_, v)| v.trim().to_owned())
    });
    let memory_kib = std::fs::read_to_string("/proc/meminfo").ok().and_then(|t| {
        t.lines()
            .find_map(|l| l.strip_prefix("MemTotal:"))
            .and_then(|v| v.trim().trim_end_matches("kB").trim().parse().ok())
    });
    let host = std::env::var("HOSTNAME")
        .ok()
        .or_else(|| read_trim("/etc/hostname"))
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .unwrap_or_default();
    Fingerprint {
        generated_utc: timestamp_utc(),
        nie_version: env!("CARGO_PKG_VERSION").to_owned(),
        git_commit: git_commit(root),
        toolchain,
        cargo_lock_sha256: hex::encode(Sha256::digest(lock)),
        host,
        os: std::env::consts::OS.to_owned(),
        arch: std::env::consts::ARCH.to_owned(),
        kernel_release: read_trim("/proc/sys/kernel/osrelease"),
        cpu_model,
        memory_kib,
        cpus: std::thread::available_parallelism().map_or(0, usize::from),
    }
}

// ---------------------------------------------------------------------------------------------
// Game
// ---------------------------------------------------------------------------------------------

/// The installation, local or behind a remote transport of `nie-formats`.
enum Source {
    Local(PathBuf),
    Remote(nie_formats::vfs_remote::RemoteStorage),
}

impl Source {
    fn open(uri: &str) -> Result<Self> {
        if nie_formats::game_source::is_remote(uri) {
            let remote = nie_formats::vfs_remote::RemoteStorage::connect(uri)
                .map_err(|e| anyhow!("connect {uri}: {e}"))?;
            return Ok(Self::Remote(remote));
        }
        let path = PathBuf::from(uri);
        let base = if path.join("nie.exe").is_file() || !path.join("data/nie.exe").is_file() {
            path
        } else {
            path.join("data")
        };
        Ok(Self::Local(base))
    }

    fn read(&self, rel: &str) -> Option<Vec<u8>> {
        match self {
            Self::Local(base) => std::fs::read(base.join(rel)).ok(),
            Self::Remote(remote) => remote.read(rel).ok(),
        }
    }

    /// Size of a file when it exists (remote listings carry names only, so the size is `None`
    /// there and presence is what the listing says).
    fn stat(&self, rel: &str) -> (bool, Option<u64>) {
        match self {
            Self::Local(base) => {
                let meta = std::fs::metadata(base.join(rel)).ok();
                (meta.is_some(), meta.map(|m| m.len()))
            }
            Self::Remote(remote) => {
                let (parent, name) = rel.rsplit_once('/').unwrap_or(("", rel));
                let present = remote
                    .list(parent)
                    .map(|names| {
                        names
                            .iter()
                            .any(|n| n.trim_end_matches('/').rsplit('/').next() == Some(name))
                    })
                    .unwrap_or(false);
                (present, None)
            }
        }
    }
}

/// The library root of a Steam game directory (`.../steamapps/common/<name>`), as a source URI.
fn library_uri(uri: &str) -> Option<String> {
    uri.split_once("/steamapps/common/")
        .map(|(library, _)| library.to_owned())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn audit_game(uri: &str, policy: &GamePolicy) -> Result<Game> {
    let source = Source::open(uri)?;
    let exe = source
        .read("nie.exe")
        .ok_or_else(|| anyhow!("nie.exe not readable from {uri}"))?;
    let nie_exe = Some(audit_binary(&exe)?);
    let cpk_list_sha256 = source.read("data/cpk_list.cfg.bin").map(|b| sha256_hex(&b));
    let chain = policy
        .chain
        .iter()
        .map(|path| {
            let (present, size) = source.stat(path);
            ChainFile {
                path: path.clone(),
                present,
                size,
            }
        })
        .collect();
    let build_id = if policy.app_id.is_empty() {
        None
    } else {
        library_uri(uri)
            .and_then(|library| Source::open(&library).ok())
            .and_then(|lib| lib.read(&format!("steamapps/appmanifest_{}.acf", policy.app_id)))
            .and_then(|b| acf_value(&String::from_utf8_lossy(&b), "buildid"))
    };
    Ok(Game {
        source: uri.to_owned(),
        build_id,
        nie_exe,
        cpk_list_sha256,
        chain,
    })
}

/// The value of `"key"  "value"` in a Valve KeyValues text such as `appmanifest_*.acf`.
fn acf_value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let mut parts = line.split('"').skip(1).step_by(2);
        (parts.next()? == key).then(|| parts.next().map(str::to_owned))?
    })
}

fn audit_binary(bytes: &[u8]) -> Result<Binary> {
    let pe = goblin::pe::PE::parse(bytes).map_err(|e| anyhow!("parse nie.exe: {e}"))?;
    // `(all imports, imports by ordinal)` per DLL: a DLL imported only by ordinal still counts.
    let mut per_dll: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut families: BTreeMap<String, usize> = BTreeMap::new();
    let mut ordinals = 0;
    for import in &pe.imports {
        let name = import.name.as_ref();
        let entry = per_dll.entry(import.dll.to_owned()).or_default();
        entry.0 += 1;
        if name.starts_with("ORDINAL ") {
            entry.1 += 1;
            ordinals += 1;
            continue;
        }
        if let Some(rest) = name.strip_prefix("EOS_") {
            let family = rest.split('_').next().unwrap_or(rest);
            *families.entry(family.to_owned()).or_default() += 1;
        }
    }
    let mut dlls: Vec<DllImports> = per_dll
        .into_iter()
        .map(|(dll, (functions, by_ordinal))| DllImports {
            dll,
            functions,
            by_ordinal,
        })
        .collect();
    dlls.sort_by(|a, b| b.functions.cmp(&a.functions).then(a.dll.cmp(&b.dll)));
    let mut eos_families: Vec<Count> = families
        .into_iter()
        .map(|(name, count)| Count { name, count })
        .collect();
    eos_families.sort_by(|a, b| b.count.cmp(&a.count).then(a.name.cmp(&b.name)));
    Ok(Binary {
        size: bytes.len() as u64,
        sha256: sha256_hex(bytes),
        image_base: pe.image_base,
        dll_count: dlls.len(),
        imports_total: pe.imports.len(),
        named_functions: pe.imports.len() - ordinals,
        ordinal_imports: ordinals,
        dlls,
        eos_families,
        steam_interfaces: steam_interfaces(bytes),
    })
}

/// Steamworks interface version strings (`SteamUser023`, `STEAMUGC_INTERFACE_VERSION021`, ...):
/// printable ASCII runs that start with `Steam` or `STEAM` and end with three digits.
fn steam_interfaces(bytes: &[u8]) -> Vec<String> {
    let mut found = BTreeSet::new();
    let mut start = None;
    for (i, &b) in bytes.iter().enumerate() {
        let printable = (0x21..0x7f).contains(&b);
        match (printable, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                let run = &bytes[s..i];
                start = None;
                if (6..=48).contains(&run.len())
                    && (run.starts_with(b"Steam") || run.starts_with(b"STEAM"))
                    && run[run.len() - 3..].iter().all(u8::is_ascii_digit)
                    && run[5..run.len() - 3]
                        .iter()
                        .all(|c| c.is_ascii_alphabetic() || *c == b'_')
                {
                    found.insert(String::from_utf8_lossy(run).into_owned());
                }
            }
            _ => {}
        }
    }
    found.into_iter().collect()
}

// ---------------------------------------------------------------------------------------------
// Kernel
// ---------------------------------------------------------------------------------------------

fn audit_kernel(location: Option<&str>) -> Result<Option<Kernel>> {
    let location = match location {
        Some(l) => l.to_owned(),
        None if Path::new("/proc/config.gz").is_file() => "/proc/config.gz".to_owned(),
        None => return Ok(None),
    };
    let raw = if nie_formats::game_source::is_remote(&location) {
        let (base, name) = location
            .rsplit_once('/')
            .ok_or_else(|| anyhow!("invalid kernel config URI {location}"))?;
        nie_formats::vfs_remote::RemoteStorage::connect(base)
            .map_err(|e| anyhow!("connect {base}: {e}"))?
            .read(name)
            .map_err(|e| anyhow!("read {location}: {e}"))?
    } else {
        std::fs::read(&location).with_context(|| format!("read {location}"))?
    };
    let text = if raw.starts_with(&[0x1f, 0x8b]) {
        let mut text = String::new();
        flate2::read::GzDecoder::new(raw.as_slice())
            .read_to_string(&mut text)
            .context("gunzip the kernel configuration")?;
        text
    } else {
        String::from_utf8_lossy(&raw).into_owned()
    };
    Ok(Some(Kernel {
        source: location,
        sha256: sha256_hex(text.as_bytes()),
        banner: text
            .lines()
            .find(|l| l.contains("Kernel Configuration"))
            .map(|l| l.trim_start_matches('#').trim().to_owned()),
        options: parse_kernel_config(&text),
    }))
}

/// Every `CONFIG_*` setting of a kernel configuration; `# CONFIG_X is not set` becomes `n`.
fn parse_kernel_config(text: &str) -> BTreeMap<String, String> {
    let mut options = BTreeMap::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("CONFIG_") {
            if let Some((name, value)) = rest.split_once('=') {
                options.insert(name.to_owned(), value.trim_matches('"').to_owned());
            }
        } else if let Some(rest) = line.strip_prefix("# CONFIG_")
            && let Some(name) = rest.strip_suffix(" is not set")
        {
            options.insert(name.to_owned(), "n".to_owned());
        }
    }
    options
}

fn kernel_release(kernel: &Kernel) -> Option<String> {
    kernel
        .banner
        .as_deref()
        .and_then(|b| b.split_whitespace().nth(1))
        .map(str::to_owned)
}

// ---------------------------------------------------------------------------------------------
// Code: Command::new call sites
// ---------------------------------------------------------------------------------------------

fn scan_code(root: &Path, policy: &CodePolicy) -> Result<Code> {
    let mut files = Vec::new();
    for dir in &policy.roots {
        collect_rs(&root.join(dir), &mut files);
    }
    files.sort();
    let mut sites = Vec::new();
    for file in &files {
        let rel = file
            .strip_prefix(root)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        let text = std::fs::read_to_string(file).with_context(|| format!("read {rel}"))?;
        let Ok(ast) = syn::parse_file(&text) else {
            continue;
        };
        let in_tests_dir = rel
            .split('/')
            .any(|c| matches!(c, "tests" | "benches" | "examples"));
        let mut finder = Finder {
            file: &rel,
            consts: BTreeMap::new(),
            test_depth: usize::from(in_tests_dir),
            functions: Vec::new(),
            sites: Vec::new(),
        };
        finder.visit_file(&ast);
        sites.extend(finder.sites);
    }
    for site in &mut sites {
        site.kept = policy.kept.get(&site.program).cloned();
    }
    sites.sort();
    Ok(Code {
        files_scanned: files.len(),
        sites,
    })
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if !matches!(&*name, "target" | "node_modules" | ".git") && !name.starts_with("target-")
            {
                collect_rs(&path, out);
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

struct Finder<'a> {
    file: &'a str,
    consts: BTreeMap<String, String>,
    test_depth: usize,
    functions: Vec<String>,
    sites: Vec<Site>,
}

fn is_test_attr(attr: &syn::Attribute) -> bool {
    let path = attr.path();
    if path.is_ident("test") {
        return true;
    }
    path.is_ident("cfg")
        && attr.meta.require_list().is_ok_and(|list| {
            list.tokens
                .to_string()
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .any(|word| word == "test")
        })
}

fn string_of(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Lit(l) => match &l.lit {
            syn::Lit::Str(s) => Some(s.value()),
            _ => None,
        },
        syn::Expr::Reference(r) => string_of(&r.expr),
        syn::Expr::Paren(p) => string_of(&p.expr),
        _ => None,
    }
}

impl Finder<'_> {
    fn program(&self, expr: &syn::Expr) -> String {
        if let Some(s) = string_of(expr) {
            return s;
        }
        match expr {
            syn::Expr::Reference(r) => self.program(&r.expr),
            syn::Expr::Paren(p) => self.program(&p.expr),
            syn::Expr::Path(p) => {
                let ident = p
                    .path
                    .segments
                    .last()
                    .map(|s| s.ident.to_string())
                    .unwrap_or_default();
                self.consts
                    .get(&ident)
                    .cloned()
                    .unwrap_or_else(|| format!("dynamic({ident})"))
            }
            syn::Expr::Field(f) => match &f.member {
                syn::Member::Named(n) => format!("dynamic({n})"),
                syn::Member::Unnamed(i) => format!("dynamic({})", i.index),
            },
            syn::Expr::Try(t) => self.program(&t.expr),
            syn::Expr::MethodCall(m)
                if matches!(m.method.to_string().as_str(), "unwrap" | "expect") =>
            {
                self.program(&m.receiver)
            }
            syn::Expr::MethodCall(m) => format!("dynamic({}())", m.method),
            syn::Expr::Call(c) => match &*c.func {
                syn::Expr::Path(p) => format!(
                    "dynamic({}())",
                    p.path
                        .segments
                        .last()
                        .map(|s| s.ident.to_string())
                        .unwrap_or_default()
                ),
                _ => "dynamic".to_owned(),
            },
            _ => "dynamic".to_owned(),
        }
    }
}

impl<'ast> Visit<'ast> for Finder<'_> {
    fn visit_file(&mut self, file: &'ast syn::File) {
        for item in &file.items {
            if let syn::Item::Const(c) = item
                && let Some(s) = string_of(&c.expr)
            {
                self.consts.insert(c.ident.to_string(), s);
            }
        }
        syn::visit::visit_file(self, file);
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let test = node.attrs.iter().any(is_test_attr);
        self.test_depth += usize::from(test);
        syn::visit::visit_item_mod(self, node);
        self.test_depth -= usize::from(test);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let test = node.attrs.iter().any(is_test_attr);
        self.test_depth += usize::from(test);
        self.functions.push(node.sig.ident.to_string());
        syn::visit::visit_item_fn(self, node);
        self.functions.pop();
        self.test_depth -= usize::from(test);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let test = node.attrs.iter().any(is_test_attr);
        self.test_depth += usize::from(test);
        self.functions.push(node.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, node);
        self.functions.pop();
        self.test_depth -= usize::from(test);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(p) = &*node.func {
            let segments: Vec<String> = p
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect();
            if segments.len() >= 2
                && segments[segments.len() - 1] == "new"
                && segments[segments.len() - 2] == "Command"
                && let Some(first) = node.args.first()
            {
                self.sites.push(Site {
                    file: self.file.to_owned(),
                    line: node.func.span().start().line,
                    program: self.program(first),
                    function: self.functions.last().cloned().unwrap_or_default(),
                    test: self.test_depth > 0,
                    kept: None,
                });
            }
        }
        syn::visit::visit_expr_call(self, node);
    }
}

// ---------------------------------------------------------------------------------------------
// Cargo.lock
// ---------------------------------------------------------------------------------------------

fn read_lock(bytes: &[u8], policy: &LockPolicy) -> Result<Lock> {
    let table: toml::Table =
        toml::from_str(&String::from_utf8_lossy(bytes)).context("parse Cargo.lock")?;
    let packages = table
        .get("package")
        .and_then(toml::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut lock = Lock {
        packages: packages.len(),
        ..Lock::default()
    };
    let mut versions: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for package in &packages {
        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .unwrap_or_default();
        let version = package
            .get("version")
            .and_then(toml::Value::as_str)
            .unwrap_or_default();
        match package.get("source").and_then(toml::Value::as_str) {
            None => lock.path += 1,
            Some(s) if s.starts_with("git+") => lock.git += 1,
            Some(_) => lock.registry += 1,
        }
        if policy.tracked.iter().any(|t| t == name) {
            versions
                .entry(name.to_owned())
                .or_default()
                .insert(version.to_owned());
        }
    }
    lock.tracked = policy
        .tracked
        .iter()
        .map(|name| Tracked {
            name: name.clone(),
            versions: versions
                .get(name)
                .map(|v| v.iter().cloned().collect())
                .unwrap_or_default(),
        })
        .collect();
    Ok(lock)
}

// ---------------------------------------------------------------------------------------------
// Markdown
// ---------------------------------------------------------------------------------------------

fn render(report: &Report, date: &str, policy: &Policy) -> String {
    use std::fmt::Write;
    let f = &report.fingerprint;
    let mut s = String::new();
    let _ = writeln!(s, "# {date}: dependency audit\n");
    let _ = writeln!(
        s,
        "## Question\n\nWhat does nie.exe import, what does the production kernel provide, which external \
         programs does nie's own code still start, and which crates carry the VFS, Steam and edge?\n"
    );
    let _ = writeln!(s, "## Method\n");
    let _ = writeln!(
        s,
        "```sh\nnie deps audit --game-source <uri> [--kernel-config <path-or-uri>]   # writes this file and its JSON\nnie deps audit --check                                              # CI: call sites and tracked crate versions only\n```\n"
    );
    let _ = writeln!(
        s,
        "Everything is read in process: the PE import table (goblin), printable strings of the binary, \
         `config.gz`, the AST of every Rust file under the policy roots (syn) and `Cargo.lock`. The \
         policy (`docs/specs/{POLICY_FILE}`) names the launch chain, the kernel options, the tracked \
         crates and the reason each kept process call is kept.\n"
    );
    let _ = writeln!(s, "## Environment\n");
    let _ = writeln!(s, "- Generated: {}", f.generated_utc);
    let _ = writeln!(
        s,
        "- nie {} at commit `{}`",
        f.nie_version,
        f.git_commit.as_deref().unwrap_or("unknown")
    );
    let _ = writeln!(
        s,
        "- Toolchain channel: {}",
        f.toolchain.as_deref().unwrap_or("unknown")
    );
    let _ = writeln!(s, "- `Cargo.lock` sha256: `{}`", f.cargo_lock_sha256);
    let _ = writeln!(
        s,
        "- Host `{}`, {} {}, kernel {}, {} CPUs{}{}",
        f.host,
        f.os,
        f.arch,
        f.kernel_release.as_deref().unwrap_or("n/a"),
        f.cpus,
        f.cpu_model
            .as_deref()
            .map(|m| format!(", {m}"))
            .unwrap_or_default(),
        f.memory_kib
            .map(|k| format!(", {} MiB", k / 1024))
            .unwrap_or_default()
    );
    if let Some(game) = &report.game {
        let _ = writeln!(
            s,
            "- Game source `{}`, build id {}",
            game.source,
            game.build_id.as_deref().unwrap_or("n/a")
        );
        if let Some(h) = &game.cpk_list_sha256 {
            let _ = writeln!(s, "- `data/cpk_list.cfg.bin` sha256: `{h}`");
        }
    }
    let _ = writeln!(s);

    if let Some(game) = &report.game {
        let _ = writeln!(s, "## nie.exe\n");
        if let Some(b) = &game.nie_exe {
            let _ = writeln!(
                s,
                "- {} bytes, sha256 `{}`, image base `0x{:x}`\n- **{} DLLs, {} imports** in the import table ({} by name, {} by ordinal)\n",
                b.size,
                b.sha256,
                b.image_base,
                b.dll_count,
                b.imports_total,
                b.named_functions,
                b.ordinal_imports
            );
            let _ = writeln!(s, "| DLL | Imports | By ordinal |\n|---|---|---|");
            for d in &b.dlls {
                let _ = writeln!(s, "| {} | {} | {} |", d.dll, d.functions, d.by_ordinal);
            }
            let _ = writeln!(s, "\nEOS families by function count:");
            let families: Vec<String> = b
                .eos_families
                .iter()
                .map(|c| format!("{} {}", c.name, c.count))
                .collect();
            let _ = writeln!(s, "\n{}\n", families.join(", "));
            let _ = writeln!(
                s,
                "Steamworks interface versions in the strings: {}\n",
                b.steam_interfaces.join(", ")
            );
        }
        let _ = writeln!(
            s,
            "Launch chain files:\n\n| File | Present | Size |\n|---|---|---|"
        );
        for c in &game.chain {
            let _ = writeln!(
                s,
                "| {} | {} | {} |",
                c.path,
                if c.present { "yes" } else { "no" },
                c.size.map_or_else(|| "n/a".to_owned(), |n| n.to_string())
            );
        }
        let _ = writeln!(s);
    }

    if let Some(k) = &report.kernel {
        let _ = writeln!(s, "## Kernel\n");
        let _ = writeln!(
            s,
            "Source `{}` (sha256 `{}`){}\n",
            k.source,
            k.sha256,
            k.banner
                .as_deref()
                .map(|b| format!(", {b}"))
                .unwrap_or_default()
        );
        let _ = writeln!(s, "| Option | Value |\n|---|---|");
        for name in &policy.kernel.options {
            let value = k.options.get(name).map_or("unset", String::as_str);
            let _ = writeln!(s, "| CONFIG_{name} | {value} |");
        }
        let _ = writeln!(s);
    }

    let _ = writeln!(s, "## External processes started by nie's code\n");
    let _ = writeln!(
        s,
        "{} Rust files scanned, {} `Command::new` sites ({} outside tests).\n",
        report.code.files_scanned,
        report.code.sites.len(),
        report.code.sites.iter().filter(|x| !x.test).count()
    );
    let mut by_program: BTreeMap<&str, Vec<&Site>> = BTreeMap::new();
    for site in report.code.sites.iter().filter(|x| !x.test) {
        by_program.entry(&site.program).or_default().push(site);
    }
    let _ = writeln!(s, "| Program | Sites | Status |\n|---|---|---|");
    for (program, sites) in &by_program {
        let status = sites[0].kept.as_deref().unwrap_or("not reviewed");
        let places: Vec<String> = sites
            .iter()
            .map(|x| format!("`{}:{}`", x.file, x.line))
            .collect();
        let _ = writeln!(
            s,
            "| `{program}` | {} | {status} ({}) |",
            sites.len(),
            places.join(", ")
        );
    }
    let tests: Vec<&Site> = report.code.sites.iter().filter(|x| x.test).collect();
    if !tests.is_empty() {
        let places: Vec<String> = tests
            .iter()
            .map(|x| format!("`{}:{}` {}", x.file, x.line, x.program))
            .collect();
        let _ = writeln!(s, "\nIn tests: {}.", places.join(", "));
    }

    let l = &report.lock;
    let _ = writeln!(s, "\n## Cargo.lock\n");
    let _ = writeln!(
        s,
        "{} packages: {} from a registry, {} from git, {} local.\n",
        l.packages, l.registry, l.git, l.path
    );
    let _ = writeln!(s, "| Crate | Versions |\n|---|---|");
    for t in &l.tracked {
        let v = if t.versions.is_empty() {
            "absent".to_owned()
        } else {
            t.versions.join(", ")
        };
        let _ = writeln!(s, "| {} | {v} |", t.name);
    }
    let _ = writeln!(s, "\n## Limits\n");
    let _ = writeln!(
        s,
        "- Imports are the static import table; delay-loaded DLLs and COM vtable calls (D3D11) are not in it.\n- Call sites are `Command::new` paths seen by the parser; processes started through other APIs, macros or shell strings are not counted.\n- Interface strings are matched by shape, not by the call sites that use them."
    );
    s
}
