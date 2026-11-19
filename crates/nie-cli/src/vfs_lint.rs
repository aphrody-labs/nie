//! `nie vfs lint`: every format parser against the real VFS, read in place.
//!
//! Each file is read, its magic detected and `nie_formats::decode::decode` run on it. Files are
//! grouped by the format that parsed them, or, when nothing parsed, by the format the magic or
//! the file name announces. A format with at least one parsed file has a decoder: its remaining
//! files are parse failures. A format with no parsed file has no standalone decoder (a `.g4mg`
//! needs its sibling `.g4md`): that is work left, not a failure.

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};

use anyhow::{Result, bail};
use nie_formats::vfs::{Vfs, open_source};
use rayon::prelude::*;
use serde::Serialize;

/// Options of `nie vfs lint`.
#[derive(Debug, Clone, Default, clap::Args)]
pub struct VfsOptions {
    /// Game installation, dump or URI (default: `NIE_GAME_SOURCE` and discovery).
    #[arg(long)]
    pub game_dir: Option<String>,
    /// Only paths starting with this prefix.
    #[arg(long)]
    pub prefix: Option<String>,
    /// Sample at most this many files, evenly spaced.
    #[arg(long)]
    pub limit: Option<usize>,
    /// Failure examples listed per format.
    #[arg(long, default_value_t = 10)]
    pub examples: usize,
    /// JSON output.
    #[arg(long)]
    pub json: bool,
    /// Fail when a format with a decoder fails on a file.
    #[arg(long)]
    pub strict: bool,
}

/// Counts for one format.
#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq)]
pub struct FormatStats {
    /// Files grouped under this format.
    pub examined: usize,
    /// Files that parsed.
    pub parsed: usize,
    /// Files that did not.
    pub failed: usize,
    /// Paths of failures.
    pub failures: Vec<String>,
}

/// The whole report.
#[derive(Debug, Default, Clone, Serialize, PartialEq)]
pub struct VfsReport {
    /// `dump` or `packs`.
    pub mount: String,
    /// Files examined.
    pub examined: usize,
    /// Files that could not be read.
    pub unreadable: usize,
    /// Files parsed by some parser.
    pub parsed: usize,
    /// Failures of formats that have a decoder.
    pub failed: usize,
    /// Share of readable files that parsed, in percent.
    pub pct_parsed: f64,
    /// Formats with a decoder, with their counts.
    pub formats: BTreeMap<String, FormatStats>,
    /// Formats announced by magic or name but with no standalone decoder, and their file count.
    pub no_decoder: BTreeMap<String, usize>,
    /// Files no format claims, by extension.
    pub unknown_by_extension: BTreeMap<String, usize>,
}

/// What one file turned out to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The file could not be read.
    Unreadable,
    /// A parser accepted it.
    Parsed(String),
    /// The magic or the name announces a format, but nothing parsed it.
    Failed(String),
    /// No format claims it; the extension.
    Unclaimed(String),
}

/// Evenly spaced sample of at most `limit` items, in input order.
#[must_use]
pub fn sample<T>(items: Vec<T>, limit: Option<usize>) -> Vec<T> {
    match limit {
        Some(n) if n > 0 && n < items.len() => {
            let step = items.len().div_ceil(n);
            items.into_iter().step_by(step).collect()
        }
        _ => items,
    }
}

fn extension(path: &str) -> String {
    let base = path.rsplit('/').next().unwrap_or(path);
    base.rsplit_once('.')
        .map_or_else(|| "<none>".to_owned(), |(_, e)| e.to_lowercase())
}

/// The format a file announces, from its magic or, failing that, its name. The names are those of
/// `nie_formats::decode`: the family of a decoded file and of a failed one must agree.
fn announced(path: &str, bytes: &[u8]) -> Option<String> {
    match nie_formats::decode::format_name(bytes) {
        "inconnu" => {
            let lower = path.to_lowercase();
            if lower.ends_with(".cfg.bin") {
                Some("cfg.bin".into())
            } else if lower.ends_with(".lua.bin") {
                Some("lua-bytecode".into())
            } else {
                None
            }
        }
        "autre" => Some(format!("{:?}", nie_formats::detect(bytes)).to_lowercase()),
        name => Some(name.to_owned()),
    }
}

