//! `cargo xtask kernel`: the mainline kernel pipeline of the build host.
//!
//! `watch` polls kernel.org (`releases.json` and the git ref advertisement of torvalds/linux.git) and decides;
//! `build source|config|package|repro` verifies the signed tag and builds linux-nie-rc in the Arch container of
//! `deploy/kernel/Dockerfile`; `boot-test` boots the result under QEMU/KVM; `cycle` chains them and records the
//! outcome; `promote` runs on the target host at boot and makes the running kernel the GRUB default once healthy.
//!
//! A build starts only when both sources name the same newest mainline tag and it is not the recorded one.
//! Signature checks happen where the source is fetched: `git verify-tag` against the fingerprints pinned in
//! `deploy/kernel/keys.toml`.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail, ensure};
use clap::{Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::os::{chmod, cpio_newc, find_in_path, gzip, symlink};
use crate::util::{capture, hex, repo_root};

const RELEASES: &str = "https://www.kernel.org/releases.json";
const LINUX_GIT: &str = "https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git";
const IMAGE: &str = "nie-kernel-build";
const DEFAULT_STATE: &str = "/var/lib/nie-kernel";

#[derive(clap::Args)]
pub struct Args {
    /// Directory of `history.jsonl` (what was built, with what outcome); default `/var/lib/nie-kernel`, `<work>/state` for `cycle`.
    #[arg(long, env = "NIE_KERNEL_STATE", global = true)]
    state: Option<PathBuf>,
    /// Working directory of the pipeline (default `$HOME/nie-kernel`).
    #[arg(long, env = "NIE_KERNEL_WORK", global = true)]
    work: Option<PathBuf>,
    /// Directory with the build inputs (Dockerfile, PKGBUILD, keys.toml, ...); default `deploy/kernel` of the checkout.
    #[arg(long, env = "NIE_KERNEL_DIR", global = true)]
    dir: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
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
    /// Build linux-nie-rc from a signed upstream tag, in the container, and compare the boot images.
    Build {
        #[arg(value_enum)]
        step: Step,
        /// Mainline tag, for example `v7.4-rc3`.
        tag: String,
        /// Working directory (default: `--work`).
        workdir: Option<PathBuf>,
        /// Run number of a `package` step (`out-<run>`), default 1.
        run: Option<u32>,
    },
    /// Boot a kernel build under QEMU/KVM with a prd-like CPU, a virtio XFS disk and a module-loading initramfs.
    BootTest {
        /// Boot image (vmlinuz / bzImage).
        kernel: PathBuf,
        /// Modules directory of the release, for example `/lib/modules/7.4.0-rc3-nie-rc`.
        mods: PathBuf,
        /// Scratch directory (default: a new temporary directory).
        workdir: Option<PathBuf>,
        /// Seconds the guest may run.
        #[arg(default_value_t = 180)]
        timeout: u64,
    },
    /// One pipeline run: watch, signed tag, two package builds, same boot image hash, boot test, record.
    Cycle,
    /// Promote the running kernel to the GRUB default once the host is healthy, else fall back.
    Promote,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Source,
    Config,
    Package,
    Repro,
}

pub fn run(args: Args) -> Result<()> {
    let work = args.work.clone().unwrap_or_else(default_work);
    let dir = args
        .dir
        .clone()
        .unwrap_or_else(|| repo_root().join("deploy/kernel"));
    match args.cmd {
        Cmd::Watch => {
            let state = args.state.unwrap_or_else(|| PathBuf::from(DEFAULT_STATE));
            let (json, outcome) = watch(&state)?;
            println!("{json}");
            match outcome.code() {
                0 => Ok(()),
                code => std::process::exit(code),
            }
        }
        Cmd::Record {
            tag,
            status,
            bzimage_sha256,
        } => {
            let state = args.state.unwrap_or_else(|| PathBuf::from(DEFAULT_STATE));
            println!("{}", record(&state, &tag, &status, bzimage_sha256)?);
            Ok(())
        }
        Cmd::Build {
            step,
            tag,
            workdir,
            run,
        } => {
            let build = Build::new(dir, workdir.unwrap_or(work), &tag)?;
            build.step(step, run.unwrap_or(1))
        }
        Cmd::BootTest {
            kernel,
            mods,
            workdir,
            timeout,
        } => {
            println!("{}", boot_test(&kernel, &mods, workdir, timeout)?);
            Ok(())
        }
        Cmd::Cycle => {
            let state = args.state.unwrap_or_else(|| work.join("state"));
            cycle(&state, &work, &dir)
        }
        Cmd::Promote => promote(),
    }
}

fn default_work() -> PathBuf {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .unwrap_or_else(|| ".".into());
    PathBuf::from(home).join("nie-kernel")
}

// ---------------------------------------------------------------------------------------------------------------
// Tags, the two sources and the decision (no network access here: the callers feed the live bytes).

/// A mainline tag: `v7.4-rc3` or `v7.4` (patch releases such as `v7.3.5` belong to stable, not here).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Tag {
    pub major: u32,
    pub minor: u32,
    /// `Some(n)` for `-rcN`, `None` for the final release.
    pub rc: Option<u32>,
}

impl Tag {
    /// Parse `v7.4-rc3`, `7.4-rc3`, `v7.4`. Returns `None` for anything else (stable `7.3.5`, `v2.6.39.4`, `-next`).
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.strip_prefix('v').unwrap_or(text);
        let (base, rc) = match text.split_once("-rc") {
            Some((base, rc)) => (base, Some(rc.parse().ok()?)),
            None => (text, None),
        };
        let (major, minor) = base.split_once('.')?;
        if minor.contains('.') {
            return None;
        }
        Some(Self {
            major: major.parse().ok()?,
            minor: minor.parse().ok()?,
            rc,
        })
    }

    fn key(&self) -> (u32, u32, u32) {
        // A final release sorts after every release candidate of the same version.
        (self.major, self.minor, self.rc.unwrap_or(u32::MAX))
    }

    /// The git tag name, `v7.4-rc3`.
    pub fn name(&self) -> String {
        format!("v{self}")
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.rc {
            Some(rc) => write!(f, "{}.{}-rc{rc}", self.major, self.minor),
            None => write!(f, "{}.{}", self.major, self.minor),
        }
    }
}

impl Ord for Tag {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key().cmp(&other.key())
    }
}

impl PartialOrd for Tag {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Deserialize)]
struct Releases {
    releases: Vec<Release>,
}

#[derive(Deserialize)]
struct Release {
    moniker: String,
    version: String,
    source: Option<String>,
}

/// The mainline entry of `releases.json`: its tag and the snapshot tarball URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Announced {
    pub tag: Tag,
    pub source: Option<String>,
}

