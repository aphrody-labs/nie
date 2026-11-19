//! Syscall audit: runs nie workloads under `strace -f -yy`, matches every call against the
//! System Call Matching Language tables of the Asterinas book (`*.scml`) with `sctrace`, and
//! reports which syscalls a Linux ABI-compatible kernel still has to implement for nie.
//!
//! The workloads come from a policy file; the SCML tables come from a checkout of the
//! Asterinas fork. Nothing about either is written here.

#![cfg(unix)]

mod report;
mod workload;

pub use report::{Environment, Report, Unsupported, WorkloadReport};
pub use workload::{Context, Policy, Ready, Workload};

use std::path::Path;

use anyhow::{Context as _, Result, bail};

/// Every `*.scml` file under `dir`, sorted.
pub fn scml_files(dir: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        for entry in std::fs::read_dir(&current).with_context(|| format!("read {}", current.display()))? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == "scml") {
                found.push(path);
            }
        }
    }
    found.sort();
    if found.is_empty() {
        bail!("no .scml file under {}", dir.display());
    }
    Ok(found)
}

/// Runs every workload of the policy and matches its strace log against the SCML tables.
pub fn audit(scml_dir: &Path, asterinas: &Path, policy: &Policy, ctx: &Context) -> Result<Report> {
    let files = scml_files(scml_dir)?;
    let mut workloads = Vec::new();
    for workload in &policy.workloads {
        let log = tempfile::Builder::new().prefix("nie-strace-").tempfile()?;
        let outcome = workload::trace(workload, ctx, log.path())?;
        workloads.push(report::analyse(workload, ctx, &outcome, log.path(), &files)?);
    }
    Ok(Report::new(asterinas, files.len(), workloads))
}

#[cfg(test)]
mod tests;