/// Classifies one file from its path and bytes; a parser panic counts as a failure.
#[must_use]
pub fn classify(path: &str, bytes: &[u8]) -> Outcome {
    let announced = announced(path, bytes);
    match catch_unwind(AssertUnwindSafe(|| nie_formats::decode::decode(bytes))) {
        // "cfg.bin (T2B)" is a cfg.bin: the family is the label before the container note.
        Ok(Some(decoded)) => {
            let family = decoded.format.split(" (").next().unwrap_or(decoded.format);
            Outcome::Parsed(family.to_owned())
        }
        Ok(None) | Err(_) => match announced {
            Some(format) => Outcome::Failed(format),
            None => Outcome::Unclaimed(extension(path)),
        },
    }
}

/// Folds per-file outcomes into a report.
#[must_use]
pub fn summarize(mount: &str, outcomes: Vec<(String, Outcome)>, examples: usize) -> VfsReport {
    let mut report = VfsReport {
        mount: mount.to_owned(),
        examined: outcomes.len(),
        ..VfsReport::default()
    };
    let mut raw: BTreeMap<String, FormatStats> = BTreeMap::new();
    for (path, outcome) in outcomes {
        match outcome {
            Outcome::Unreadable => report.unreadable += 1,
            Outcome::Parsed(format) => {
                let s = raw.entry(format).or_default();
                s.examined += 1;
                s.parsed += 1;
            }
            Outcome::Failed(format) => {
                let s = raw.entry(format).or_default();
                s.examined += 1;
                s.failed += 1;
                if s.failures.len() < examples {
                    s.failures.push(path);
                }
            }
            Outcome::Unclaimed(ext) => *report.unknown_by_extension.entry(ext).or_default() += 1,
        }
    }
    for (format, stats) in raw {
        if stats.parsed == 0 {
            report.no_decoder.insert(format, stats.failed);
        } else {
            report.parsed += stats.parsed;
            report.failed += stats.failed;
            report.formats.insert(format, stats);
        }
    }
    let readable = report.examined - report.unreadable;
    report.pct_parsed = if readable == 0 {
        0.0
    } else {
        report.parsed as f64 * 100.0 / readable as f64
    };
    report
}

/// Lints every selected file of a mounted VFS.
///
/// # Errors
/// When nothing matches the selection.
pub fn lint_vfs(vfs: &Vfs, options: &VfsOptions) -> Result<VfsReport> {
    let mut paths: Vec<&str> = vfs
        .iter()
        .map(|(p, _)| p)
        .filter(|p| {
            options
                .prefix
                .as_deref()
                .is_none_or(|pre| p.starts_with(pre))
        })
        .collect();
    paths.sort_unstable();
    let paths = sample(paths, options.limit);
    if paths.is_empty() {
        bail!("no VFS file matches the selection");
    }
    // A parser that panics is a finding, not a reason to print a backtrace per file.
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let outcomes: Vec<(String, Outcome)> = paths
        .par_iter()
        .map(|path| {
            let outcome = match vfs.read(path) {
                Ok(bytes) => classify(path, &bytes),
                Err(_) => Outcome::Unreadable,
            };
            ((*path).to_owned(), outcome)
        })
        .collect();
    std::panic::set_hook(hook);
    let mount = if vfs.is_dump() { "dump" } else { "packs" };
    Ok(summarize(mount, outcomes, options.examples))
}

/// The human readable report.
#[must_use]
pub fn render(report: &VfsReport) -> String {
    let pct = |n: usize| {
        if report.examined == 0 {
            0.0
        } else {
            n as f64 * 100.0 / report.examined as f64
        }
    };
    let mut out = format!(
        "mount {} | {} files examined\n  parsed      {:>8}  ({:.2} %)\n  parse fail  {:>8}\n  unreadable  {:>8}\n",
        report.mount,
        report.examined,
        report.parsed,
        pct(report.parsed),
        report.failed,
        report.unreadable
    );
    let no_decoder: usize = report.no_decoder.values().sum();
    if no_decoder > 0 {
        let list: Vec<String> = report
            .no_decoder
            .iter()
            .map(|(f, n)| format!("{f} x{n}"))
            .collect();
        out.push_str(&format!(
            "  no standalone decoder {no_decoder:>8}: {}\n",
            list.join(", ")
        ));
    }
    let unknown: usize = report.unknown_by_extension.values().sum();
    if unknown > 0 {
        let mut by: Vec<_> = report.unknown_by_extension.iter().collect();
        by.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let top: Vec<String> = by
            .iter()
            .take(8)
            .map(|(e, n)| format!(".{e} x{n}"))
            .collect();
        out.push_str(&format!("  unknown     {unknown:>8}: {}\n", top.join(", ")));
    }
    out.push_str("\nformat            examined    parsed    failed  parse %\n");
    for (name, s) in &report.formats {
        let p = s.parsed as f64 * 100.0 / s.examined.max(1) as f64;
        out.push_str(&format!(
            "{name:<16}  {:>8}  {:>8}  {:>8}  {p:>6.2}\n",
            s.examined, s.parsed, s.failed
        ));
    }
    for (name, s) in report.formats.iter().filter(|(_, s)| s.failed > 0) {
        out.push_str(&format!("\n{name}: {} failure(s)\n", s.failed));
        for path in &s.failures {
            out.push_str(&format!("  {path}\n"));
        }
    }
    out
}

