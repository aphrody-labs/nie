//! The audit result and the matching of one strace log against the SCML tables.

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, anyhow};
use regex::Regex;
use sctrace::{CliReporterBuilder, Patterns, SctraceBuilder, StraceLogStream};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::workload::{Context, Outcome, Workload};

const EXAMPLE_LIMIT: usize = 240;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Report {
    pub asterinas: Source,
    pub environment: Environment,
    pub workloads: Vec<WorkloadReport>,
    /// Union over the workloads, most frequent first.
    pub unsupported: Vec<Unsupported>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Source {
    pub commit: Option<String>,
    pub scml_files: usize,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Environment {
    pub kernel: Option<String>,
    pub cpu: Option<String>,
    pub strace: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkloadReport {
    pub name: String,
    pub argv: Vec<String>,
    pub exit_code: Option<i32>,
    pub probe_status: Option<u16>,
    pub timed_out: bool,
    pub strace_sha256: String,
    pub calls: u64,
    pub syscalls: BTreeMap<String, u64>,
    pub unsupported: Vec<Unsupported>,
    pub parse_errors: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Unsupported {
    pub syscall: String,
    pub count: u64,
    pub example: String,
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

impl Report {
    /// The report as the committed JSON document.
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)? + "\n")
    }

    pub fn new(asterinas: &Path, scml_files: usize, workloads: Vec<WorkloadReport>) -> Self {
        let mut union: BTreeMap<String, Unsupported> = BTreeMap::new();
        for entry in workloads.iter().flat_map(|w| &w.unsupported) {
            union
                .entry(entry.syscall.clone())
                .and_modify(|u| u.count += entry.count)
                .or_insert_with(|| entry.clone());
        }
        let mut unsupported: Vec<_> = union.into_values().collect();
        unsupported.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.syscall.cmp(&b.syscall)));
        Self {
            asterinas: Source { commit: git_head(asterinas), scml_files },
            environment: Environment::detect(),
            workloads,
            unsupported,
        }
    }
}

impl Environment {
    fn detect() -> Self {
        let read = |p: &str| std::fs::read_to_string(p).ok().map(|s| s.trim().to_owned());
        Self {
            kernel: read("/proc/sys/kernel/osrelease"),
            cpu: std::fs::read_to_string("/proc/cpuinfo").ok().and_then(|t| {
                t.lines()
                    .find_map(|l| l.strip_prefix("model name").and_then(|r| r.split_once(':')))
                    .map(|(_, v)| v.trim().to_owned())
            }),
            strace: std::process::Command::new("strace")
                .arg("--version")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .and_then(|s| s.lines().next().map(str::to_owned)),
        }
    }
}

