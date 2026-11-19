//! `nie-kernel`: poll kernel.org, decide, record. The build itself is `deploy/kernel/build.sh`.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use nie_kernel::{Announced, Decision, Record, Tag, announced_mainline, decide, last_ok, mainline_tags};
use serde::Serialize;

const RELEASES: &str = "https://www.kernel.org/releases.json";
const LINUX_GIT: &str = "https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git";

#[derive(Parser)]
#[command(name = "nie-kernel", version, about)]
struct Cli {
    /// Directory of `history.jsonl` (what was built, with what outcome).
    #[arg(long, env = "NIE_KERNEL_STATE", default_value = "/var/lib/nie-kernel", global = true)]
    state: PathBuf,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Poll releases.json and the git tag advertisement; print the decision as JSON.
    /// Exit 0 = build this tag, 10 = up to date, 11 = the sources disagree (wait).
    Watch,
    /// Append a pipeline outcome to the history.
    Record {
        #[arg(long)]
        tag: String,
        /// `ok` or `failed`.
        #[arg(long)]
        status: String,
        /// sha256 of the built boot image.
        #[arg(long)]
        bzimage_sha256: Option<String>,
    },
}

#[derive(Serialize)]
struct Report<'a> {
    checked_at: String,
    announced: &'a Announced,
    newest_tag: Option<String>,
    source_status: Option<u16>,
    recorded: Option<String>,
    #[serde(flatten)]
    decision: &'a Decision,
}

fn now_iso() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // civil-from-days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder().user_agent("nie-kernel").timeout(Duration::from_secs(60)).build()?)
}

fn get(client: &reqwest::blocking::Client, url: &str) -> Result<Vec<u8>> {
    let response = client.get(url).send().with_context(|| format!("GET {url}"))?.error_for_status()?;
    Ok(response.bytes()?.to_vec())
}

fn history(state: &std::path::Path) -> String {
    fs::read_to_string(state.join("history.jsonl")).unwrap_or_default()
}

fn watch(state: &std::path::Path) -> Result<ExitCode> {
    let client = client()?;
    let announced = announced_mainline(&get(&client, RELEASES)?)?;
    let advertisement = get(&client, &format!("{LINUX_GIT}/info/refs?service=git-upload-pack"))?;
    let tags = mainline_tags(&advertisement)?;
    let recorded = last_ok(&history(state));
    let decision = decide(&announced, &tags, recorded);
    // Third check: the snapshot tarball that releases.json names answers (the tag is servable, not only listed).
    let source_status = match (&decision, &announced.source) {
        (Decision::Build { .. }, Some(url)) => client.head(url).send().ok().map(|r| r.status().as_u16()),
        _ => None,
    };
    let decision = match (&decision, source_status) {
        (Decision::Build { tag, .. }, Some(code)) if code != 200 => {
            Decision::Wait { reason: format!("snapshot of {tag} answers HTTP {code}") }
        }
        _ => decision,
    };
    let report = Report {
        checked_at: now_iso(),
        announced: &announced,
        newest_tag: tags.iter().map(|t| t.tag).max().map(|t| t.name()),
        source_status,
        recorded: recorded.map(|t| t.name()),
        decision: &decision,
    };
    println!("{}", serde_json::to_string(&report)?);
    Ok(match decision {
        Decision::Build { .. } => ExitCode::from(0),
        Decision::UpToDate { .. } => ExitCode::from(10),
        Decision::Wait { .. } => ExitCode::from(11),
    })
}

fn record(state: &std::path::Path, tag: String, status: String, bzimage_sha256: Option<String>) -> Result<ExitCode> {
    anyhow::ensure!(Tag::parse(&tag).is_some(), "{tag} is not a mainline tag");
    anyhow::ensure!(matches!(status.as_str(), "ok" | "failed"), "status is ok or failed");
    fs::create_dir_all(state)?;
    let line = serde_json::to_string(&Record { tag, status, at: now_iso(), bzimage_sha256 })?;
    let mut file = OpenOptions::new().create(true).append(true).open(state.join("history.jsonl"))?;
    writeln!(file, "{line}")?;
    println!("{line}");
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Cmd::Watch => watch(&cli.state),
        Cmd::Record { tag, status, bzimage_sha256 } => record(&cli.state, tag, status, bzimage_sha256),
    };
    result.unwrap_or_else(|e| {
        eprintln!("nie-kernel: {e:#}");
        ExitCode::FAILURE
    })
}