/// Mounts the VFS and runs the lint.
///
/// # Errors
/// When the VFS cannot be mounted, nothing matches, or `--strict` meets a parse failure.
pub fn run(options: &VfsOptions) -> Result<()> {
    let vfs = open_source(options.game_dir.as_deref())?;
    let report = lint_vfs(&vfs, options)?;
    if options.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", render(&report));
    }
    if options.strict && report.failed > 0 {
        bail!(
            "{} file(s) failed to parse with a format that has a decoder",
            report.failed
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t2b(name: &str) -> Vec<u8> {
        let doc = serde_json::json!({ "entries": [
            { "name": name, "variables": [{ "type": "Int", "value": "7" }], "children": [] }
        ]});
        nie_formats::cfgbin::encode_tagged_t2b(&doc).expect("synthetic T2B")
    }

    #[test]
    fn sampling_is_evenly_spaced() {
        let items: Vec<u32> = (0..100).collect();
        assert_eq!(sample(items.clone(), None).len(), 100);
        let s = sample(items.clone(), Some(10));
        assert_eq!(s.len(), 10);
        assert_eq!(s[1] - s[0], 10);
        assert_eq!(sample(items, Some(1000)).len(), 100);
    }

    #[test]
    fn extensions_are_lowercased() {
        assert_eq!(extension("data/common/a/B.G4MG"), "g4mg");
        assert_eq!(extension("data/common/noext"), "<none>");
    }

    #[test]
    fn summarize_separates_decoders_from_missing_decoders() {
        let outcomes = vec![
            ("a".into(), Outcome::Parsed("cfg.bin".into())),
            ("b".into(), Outcome::Failed("cfg.bin".into())),
            ("c".into(), Outcome::Failed("g4mg".into())),
            ("d".into(), Outcome::Unclaimed("vfxo".into())),
            ("e".into(), Outcome::Unreadable),
        ];
        let r = summarize("dump", outcomes, 5);
        assert_eq!(r.examined, 5);
        assert_eq!((r.parsed, r.failed, r.unreadable), (1, 1, 1));
        assert_eq!(r.formats["cfg.bin"].failures, ["b"]);
        assert_eq!(r.no_decoder["g4mg"], 1);
        assert_eq!(r.unknown_by_extension["vfxo"], 1);
        assert!((r.pct_parsed - 25.0).abs() < 1e-9);
        assert!(render(&r).contains("cfg.bin"));
    }

    #[test]
    fn a_synthetic_vfs_is_linted_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let common = dir.path().join("common");
        std::fs::create_dir_all(&common).unwrap();
        let good = t2b("A_BEG");
        std::fs::write(common.join("good.cfg.bin"), &good).unwrap();
        std::fs::write(common.join("cut.cfg.bin"), &good[..good.len() / 2]).unwrap();
        std::fs::write(common.join("blob.dat"), b"plain bytes").unwrap();
        std::fs::write(common.join("mesh.g4mg"), b"G4MG....").unwrap();
        let mut vfs = Vfs::new();
        vfs.init_loose(dir.path()).unwrap();

        let all = lint_vfs(
            &vfs,
            &VfsOptions {
                examples: 3,
                ..VfsOptions::default()
            },
        )
        .unwrap();
        assert_eq!(all.mount, "dump");
        assert_eq!(all.examined, 4);
        assert_eq!(all.formats["cfg.bin"].parsed, 1);
        assert_eq!(all.formats["cfg.bin"].failed, 1);
        assert!(all.formats["cfg.bin"].failures[0].ends_with("cut.cfg.bin"));
        assert_eq!(all.no_decoder["g4mg"], 1);
        assert_eq!(all.unknown_by_extension["dat"], 1);

        let sampled = lint_vfs(
            &vfs,
            &VfsOptions {
                limit: Some(2),
                ..VfsOptions::default()
            },
        )
        .unwrap();
        assert_eq!(sampled.examined, 2);
        let none = lint_vfs(
            &vfs,
            &VfsOptions {
                prefix: Some("nothing".into()),
                ..VfsOptions::default()
            },
        );
        assert!(none.is_err());
    }
}