/// The commit `HEAD` of a checkout or submodule, read from `.git` without running git.
fn git_head(dir: &Path) -> Option<String> {
    let mut git = dir.join(".git");
    if git.is_file() {
        let text = std::fs::read_to_string(&git).ok()?;
        git = dir.join(text.trim().strip_prefix("gitdir: ")?);
    }
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

/// Matches one strace log against the SCML tables.
pub fn analyse(
    workload: &Workload,
    _ctx: &Context,
    outcome: &Outcome,
    log: &Path,
    scml: &[PathBuf],
) -> Result<WorkloadReport> {
    let bytes = std::fs::read(log).with_context(|| format!("read {}", log.display()))?;
    let text = String::from_utf8_lossy(&bytes);
    let call = Regex::new(r"^(?:\d+\s+)?([a-z_][a-z0-9_]*)\(").expect("static pattern");
    let mut syscalls: BTreeMap<String, u64> = BTreeMap::new();
    for line in text.lines() {
        if let Some(name) = call.captures(line).map(|c| c[1].to_owned()) {
            *syscalls.entry(name).or_default() += 1;
        }
    }

    let (unsupported, parse_errors) = match_log(log, scml)?;
    Ok(WorkloadReport {
        name: workload.name.clone(),
        argv: outcome.argv.clone(),
        exit_code: outcome.exit_code,
        probe_status: outcome.probe_status,
        timed_out: outcome.timed_out,
        strace_sha256: hex(&Sha256::digest(&bytes)),
        calls: syscalls.values().sum(),
        syscalls,
        unsupported,
        parse_errors,
    })
}

/// Lines per sctrace run: the unit that is bisected when sctrace panics on a line it has no rule for.
const CHUNK: usize = 2000;

/// Unsupported syscalls (aggregated per name) and the number of lines sctrace could not parse.
pub fn match_log(log: &Path, scml: &[PathBuf]) -> Result<(Vec<Unsupported>, u64)> {
    let patterns = Patterns::from_scml_files(&scml.to_vec()).map_err(|e| anyhow!("scml: {e:?}"))?;
    let text = std::fs::read_to_string(log).with_context(|| format!("read {}", log.display()))?;
    // Signal and exit notices (`--- SIGTERM ... ---`, `+++ killed by SIGTERM +++`) carry no syscall.
    let notice = Regex::new(r"^(?:\d+\s+)?(?:\+\+\+|---)").expect("static pattern");
    // strace abbreviates `FUTEX_WAIT|FUTEX_PRIVATE_FLAG` to `FUTEX_WAIT_PRIVATE`; the SCML tables
    // spell the flag out.
    let futex = Regex::new(r"\bFUTEX_([A-Z0-9_]+)_PRIVATE\b").expect("static pattern");
    let normalised: Vec<String> = text
        .lines()
        .filter(|l| !notice.is_match(l))
        .map(|l| futex.replace_all(l, "FUTEX_${1}|FUTEX_PRIVATE_FLAG").into_owned())
        .collect();
    let lines: Vec<&str> = normalised.iter().map(String::as_str).collect();

    let mut output = Vec::new();
    let mut parse_errors = 0;
    // The bisection triggers panics on purpose; keep them off stderr.
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let ran: Result<()> = lines
        .chunks(CHUNK)
        .try_for_each(|chunk| run_chunk(&patterns, chunk, &mut output, &mut parse_errors));
    std::panic::set_hook(hook);
    ran?;

    let name = Regex::new(r"^Unsupported syscall: (?:\d+\s+)?([a-z_][a-z0-9_]*)\(").expect("static pattern");
    let mut by_name: BTreeMap<String, Unsupported> = BTreeMap::new();
    for line in output {
        if line.starts_with("Strace Parse Error") {
            parse_errors += 1;
        } else if let Some(syscall) = name.captures(&line).map(|c| c[1].to_owned()) {
            let example: String = line
                .trim_start_matches("Unsupported syscall: ")
                .chars()
                .take(EXAMPLE_LIMIT)
                .collect();
            by_name
                .entry(syscall.clone())
                .and_modify(|u| u.count += 1)
                .or_insert(Unsupported { syscall, count: 1, example });
        }
    }
    let mut out: Vec<_> = by_name.into_values().collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.syscall.cmp(&b.syscall)));
    Ok((out, parse_errors))
}

/// Runs sctrace on `lines`. sctrace panics on a line it cannot classify; the chunk is then halved
/// until the offending line is alone, and that line counts as a parse error. Blocked-call pairing
/// state lives in sctrace's thread-local storage, so the chunks must run on one thread.
fn run_chunk(
    patterns: &Patterns<'_>,
    lines: &[&str],
    output: &mut Vec<String>,
    parse_errors: &mut u64,
) -> Result<()> {
    if lines.is_empty() {
        return Ok(());
    }
    let attempt = catch_unwind(AssertUnwindSafe(|| -> Result<Vec<String>> {
        let stream = StraceLogStream::from_string(&lines.join("\n")).map_err(|e| anyhow!("{e}"))?;
        let tracer = SctraceBuilder::new()
            .strace(stream)
            .patterns(patterns.clone())
            .reporter(CliReporterBuilder::new().quiet().collect().build())
            .build();
        Ok(tracer.run().map_err(|e| anyhow!("sctrace: {e}"))?.unwrap_or_default())
    }));
    match attempt {
        Ok(result) => output.extend(result?),
        Err(_) if lines.len() == 1 => *parse_errors += 1,
        Err(_) => {
            let (head, tail) = lines.split_at(lines.len() / 2);
            run_chunk(patterns, head, output, parse_errors)?;
            run_chunk(patterns, tail, output, parse_errors)?;
        }
    }
    Ok(())
}