/// Read the mainline release out of `https://www.kernel.org/releases.json`.
pub fn announced_mainline(json: &[u8]) -> Result<Announced> {
    let releases: Releases = serde_json::from_slice(json).context("releases.json")?;
    let mainline = releases
        .releases
        .into_iter()
        .find(|r| r.moniker == "mainline")
        .context("releases.json has no mainline entry")?;
    let tag = Tag::parse(&mainline.version).with_context(|| {
        format!(
            "mainline version {:?} is not a mainline tag",
            mainline.version
        )
    })?;
    Ok(Announced {
        tag,
        source: mainline.source,
    })
}

/// A tag of the git ref advertisement: the annotated tag object and the commit it points at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TagRef {
    pub tag: Tag,
    pub tag_oid: String,
    pub commit_oid: Option<String>,
}

fn pkt_lines(mut data: &[u8]) -> Result<Vec<Option<&[u8]>>> {
    let mut lines = Vec::new();
    while !data.is_empty() {
        let len = data.get(..4).context("truncated pkt-line length")?;
        let len =
            usize::from_str_radix(std::str::from_utf8(len)?, 16).context("pkt-line length")?;
        if len == 0 {
            lines.push(None);
            data = &data[4..];
            continue;
        }
        if len < 4 || len > data.len() {
            bail!(
                "pkt-line length {len} out of range ({} bytes left)",
                data.len()
            );
        }
        lines.push(Some(&data[4..len]));
        data = &data[len..];
    }
    Ok(lines)
}

/// Tags of a protocol v0 `info/refs?service=git-upload-pack` response, mainline tags only.
pub fn mainline_tags(advertisement: &[u8]) -> Result<Vec<TagRef>> {
    let mut found: BTreeMap<Tag, TagRef> = BTreeMap::new();
    for line in pkt_lines(advertisement)?.into_iter().flatten() {
        let line = String::from_utf8_lossy(line);
        let line = line.split('\0').next().unwrap_or("").trim_end();
        let Some((oid, name)) = line.split_once(' ') else {
            continue;
        };
        let Some(name) = name.strip_prefix("refs/tags/") else {
            continue;
        };
        let (name, peeled) = match name.strip_suffix("^{}") {
            Some(name) => (name, true),
            None => (name, false),
        };
        if !name.starts_with('v') {
            continue;
        }
        let Some(tag) = Tag::parse(name) else {
            continue;
        };
        let entry = found.entry(tag).or_insert_with(|| TagRef {
            tag,
            tag_oid: String::new(),
            commit_oid: None,
        });
        if peeled {
            entry.commit_oid = Some(oid.to_string());
        } else {
            entry.tag_oid = oid.to_string();
        }
    }
    Ok(found
        .into_values()
        .filter(|t| !t.tag_oid.is_empty())
        .collect())
}

/// What the watcher concluded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Decision {
    /// Both sources name `tag` and it was not built yet.
    Build {
        tag: String,
        tag_oid: String,
        commit_oid: Option<String>,
        source: Option<String>,
    },
    /// Both sources name `tag` and it is the recorded one.
    UpToDate { tag: String },
    /// The sources disagree, or one is empty: nothing starts, the reason says why.
    Wait { reason: String },
}

impl Decision {
    /// Exit code of `watch`: 0 build, 10 up to date, 11 wait.
    fn code(&self) -> i32 {
        match self {
            Decision::Build { .. } => 0,
            Decision::UpToDate { .. } => 10,
            Decision::Wait { .. } => 11,
        }
    }
}

/// Decide from the two sources and the last recorded tag.
pub fn decide(announced: &Announced, tags: &[TagRef], recorded: Option<Tag>) -> Decision {
    let Some(newest) = tags.iter().max_by_key(|t| t.tag) else {
        return Decision::Wait {
            reason: "the git advertisement has no mainline tag".into(),
        };
    };
    if newest.tag != announced.tag {
        return Decision::Wait {
            reason: format!(
                "releases.json announces {} but the newest tag is {}",
                announced.tag, newest.tag
            ),
        };
    }
    if recorded.is_some_and(|r| r >= newest.tag) {
        return Decision::UpToDate {
            tag: newest.tag.name(),
        };
    }
    Decision::Build {
        tag: newest.tag.name(),
        tag_oid: newest.tag_oid.clone(),
        commit_oid: newest.commit_oid.clone(),
        source: announced.source.clone(),
    }
}

/// One recorded outcome of a pipeline run, kept as a JSON line in `<state>/history.jsonl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub tag: String,
    pub status: String,
    pub at: String,
    #[serde(default)]
    pub bzimage_sha256: Option<String>,
}

/// The newest tag among records whose status is `ok`.
pub fn last_ok(history: &str) -> Option<Tag> {
    history
        .lines()
        .filter_map(|l| serde_json::from_str::<Record>(l).ok())
        .filter(|r| r.status == "ok")
        .filter_map(|r| Tag::parse(&r.tag))
        .max()
}

// ---------------------------------------------------------------------------------------------------------------
// watch and record

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
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
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
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn client(agent: &str) -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .user_agent(agent)
        .timeout(Duration::from_secs(60))
        .build()?)
}

fn get(client: &reqwest::blocking::Client, url: &str) -> Result<Vec<u8>> {
    let response = client
        .get(url)
        .send()
        .with_context(|| format!("GET {url}"))?
        .error_for_status()?;
    Ok(response.bytes()?.to_vec())
}

fn history(state: &Path) -> String {
    fs::read_to_string(state.join("history.jsonl")).unwrap_or_default()
}

