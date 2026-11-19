//! `cargo xtask test`: the CI test job, the whole workspace, or coverage through `cargo llvm-cov`.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};

use super::host::Cmd;
use super::step::{Action, Exec, Runner, Step, run_with};

/// Packages of the CI test job.
pub const CI_PACKAGES: [&str; 4] = ["nie-assets", "nie-formats", "nie-plugin", "nie-abi"];

/// Arguments of `cargo xtask test`.
#[derive(clap::Args, Debug, Clone, Copy, Default)]
pub struct Args {
    #[command(flatten)]
    pub exec: Exec,
    /// Test every workspace member instead of the CI packages.
    #[arg(long)]
    pub workspace: bool,
    /// Measure line and function coverage with `cargo llvm-cov` and print it per crate.
    #[arg(long)]
    pub coverage: bool,
}

fn scope(workspace: bool) -> Vec<String> {
    if workspace {
        vec!["--workspace".into()]
    } else {
        CI_PACKAGES
            .iter()
            .flat_map(|p| ["-p".to_owned(), (*p).to_owned()])
            .collect()
    }
}

/// `cargo test` as CI runs it.
#[must_use]
pub fn test_step(workspace: bool) -> Step {
    let cmd = Cmd::cargo(["test", "--locked"])
        .args(scope(workspace))
        .args(["--lib", "--tests"]);
    Step::build("test", cmd)
}

/// Coverage of the same scope; the JSON summary is read back and summed per crate.
#[must_use]
pub fn coverage_step(workspace: bool, local_json: &Path) -> Step {
    let remote_json = "coverage-summary.json";
    let base = |out: &str| {
        Cmd::cargo(["llvm-cov", "--locked"])
            .args(scope(workspace))
            .args([
                "--lib",
                "--tests",
                "--json",
                "--summary-only",
                "--output-path",
                out,
            ])
    };
    let local_path = local_json.to_path_buf();
    let read_back = local_path.clone();
    let script = format!("{} >&2 && cat {remote_json}", base(remote_json).display());
    Step::build("coverage", Cmd::new("sh").args(["-c", &script]))
        .or_locally(Action::Run(base(&local_path.to_string_lossy())))
        .captured(move |out| {
            let text = if out.trim_start().starts_with('{') {
                out.to_owned()
            } else {
                std::fs::read_to_string(&read_back)
                    .with_context(|| format!("reading {}", read_back.display()))?
            };
            print!("{}", render_coverage(&summarize(&text)?));
            Ok(())
        })
}

/// Line and function counts of a crate.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    /// Lines in the crate.
    pub lines: u64,
    /// Covered lines.
    pub lines_covered: u64,
    /// Functions in the crate.
    pub functions: u64,
    /// Covered functions.
    pub functions_covered: u64,
}

/// The crate a source file belongs to: `crates/<name>/...`, `plugins/<group>/<name>/...` or `xtask/...`.
#[must_use]
pub fn crate_of(filename: &str) -> Option<String> {
    let path = filename.replace('\\', "/");
    let parts: Vec<&str> = path.split('/').collect();
    let Some(at) = parts
        .iter()
        .rposition(|p| *p == "crates" || *p == "plugins")
    else {
        return parts.contains(&"xtask").then(|| "nie-xtask".to_owned());
    };
    match parts[at] {
        "crates" => parts.get(at + 1).map(|s| (*s).to_owned()),
        _ => parts.get(at + 2).map(|s| (*s).to_owned()),
    }
}

/// Sums an `llvm-cov export` summary per crate.
///
/// # Errors
/// When the text is not an llvm-cov JSON export.
pub fn summarize(json: &str) -> Result<BTreeMap<String, Counts>> {
    let value: serde_json::Value = serde_json::from_str(json).context("llvm-cov JSON")?;
    let files = value["data"][0]["files"]
        .as_array()
        .context("no files in the llvm-cov export")?;
    let mut out: BTreeMap<String, Counts> = BTreeMap::new();
    for file in files {
        let Some(name) = file["filename"].as_str().and_then(crate_of) else {
            continue;
        };
        let get = |kind: &str, key: &str| file["summary"][kind][key].as_u64().unwrap_or(0);
        let counts = out.entry(name).or_default();
        counts.lines += get("lines", "count");
        counts.lines_covered += get("lines", "covered");
        counts.functions += get("functions", "count");
        counts.functions_covered += get("functions", "covered");
    }
    Ok(out)
}