/// Polls both sources and decides; returns the report line and the decision.
fn watch(state: &Path) -> Result<(String, Decision)> {
    let client = client("nie-kernel")?;
    let announced = announced_mainline(&get(&client, RELEASES)?)?;
    let advertisement = get(
        &client,
        &format!("{LINUX_GIT}/info/refs?service=git-upload-pack"),
    )?;
    let tags = mainline_tags(&advertisement)?;
    let recorded = last_ok(&history(state));
    let decision = decide(&announced, &tags, recorded);
    // Third check: the snapshot tarball that releases.json names answers (the tag is servable, not only listed).
    let source_status = match (&decision, &announced.source) {
        (Decision::Build { .. }, Some(url)) => {
            client.head(url).send().ok().map(|r| r.status().as_u16())
        }
        _ => None,
    };
    let decision = match (&decision, source_status) {
        (Decision::Build { tag, .. }, Some(code)) if code != 200 => Decision::Wait {
            reason: format!("snapshot of {tag} answers HTTP {code}"),
        },
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
    Ok((serde_json::to_string(&report)?, decision))
}

/// Appends an outcome to `<state>/history.jsonl` and returns the written line.
fn record(state: &Path, tag: &str, status: &str, bzimage_sha256: Option<String>) -> Result<String> {
    ensure!(Tag::parse(tag).is_some(), "{tag} is not a mainline tag");
    ensure!(matches!(status, "ok" | "failed"), "status is ok or failed");
    fs::create_dir_all(state)?;
    let line = serde_json::to_string(&Record {
        tag: tag.to_owned(),
        status: status.to_owned(),
        at: now_iso(),
        bzimage_sha256,
    })?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(state.join("history.jsonl"))?;
    writeln!(file, "{line}")?;
    Ok(line)
}

// ---------------------------------------------------------------------------------------------------------------
// build: signed source, container image, package builds

/// Fingerprint of Linus Torvalds and the keyserver base URL, read from `keys.toml`.
fn pinned_key(keys_toml: &str) -> Result<(String, String)> {
    let doc: toml_edit::DocumentMut = keys_toml.parse().context("keys.toml")?;
    let keyserver = doc
        .get("keyserver")
        .and_then(|v| v.as_str())
        .context("keys.toml has no keyserver")?;
    let keys = doc
        .get("key")
        .and_then(|k| k.as_array_of_tables())
        .context("keys.toml has no [[key]]")?;
    let fingerprint = keys
        .iter()
        .find(|k| k.get("name").and_then(|n| n.as_str()) == Some("Linus Torvalds"))
        .and_then(|k| k.get("fingerprint"))
        .and_then(|f| f.as_str())
        .context("keys.toml has no fingerprint for Linus Torvalds")?;
    Ok((keyserver.to_owned(), fingerprint.to_owned()))
}

/// The primary key fingerprint of a `VALIDSIG` status line pair of `git verify-tag --raw`.
fn valid_signer(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let rest = line.strip_prefix("[GNUPG:] VALIDSIG ")?;
        let last = rest.split_whitespace().last()?;
        (last.len() == 40
            && last
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b)))
        .then(|| last.to_owned())
    })
}

/// The first fingerprint of `gpg --with-colons --fingerprint` output.
fn colon_fingerprint(output: &str) -> Option<String> {
    output
        .lines()
        .filter(|l| l.starts_with("fpr:"))
        .find_map(|l| {
            l.split(':')
                .nth(9)
                .filter(|f| !f.is_empty())
                .map(str::to_owned)
        })
}

/// The tagger time (epoch seconds) of a raw annotated tag object.
fn tagger_epoch(tag_object: &str) -> Option<u64> {
    let line = tag_object.lines().find(|l| l.starts_with("tagger "))?;
    let mut words = line.split_whitespace().rev();
    let _zone = words.next()?;
    words.next()?.parse().ok()
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).with_context(|| path.display().to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

fn read_trimmed(path: &Path) -> Result<String> {
    Ok(fs::read_to_string(path)
        .with_context(|| path.display().to_string())?
        .trim_end()
        .to_owned())
}

struct Build {
    /// Directory with the Dockerfile, PKGBUILD, keys.toml and kernel fragments.
    here: PathBuf,
    work: PathBuf,
    tag: String,
    /// `<work>/<tag>`.
    dir: PathBuf,
    jobs: usize,
}

impl Build {
    fn new(here: PathBuf, work: PathBuf, tag: &str) -> Result<Self> {
        ensure!(Tag::parse(tag).is_some(), "{tag} is not a mainline tag");
        let jobs = std::env::var("NIE_KERNEL_JOBS")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or_else(|| {
                std::thread::available_parallelism()
                    .map_or(2, |n| n.get() / 2)
                    .max(1)
            });
        let dir = work.join(tag);
        Ok(Self {
            here,
            work,
            tag: tag.to_owned(),
            dir,
            jobs,
        })
    }

    fn step(&self, step: Step, run: u32) -> Result<()> {
        match step {
            Step::Source => self.source_tree(),
            Step::Config => self.config(),
            Step::Package => {
                self.image()?;
                self.run_in(&self.dir.join(format!("out-{run}")), MAKEPKG)
            }
            Step::Repro => {
                self.image()?;
                let (one, two) = (self.dir.join("out-1"), self.dir.join("out-2"));
                std::thread::scope(|scope| {
                    let first = scope.spawn(|| self.run_in(&one, MAKEPKG));
                    let second = scope.spawn(|| self.run_in(&two, MAKEPKG));
                    let first = first
                        .join()
                        .map_err(|_| anyhow::anyhow!("build 1 panicked"))?;
                    let second = second
                        .join()
                        .map_err(|_| anyhow::anyhow!("build 2 panicked"))?;
                    first.and(second)
                })
            }
        }
    }

    /// Fetches the tag into a bare repository, verifies the signature against the pinned fingerprint and
    /// archives the tree.
    fn source_tree(&self) -> Result<()> {
        let tag = &self.tag;
        let (keyserver, linus) =
            pinned_key(&fs::read_to_string(self.here.join("keys.toml")).context("keys.toml")?)?;
        let gnupg = self.work.join("gnupg");
        fs::create_dir_all(&self.dir)?;
        fs::create_dir_all(&gnupg)?;
        chmod(&gnupg, 0o700)?;
        let bare = self.work.join("linux.git");
        if !bare.is_dir() {
            capture(
                Command::new("git")
                    .args(["init", "--bare", "-q"])
                    .arg(&bare),
            )?;
        }
        capture(
            Command::new("git")
                .arg("-C")
                .arg(&bare)
                .args(["fetch", "-q", "--depth", "1", "--no-tags", LINUX_GIT])
                .arg(format!("refs/tags/{tag}:refs/tags/{tag}")),
        )?;

        let long_id = linus
            .get(24..)
            .context("the pinned fingerprint is shorter than 40 characters")?;
        let key = get(&client("nie")?, &format!("{keyserver}/{long_id}.asc"))?;
        let mut import = Command::new("gpg")
            .args(["--quiet", "--import"])
            .env("GNUPGHOME", &gnupg)
            .stdin(Stdio::piped())
            .spawn()
            .context("spawn gpg")?;
        import.stdin.take().context("gpg stdin")?.write_all(&key)?;
        ensure!(import.wait()?.success(), "gpg --import failed");
        let colons = capture(
            Command::new("gpg")
                .args(["--with-colons", "--fingerprint", &linus])
                .env("GNUPGHOME", &gnupg),
        )?;
        let got = colon_fingerprint(&colons).unwrap_or_default();
        ensure!(
            got == linus,
            "key fingerprint {got} is not the pinned {linus}"
        );

        let verify = Command::new("git")
            .arg("-C")
            .arg(&bare)
            .args(["verify-tag", "--raw", tag])
            .env("GNUPGHOME", &gnupg)
            .output()
            .context("spawn git verify-tag")?;
        let verified = format!(
            "{}{}",
            String::from_utf8_lossy(&verify.stderr),
            String::from_utf8_lossy(&verify.stdout)
        );
        let signer = valid_signer(&verified).unwrap_or_default();
        ensure!(
            signer == linus,
            "tag {tag} is not signed by {linus} (got '{signer}')"
        );

        let object = capture(
            Command::new("git")
                .arg("-C")
                .arg(&bare)
                .args(["cat-file", "tag", tag]),
        )?;
        let epoch =
            tagger_epoch(&object).with_context(|| format!("tag {tag} has no tagger time"))?;
        fs::write(self.dir.join("epoch"), format!("{epoch}\n"))?;
        let version = tag.strip_prefix('v').unwrap_or(tag);
        let tar = self.dir.join("linux.tar");
        capture(
            Command::new("git")
                .arg("-C")
                .arg(&bare)
                .args(["archive", "--format=tar"])
                .arg(format!("--prefix=linux-{version}/"))
                .arg("-o")
                .arg(&tar)
                .arg(tag),
        )?;
        let sha = sha256_file(&tar)?;
        fs::write(self.dir.join("linux.tar.sha256"), format!("{sha}\n"))?;
        let oids = capture(Command::new("git").arg("-C").arg(&bare).args([
            "rev-parse",
            tag,
            &format!("{tag}^{{commit}}"),
        ]))?;
        fs::write(self.dir.join("tag.oid"), format!("{oids}\n"))?;
        println!("tag {tag} verified: signer {signer}, tagger epoch {epoch}, tar sha256 {sha}");
        Ok(())
    }

    /// Builds the container image once and records its id and package set.
    fn image(&self) -> Result<()> {
        fs::create_dir_all(&self.work)?;
        let id = capture(
            Command::new("docker")
                .args(["build", "-q", "-t", IMAGE])
                .arg(&self.here),
        )?;
        fs::write(self.work.join("image.id"), format!("{id}\n"))?;
        let packages = capture(Command::new("docker").args([
            "run",
            "--rm",
            IMAGE,
            "sh",
            "-c",
            "pacman -Q | sort",
        ]))?;
        let digest = hex(&Sha256::digest(format!("{packages}\n").as_bytes()));
        fs::write(self.work.join("image.pkgs.sha256"), format!("{digest}\n"))?;
        println!("image {id} package set {digest}");
        Ok(())
    }

    /// Regenerates `prd.config` inputs from the raw `/proc/config.gz` of prd (standard input) for this tag.
    fn config(&self) -> Result<()> {
        self.image()?;
        let mut raw = Vec::new();
        std::io::stdin().read_to_end(&mut raw)?;
        fs::write(self.dir.join("raw.config"), raw)?;
        let cfg = self.dir.join("cfg");
        fs::create_dir_all(&cfg)?;
        chmod(&cfg, 0o777)?;
        let script = "tar -xf linux.tar && cd linux-* && cp ../raw.config .config && make olddefconfig >/dev/null && scripts/kconfig/merge_config.sh -m -O . .config /dev/null >/dev/null 2>&1; make olddefconfig >/dev/null\n      cp .config /out/olddefconfig.config; scripts/diffconfig ../raw.config .config > /out/diffconfig.txt; make -s rustavailable > /out/rust.txt 2>&1 || true";
        crate::util::run(
            Command::new("docker")
                .args(["run", "--rm", "--network", "none", "-v"])
                .arg(format!(
                    "{}:/build/linux.tar:ro",
                    self.dir.join("linux.tar").display()
                ))
                .arg("-v")
                .arg(format!(
                    "{}:/build/raw.config:ro",
                    self.dir.join("raw.config").display()
                ))
                .arg("-v")
                .arg(format!("{}:/out", cfg.display()))
                .args([IMAGE, "sh", "-c", script]),
        )
    }

    /// Runs a command in the image with the network off and the build inputs mounted read-only.
    fn run_in(&self, out: &Path, script: &str) -> Result<()> {
        fs::create_dir_all(out)?;
        chmod(out, 0o777)?;
        let sha = read_trimmed(&self.dir.join("linux.tar.sha256"))?;
        let epoch = read_trimmed(&self.dir.join("epoch"))?;
        let mount = |name: &str| format!("{}:/build/{name}:ro", self.here.join(name).display());
        crate::util::run(
            Command::new("docker")
                .args(["run", "--rm", "--network", "none", "--cpus"])
                .arg(self.jobs.to_string())
                .arg("-e")
                .arg(format!("NIE_KERNEL_TAG={}", self.tag))
                .arg("-e")
                .arg(format!("NIE_KERNEL_SHA256={sha}"))
                .arg("-e")
                .arg(format!("SOURCE_DATE_EPOCH={epoch}"))
                .args(["-e", "PKGDEST=/out", "-e"])
                .arg(format!("MAKEFLAGS=-j{}", self.jobs))
                .arg("-v")
                .arg(mount("PKGBUILD"))
                .arg("-v")
                .arg(mount("prd.config"))
                .arg("-v")
                .arg(mount("nie-rc.fragment"))
                .arg("-v")
                .arg(mount("linux-nie-rc.preset"))
                .arg("-v")
                .arg(format!(
                    "{}:/build/linux.tar:ro",
                    self.dir.join("linux.tar").display()
                ))
                .arg("-v")
                .arg(format!("{}:/out", out.display()))
                .args([IMAGE, "sh", "-c", script]),
        )
    }
}

/// The package build, run inside the container.
const MAKEPKG: &str = "cd /build && makepkg -f --nocheck --noconfirm --skippgpcheck > /out/makepkg.log 2>&1; echo \"exit $?\" > /out/exit";

// ---------------------------------------------------------------------------------------------------------------
// boot-test

/// Library paths printed by `ldd`: `name => /path (0x...)` lines and the `/lib.../ld-linux...` loader line.
fn ldd_libs(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| {
            if let Some(at) = line.rfind("=> /") {
                let rest = &line[at + 3..];
                let (path, _) = rest.split_once(' ')?;
                return Some(path.to_owned());
            }
            let trimmed = line.trim_start();
            let (path, _) = trimmed.split_once(' ')?;
            (path.starts_with("/lib") && path.contains("ld-linux")).then(|| path.to_owned())
        })
        .collect()
}