fn pct(covered: u64, total: u64) -> f64 {
    if total == 0 {
        100.0
    } else {
        covered as f64 * 100.0 / total as f64
    }
}

/// The per-crate coverage table.
#[must_use]
pub fn render_coverage(by_crate: &BTreeMap<String, Counts>) -> String {
    let width = by_crate.keys().map(String::len).max().unwrap_or(5).max(5);
    let mut out = format!(
        "\n{:<width$}  {:>8}  {:>8}\n",
        "crate", "lines %", "funcs %"
    );
    let mut total = Counts::default();
    for (name, c) in by_crate {
        out.push_str(&format!(
            "{name:<width$}  {:>8.2}  {:>8.2}\n",
            pct(c.lines_covered, c.lines),
            pct(c.functions_covered, c.functions)
        ));
        total.lines += c.lines;
        total.lines_covered += c.lines_covered;
        total.functions += c.functions;
        total.functions_covered += c.functions_covered;
    }
    out.push_str(&format!(
        "{:<width$}  {:>8.2}  {:>8.2}\n",
        "total",
        pct(total.lines_covered, total.lines),
        pct(total.functions_covered, total.functions)
    ));
    out
}

/// Runs `cargo xtask test`.
///
/// # Errors
/// When a step fails.
pub fn run(args: Args) -> Result<()> {
    let mut runner = Runner::new(args.exec.mode());
    let step = if args.coverage {
        let local = runner.root().join("target").join("nie-coverage.json");
        coverage_step(args.workspace, &local)
    } else {
        test_step(args.workspace)
    };
    run_with(&mut runner, vec![step])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_scope_is_the_ci_job() {
        let Action::Run(cmd) = test_step(false).action else {
            panic!("a command");
        };
        assert_eq!(
            cmd.display(),
            "cargo test --locked -p nie-assets -p nie-formats -p nie-plugin -p nie-abi --lib --tests"
        );
        let Action::Run(all) = test_step(true).action else {
            panic!("a command");
        };
        assert_eq!(
            all.display(),
            "cargo test --locked --workspace --lib --tests"
        );
    }

    #[test]
    fn files_map_to_their_crate() {
        assert_eq!(
            crate_of("/srv/x/tree/crates/nie-formats/src/cpk.rs").as_deref(),
            Some("nie-formats")
        );
        assert_eq!(
            crate_of("C:\\src\\nie\\plugins\\exe\\nie-pe\\src\\lib.rs").as_deref(),
            Some("nie-pe")
        );
        assert_eq!(
            crate_of("/srv/x/tree/xtask/src/main.rs").as_deref(),
            Some("nie-xtask")
        );
        assert_eq!(crate_of("/rustc/abc/library/core/src/lib.rs"), None);
    }

    #[test]
    fn summaries_sum_per_crate() {
        let json = r#"{"data":[{"files":[
          {"filename":"/t/crates/a/src/x.rs","summary":{"lines":{"count":10,"covered":5},"functions":{"count":2,"covered":1}}},
          {"filename":"/t/crates/a/src/y.rs","summary":{"lines":{"count":10,"covered":10},"functions":{"count":2,"covered":2}}},
          {"filename":"/t/plugins/g/b/src/z.rs","summary":{"lines":{"count":4,"covered":0},"functions":{"count":1,"covered":0}}}
        ]}]}"#;
        let sums = summarize(json).unwrap();
        assert_eq!(
            sums["a"],
            Counts {
                lines: 20,
                lines_covered: 15,
                functions: 4,
                functions_covered: 3
            }
        );
        assert_eq!(sums["b"].lines, 4);
        let table = render_coverage(&sums);
        assert!(table.contains("75.00"));
        assert!(table.contains("total"));
        assert!(summarize("{}").is_err());
    }
}