/// Names from a `modinfo -F depends` line: `a,b` -> `[a, b]`.
fn module_deps(depends: &str) -> Vec<String> {
    depends
        .split(',')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Runs `modinfo` against the module tree; `None` when it fails.
fn modinfo(base: &Path, release: &str, flags: &[&str], module: &str) -> Option<String> {
    let out = Command::new("modinfo")
        .arg("-b")
        .arg(base)
        .args(["-k", release])
        .args(flags)
        .arg(module)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// Collects `module` and its dependencies, dependencies first, as plain `.ko` files in `dest`.
fn add_module(
    base: &Path,
    release: &str,
    dest: &Path,
    module: &str,
    order: &mut Vec<String>,
    seen: &mut Vec<String>,
) -> Result<()> {
    if seen.iter().any(|m| m == module) {
        return Ok(());
    }
    seen.push(module.to_owned());
    if let Some(depends) = modinfo(base, release, &["-F", "depends"], module) {
        for dep in module_deps(&depends) {
            add_module(base, release, dest, &dep, order, seen)?;
        }
    }
    let Some(path) = modinfo(base, release, &["-n"], module) else {
        return Ok(());
    };
    let target = dest.join(format!("{module}.ko"));
    if path.ends_with(".zst") {
        let out = capture_bytes(Command::new("zstd").args(["-dqc", &path]))?;
        fs::write(&target, out)?;
    } else if path.ends_with(".xz") {
        let out = capture_bytes(Command::new("xz").args(["-dc", &path]))?;
        fs::write(&target, out)?;
    } else if path.ends_with(".ko") {
        fs::copy(&path, &target)?;
    } else {
        return Ok(());
    }
    order.push(module.to_owned());
    Ok(())
}

fn capture_bytes(cmd: &mut Command) -> Result<Vec<u8>> {
    let out = cmd.output().with_context(|| format!("spawn {cmd:?}"))?;
    ensure!(out.status.success(), "{cmd:?} failed ({})", out.status);
    Ok(out.stdout)
}

/// Runs QEMU with a deadline; the guest output goes to the serial log file.
fn qemu_run(cpu: &str, kernel: &Path, work: &Path, limit: u64) -> Result<()> {
    let log = fs::File::create(work.join("qemu.out"))?;
    let mut child = Command::new("qemu-system-x86_64")
        .args([
            "-enable-kvm",
            "-cpu",
            cpu,
            "-smp",
            "4",
            "-m",
            "2G",
            "-nographic",
            "-no-reboot",
            "-kernel",
        ])
        .arg(kernel)
        .arg("-initrd")
        .arg(work.join("initrd.gz"))
        .args(["-append", "console=ttyS0 panic=-1 loglevel=4", "-drive"])
        .arg(format!(
            "file={},if=virtio,format=raw",
            work.join("xfs.img").display()
        ))
        .arg("-serial")
        .arg(format!("file:{}", work.join("serial.log").display()))
        .args(["-monitor", "none"])
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()
        .context("spawn qemu-system-x86_64")?;
    let deadline = Instant::now() + Duration::from_secs(limit);
    loop {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn tail(path: &Path, lines: usize) -> String {
    let text = fs::read_to_string(path).unwrap_or_default();
    let all: Vec<&str> = text.lines().collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

/// Boots a kernel build in QEMU/KVM with a prd-like CPU model, a virtio disk holding an XFS image and an
/// initramfs that loads the XFS module, mounts the disk read-only, reads a marker file and reports the io_uring
/// sysctl. Passes when the serial console shows the `NIE-BOOT-OK` line with the marker. Returns the PASS lines.
fn boot_test(kernel: &Path, mods: &Path, workdir: Option<PathBuf>, limit: u64) -> Result<String> {
    let release = mods
        .file_name()
        .context("modules directory has no name")?
        .to_string_lossy()
        .into_owned();
    let work = workdir.unwrap_or_else(|| {
        std::env::temp_dir().join(format!("nie-boot-test-{}", std::process::id()))
    });
    let root = work.join("root");
    for dir in ["bin", "modules", "dev", "proc", "sys", "mnt", "lib64"] {
        fs::create_dir_all(root.join(dir))?;
    }

    // busybox with the libraries it needs (the build host ships it dynamically linked)
    let busybox = find_in_path(&["busybox"]).context("busybox is not on PATH")?;
    fs::copy(&busybox, root.join("bin/busybox"))?;
    chmod(&root.join("bin/busybox"), 0o755)?;
    let ldd = Command::new("ldd")
        .arg(&busybox)
        .stderr(Stdio::null())
        .output()
        .ok();
    for lib in ldd
        .map(|o| ldd_libs(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
    {
        let target = root.join(lib.trim_start_matches('/'));
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&lib, &target).with_context(|| format!("copy {lib}"))?;
    }
    for applet in [
        "sh", "mount", "insmod", "cat", "echo", "poweroff", "sleep", "ls", "mkdir",
    ] {
        symlink("busybox", &root.join("bin").join(applet))?;
    }

    // xfs and its module dependencies, in load order (modules that are built in are skipped)
    let base = mods.join("../../..");
    if !mods.join("modules.dep").exists() {
        crate::util::run(Command::new("depmod").arg("-b").arg(&base).arg(&release))?;
    }
    let (mut order, mut seen) = (Vec::new(), Vec::new());
    add_module(
        &base,
        &release,
        &root.join("modules"),
        "xfs",
        &mut order,
        &mut seen,
    )?;

    let init = format!(
        "#!/bin/sh\n\
         mount -t proc proc /proc; mount -t sysfs sys /sys; mount -t devtmpfs dev /dev\n\
         for m in {order}; do insmod /modules/$m.ko || echo \"NIE-BOOT-MODULE-FAIL $m\"; done\n\
         mount -t xfs -o ro /dev/vda /mnt || echo \"NIE-BOOT-MOUNT-FAIL\"\n\
         echo \"NIE-BOOT-OK release=$(cat /proc/sys/kernel/osrelease) marker=$(cat /mnt/marker) io_uring_disabled=$(cat /proc/sys/kernel/io_uring_disabled 2>/dev/null)\"\n\
         poweroff -f\n",
        order = order.join(" ")
    );
    fs::write(root.join("init"), init)?;
    chmod(&root.join("init"), 0o755)?;
    fs::write(work.join("initrd.gz"), gzip(&cpio_newc(&root)?, 1)?)?;

    // XFS image with a marker, no mount needed (mkfs.xfs -p)
    let marker = work.join("marker");
    fs::write(&marker, format!("nie-boot-marker-{release}\n"))?;
    fs::write(
        work.join("proto"),
        format!(
            "bootmarker\n0 0\nd--755 0 0\n marker ---644 0 0 {}\n$\n",
            marker.display()
        ),
    )?;
    fs::File::create(work.join("xfs.img"))?.set_len(512 << 20)?;
    capture(
        Command::new("mkfs.xfs")
            .args(["-q", "-f", "-L", "niegame", "-p"])
            .arg(work.join("proto"))
            .arg(work.join("xfs.img")),
    )?;

    let serial = work.join("serial.log");
    fs::write(&serial, "")?;
    let mut cpu = "EPYC-Genoa";
    qemu_run(cpu, kernel, &work, limit)?;
    if !fs::read_to_string(&serial)
        .unwrap_or_default()
        .contains("NIE-BOOT-OK")
    {
        cpu = "host";
        fs::write(&serial, "")?;
        qemu_run(cpu, kernel, &work, limit)?;
    }
    let log = fs::read_to_string(&serial).unwrap_or_default();
    let expected = format!("NIE-BOOT-OK release={release} marker=nie-boot-marker-{release}");
    if log.contains(&expected) {
        let line = log
            .lines()
            .find(|l| l.contains("NIE-BOOT-OK"))
            .unwrap_or_default()
            .replace('\r', "");
        return Ok(format!(
            "PASS cpu={cpu} {line}\nkernel sha256 {}",
            sha256_file(kernel)?
        ));
    }
    println!("FAIL (serial log: {})", serial.display());
    println!("{}", tail(&serial, 20));
    println!("{}", tail(&work.join("qemu.out"), 5));
    bail!("boot test failed")
}

// ---------------------------------------------------------------------------------------------------------------
// cycle

/// The first `linux-nie-rc-<digit>*.pkg.tar.zst` of a directory, in name order.
fn first_package(dir: &Path) -> Result<PathBuf> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .with_context(|| dir.display().to_string())?
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            n.strip_prefix("linux-nie-rc-")
                .is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit()))
                && n.ends_with(".pkg.tar.zst")
        })
        .collect();
    names.sort();
    let name = names
        .into_iter()
        .next()
        .with_context(|| format!("no linux-nie-rc package in {}", dir.display()))?;
    Ok(dir.join(name))
}

/// sha256 of the boot image inside a package.
fn image_sha(dir: &Path, run: u32) -> Result<String> {
    let package = first_package(&dir.join(format!("out-{run}")))?;
    let bytes = capture_bytes(
        Command::new("tar")
            .args(["--zstd", "-xOf"])
            .arg(&package)
            .args(["--wildcards", "usr/lib/modules/*/vmlinuz"]),
    )?;
    Ok(hex(&Sha256::digest(&bytes)))
}

/// One pipeline run on the build host: watch -> signed tag -> two package builds -> same boot image hash ->
/// QEMU boot test -> record. Nothing to do, or a recorded `ok`, is success; the new package is left in
/// `<work>/<tag>/out-1` for the prd rollout (`pacman -U`, `grub-mkconfig`, `grub-reboot`).
fn cycle(state: &Path, work: &Path, here: &Path) -> Result<()> {
    let (json, decision) = watch(state)?;
    println!("{json}");
    let Decision::Build { tag, .. } = decision else {
        return Ok(());
    };
    let build = Build::new(here.to_path_buf(), work.to_path_buf(), &tag)?;
    match pipeline(&build) {
        Ok((package, sha)) => {
            println!("{}", record(state, &tag, "ok", Some(sha.clone()))?);
            println!("ready: {} (boot image sha256 {sha})", package.display());
            Ok(())
        }
        Err(e) => {
            record(state, &tag, "failed", None)?;
            eprintln!("pipeline failed: {e:#}");
            bail!("pipeline failed for {tag}")
        }
    }
}

fn pipeline(build: &Build) -> Result<(PathBuf, String)> {
    let dir = &build.dir;
    build.step(Step::Source, 1).context("tag verification")?;
    for run in 1..=2 {
        build
            .step(Step::Package, run)
            .with_context(|| format!("package build {run}"))?;
        let out = dir.join(format!("out-{run}"));
        let exit = fs::read_to_string(out.join("exit")).unwrap_or_default();
        if exit.trim_end() != "exit 0" {
            let log = tail(&out.join("makepkg.log"), 3);
            bail!("makepkg {run}: {log}");
        }
    }
    let (one, two) = (image_sha(dir, 1)?, image_sha(dir, 2)?);
    ensure!(one == two, "boot image differs between builds: {one} {two}");

    let package = first_package(&dir.join("out-1"))?;
    let extract = dir.join("extract");
    let _ = fs::remove_dir_all(&extract);
    fs::create_dir_all(&extract)?;
    capture(
        Command::new("tar")
            .args(["--zstd", "-xf"])
            .arg(&package)
            .arg("-C")
            .arg(&extract),
    )?;
    let modules = extract.join("usr/lib/modules");
    let mut releases: Vec<String> = fs::read_dir(&modules)?
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    releases.sort();
    let release = releases
        .into_iter()
        .next()
        .context("the package has no modules directory")?;
    let report = boot_test(
        &modules.join(&release).join("vmlinuz"),
        &modules.join(&release),
        Some(dir.join("boot-test")),
        180,
    )
    .context("boot test")?;
    println!("{report}");
    Ok((package, one))
}

// ---------------------------------------------------------------------------------------------------------------
// promote

/// The GRUB entry of the running kernel, from `/proc/cmdline`: `gnulinux-<image name>-advanced-<root uuid>`.
fn running_entry(cmdline: &str) -> Option<String> {
    let word = |key: &str| {
        cmdline
            .split_whitespace()
            .find_map(|w| w.strip_prefix(key))
            .map(str::to_owned)
    };
    let image = word("BOOT_IMAGE=")?;
    let root = word("root=UUID=")?;
    let name = image.rsplit('/').next().unwrap_or(&image);
    let name = name.strip_prefix("vmlinuz-").unwrap_or(name);
    (!name.is_empty() && !root.is_empty()).then(|| format!("gnulinux-{name}-advanced-{root}"))
}

fn log_line(message: &str) {
    let _ = Command::new("logger")
        .args([
            "-t",
            "nie-kernel-promote",
            "-p",
            "daemon.notice",
            "--",
            message,
        ])
        .status();
    println!("{message}");
}

/// Entries of the required list (`unit <name>` or `mount <path>` per line) that are not satisfied.
fn missing(conf: &str) -> Vec<String> {
    let quiet = |cmd: &mut Command| {
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    };
    conf.lines()
        .filter_map(|line| {
            let line = line.trim();
            let (kind, arg) = line.split_once(char::is_whitespace)?;
            let arg = arg.trim();
            let ok = match kind {
                "unit" => quiet(Command::new("systemctl").args(["is-active", "--quiet", arg])),
                "mount" => quiet(Command::new("findmnt").args(["--mountpoint", arg])),
                _ => return None,
            };
            (!ok).then(|| format!("{kind} {arg}"))
        })
        .collect()
}

/// Promote the running kernel to the GRUB default once the host is healthy.
///
/// Flow: `grub-reboot <entry>` boots a new kernel once; this runs at boot, and only when the running kernel is not
/// the saved default. Healthy (required units active, required mounts present) -> `grub-set-default <running entry>`.
/// Not healthy within the timeout -> the saved default stays and the host reboots into it. The one-shot `next_entry`
/// is already cleared by GRUB, so the next boot is the saved fallback and there is no loop.
/// Config: `NIE_PROMOTE_CONF` (default `/etc/kernel-promote/required`), `NIE_PROMOTE_TIMEOUT` seconds (default 300).
fn promote() -> Result<()> {
    let conf_path = std::env::var("NIE_PROMOTE_CONF")
        .unwrap_or_else(|_| "/etc/kernel-promote/required".to_owned());
    let timeout: u64 = std::env::var("NIE_PROMOTE_TIMEOUT")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(300);
    let cmdline = fs::read_to_string("/proc/cmdline").unwrap_or_default();
    let Some(running) = running_entry(&cmdline) else {
        log_line("cannot derive the running entry from /proc/cmdline");
        return Ok(());
    };
    let env = capture(Command::new("grub-editenv").arg("list"))?;
    let saved = env
        .lines()
        .find_map(|l| l.strip_prefix("saved_entry="))
        .unwrap_or_default()
        .to_owned();
    if running == saved {
        log_line(&format!("running {running} is the saved default"));
        return Ok(());
    }
    let release = read_trimmed(Path::new("/proc/sys/kernel/osrelease")).unwrap_or_default();
    let deadline = Instant::now() + Duration::from_secs(timeout);
    let bad = loop {
        let conf = fs::read_to_string(&conf_path).with_context(|| conf_path.clone())?;
        let bad = missing(&conf);
        if bad.is_empty() {
            capture(Command::new("grub-set-default").arg(&running))?;
            log_line(&format!(
                "healthy on {release}: saved default {saved} -> {running}"
            ));
            return Ok(());
        }
        if Instant::now() >= deadline {
            break bad;
        }
        std::thread::sleep(Duration::from_secs(5));
    };
    log_line(&format!(
        "unhealthy on {release} after {timeout}s ({}): rebooting into saved default {saved}",
        bad.join(" ")
    ));
    crate::util::run(Command::new("systemctl").args(["--no-block", "reboot"]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkt(s: &str) -> Vec<u8> {
        format!("{:04x}{s}", s.len() + 4).into_bytes()
    }

    fn advertisement() -> Vec<u8> {
        let mut out = pkt("# service=git-upload-pack\n");
        out.extend_from_slice(b"0000");
        out.extend(pkt(
            "1111111111111111111111111111111111111111 HEAD\0multi_ack thin-pack\n",
        ));
        for (oid, name) in [
            ("2222222222222222222222222222222222222222", "refs/tags/v7.3"),
            (
                "3333333333333333333333333333333333333333",
                "refs/tags/v7.3^{}",
            ),
            (
                "4444444444444444444444444444444444444444",
                "refs/tags/v7.4-rc2",
            ),
            (
                "5555555555555555555555555555555555555555",
                "refs/tags/v7.4-rc3",
            ),
            (
                "6666666666666666666666666666666666666666",
                "refs/tags/v7.4-rc3^{}",
            ),
            (
                "7777777777777777777777777777777777777777",
                "refs/tags/v7.3.5",
            ),
            (
                "8888888888888888888888888888888888888888",
                "refs/tags/v2.6.39-rc1",
            ),
            (
                "9999999999999999999999999999999999999999",
                "refs/tags/v7.4-rc10",
            ),
            (
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "refs/tags/next-20261118",
            ),
        ] {
            out.extend(pkt(&format!("{oid} {name}\n")));
        }
        out.extend_from_slice(b"0000");
        out
    }

    #[test]
    fn release_candidates_sort_below_the_final_and_numerically() {
        let t = |s| Tag::parse(s).unwrap();
        assert!(t("v7.4-rc3") < t("v7.4-rc10"));
        assert!(t("v7.4-rc10") < t("v7.4"));
        assert!(t("v7.4") < t("v7.5-rc1"));
        assert!(t("v7.9") < t("v7.10-rc1"));
        assert_eq!(t("7.4-rc3").name(), "v7.4-rc3");
        assert_eq!(t("v7.4").to_string(), "7.4");
    }

    #[test]
    fn stable_and_legacy_names_are_not_mainline_tags() {
        for s in [
            "7.3.5",
            "v7.3.5",
            "next-20261118",
            "v7.4-rc",
            "v7",
            "v2.6.39.4",
            "v7.4-rcx",
        ] {
            assert_eq!(Tag::parse(s), None, "{s}");
        }
    }

    #[test]
    fn ref_advertisement_gives_tags_with_their_commit() {
        let tags = mainline_tags(&advertisement()).unwrap();
        let rc3 = tags
            .iter()
            .find(|t| t.tag == Tag::parse("v7.4-rc3").unwrap())
            .unwrap();
        assert_eq!(rc3.tag_oid, "5555555555555555555555555555555555555555");
        assert_eq!(
            rc3.commit_oid.as_deref(),
            Some("6666666666666666666666666666666666666666")
        );
        assert_eq!(
            tags.iter().map(|t| t.tag).max().unwrap(),
            Tag::parse("v7.4-rc10").unwrap()
        );
        assert!(tags.iter().all(|t| t.tag.major >= 2));
    }

    #[test]
    fn truncated_advertisement_is_an_error() {
        assert!(mainline_tags(b"00ff# service").is_err());
        assert!(mainline_tags(b"zz").is_err());
    }

    #[test]
    fn mainline_entry_of_releases_json() {
        let json = br#"{"releases":[{"moniker":"mainline","version":"7.4-rc3","source":"https://git.kernel.org/torvalds/t/linux-7.4-rc3.tar.gz"},
            {"moniker":"stable","version":"7.3.5","source":"x"}]}"#;
        let a = announced_mainline(json).unwrap();
        assert_eq!(a.tag, Tag::parse("v7.4-rc3").unwrap());
        assert!(a.source.unwrap().ends_with("linux-7.4-rc3.tar.gz"));
        assert!(
            announced_mainline(br#"{"releases":[{"moniker":"stable","version":"7.3.5"}]}"#)
                .is_err()
        );
    }

    fn refs(names: &[&str]) -> Vec<TagRef> {
        names
            .iter()
            .map(|n| TagRef {
                tag: Tag::parse(n).unwrap(),
                tag_oid: "ab".repeat(20),
                commit_oid: None,
            })
            .collect()
    }

    fn announced(v: &str) -> Announced {
        Announced {
            tag: Tag::parse(v).unwrap(),
            source: None,
        }
    }

    #[test]
    fn decision_builds_only_when_both_sources_agree_and_the_tag_is_new() {
        let tags = refs(&["v7.4-rc2", "v7.4-rc3"]);
        assert!(
            matches!(decide(&announced("7.4-rc3"), &tags, None), Decision::Build { tag, .. } if tag == "v7.4-rc3")
        );
        assert!(matches!(
            decide(&announced("7.4-rc3"), &tags, Tag::parse("v7.4-rc2")),
            Decision::Build { .. }
        ));
        assert_eq!(
            decide(&announced("7.4-rc3"), &tags, Tag::parse("v7.4-rc3")),
            Decision::UpToDate {
                tag: "v7.4-rc3".into()
            }
        );
        assert!(matches!(
            decide(&announced("7.4-rc2"), &tags, None),
            Decision::Wait { .. }
        ));
        assert!(matches!(
            decide(&announced("7.4-rc3"), &[], None),
            Decision::Wait { .. }
        ));
    }

    #[test]
    fn the_decision_maps_to_the_exit_codes_of_watch() {
        let tags = refs(&["v7.4-rc3"]);
        assert_eq!(decide(&announced("7.4-rc3"), &tags, None).code(), 0);
        assert_eq!(
            decide(&announced("7.4-rc3"), &tags, Tag::parse("v7.4-rc3")).code(),
            10
        );
        assert_eq!(decide(&announced("7.4-rc2"), &tags, None).code(), 11);
    }

    #[test]
    fn history_keeps_the_newest_successful_tag() {
        let history = concat!(
            "{\"tag\":\"v7.4-rc2\",\"status\":\"ok\",\"at\":\"2026-11-12T00:00:00Z\"}\n",
            "{\"tag\":\"v7.4-rc3\",\"status\":\"failed\",\"at\":\"2026-11-19T00:00:00Z\"}\n",
            "not json\n"
        );
        assert_eq!(last_ok(history), Tag::parse("v7.4-rc2"));
        assert_eq!(last_ok(""), None);
    }

    #[test]
    fn records_append_to_the_history_and_validate_their_input() {
        let dir = tempfile::tempdir().unwrap();
        assert!(record(dir.path(), "7.3.5", "ok", None).is_err());
        assert!(record(dir.path(), "v7.4-rc3", "maybe", None).is_err());
        record(dir.path(), "v7.4-rc2", "ok", Some("ab".into())).unwrap();
        record(dir.path(), "v7.4-rc3", "failed", None).unwrap();
        assert_eq!(last_ok(&history(dir.path())), Tag::parse("v7.4-rc2"));
    }

    #[test]
    fn the_pinned_key_is_read_from_keys_toml() {
        let text = include_str!("../../deploy/kernel/keys.toml");
        let (keyserver, linus) = pinned_key(text).unwrap();
        assert!(keyserver.starts_with("https://"));
        assert_eq!(linus.len(), 40);
        assert_eq!(&linus[24..], "79BE3E4300411886");
    }

    #[test]
    fn verify_tag_status_lines_name_the_signer() {
        let fpr = "ABAF11C65A2970B130ABE3C479BE3E4300411886";
        let out = format!(
            "[GNUPG:] NEWSIG\n[GNUPG:] VALIDSIG {fpr} 2026-11-15 1763222400 0 4 0 1 10 00 {fpr}\n"
        );
        assert_eq!(valid_signer(&out).as_deref(), Some(fpr));
        assert_eq!(valid_signer("[GNUPG:] BADSIG abc"), None);
        assert_eq!(
            colon_fingerprint("tru::1:0:0:3:1:5\npub:-:4096:1:79BE3E4300411886:1:::-:::scESC\nfpr:::::::::ABAF11C65A2970B130ABE3C479BE3E4300411886:\n").as_deref(),
            Some(fpr)
        );
    }

    #[test]
    fn the_tagger_time_is_the_epoch_before_the_zone() {
        let object = "object abc\ntype commit\ntag v7.4-rc3\ntagger Linus Torvalds <torvalds@linux-foundation.org> 1763222400 -0800\n\nLinux 7.4-rc3\n";
        assert_eq!(tagger_epoch(object), Some(1_763_222_400));
        assert_eq!(tagger_epoch("object abc\n"), None);
    }

    #[test]
    fn ldd_output_gives_the_libraries_and_the_loader() {
        let out = "\tlinux-vdso.so.1 (0x00007ffc)\n\tlibc.so.6 => /usr/lib/libc.so.6 (0x00007f00)\n\t/lib64/ld-linux-x86-64.so.2 (0x00007f11)\n\tlibgone.so => not found\n";
        assert_eq!(
            ldd_libs(out),
            ["/usr/lib/libc.so.6", "/lib64/ld-linux-x86-64.so.2"]
        );
        assert_eq!(module_deps("a, b,,c"), ["a", "b", "c"]);
        assert!(module_deps("").is_empty());
    }

    #[test]
    fn the_running_grub_entry_comes_from_the_command_line() {
        let cmdline = "BOOT_IMAGE=/boot/vmlinuz-linux-nie-rc root=UUID=1234-abcd rw quiet";
        assert_eq!(
            running_entry(cmdline).as_deref(),
            Some("gnulinux-linux-nie-rc-advanced-1234-abcd")
        );
        assert_eq!(running_entry("root=UUID=1 rw"), None);
        assert_eq!(running_entry("BOOT_IMAGE=/vmlinuz root=/dev/sda1"), None);
    }

    #[test]
    fn only_unit_and_mount_lines_count_in_the_required_list() {
        assert!(missing("").is_empty());
        assert!(missing("other thing\n# comment\n").is_empty());
    }

    #[test]
    fn the_first_package_is_the_lowest_versioned_name() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "linux-nie-rc-headers-7.4.pkg.tar.zst",
            "linux-nie-rc-7.4.0rc3-1-x86_64.pkg.tar.zst",
            "makepkg.log",
        ] {
            fs::write(dir.path().join(name), b"").unwrap();
        }
        assert!(
            first_package(dir.path())
                .unwrap()
                .ends_with("linux-nie-rc-7.4.0rc3-1-x86_64.pkg.tar.zst")
        );
        assert!(first_package(&dir.path().join("none")).is_err());
    }
}
