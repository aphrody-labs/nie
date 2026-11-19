//! `nie-link`: serve the installation as pages, link it on Windows, verify and benchmark the link.
use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use nie_link::client::{Role, Upstream};
use nie_link::config::{Config, HostCfg, state_dir};
use nie_link::link::Link;
use nie_link::server::{self, Cache, Origin};
use nie_link::{bench, proto, scan, steam, verify};
use std::net::TcpListener;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "nie-link", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a page server.
    #[command(subcommand)]
    Serve(Serve),
    /// Hash a Steam library into a manifest and print its summary (no server).
    Scan(ScanArgs),
    /// Write the client configuration (`link.toml`).
    Configure(ConfigureArgs),
    /// Hosts, manifest and cache state as JSON.
    Status(Common),
    /// Present the installation as a real NTFS folder (Windows, ProjFS) and keep serving it.
    Mount(MountArgs),
    /// Start `mount` at every logon of this user (Windows) and start it now.
    Install(MountArgs),
    /// Remove the logon start and stop the running mount (Windows).
    Uninstall,
    /// Fetch the pages of a recorded trace into the page cache.
    Prefetch(PrefetchArgs),
    /// Fill a cache tier with the pages of a recorded trace.
    Warm(WarmArgs),
    /// Read every file of the linked folder once so that ProjFS stores all of it locally.
    Hydrate(HydrateArgs),
    /// Hash every file of the linked folder and compare it with the origin and with Steam.
    Verify(VerifyArgs),
    /// Cold and warm read benchmark.
    Bench(BenchArgs),
}

#[derive(Subcommand)]
enum Serve {
    /// Serve a Steam library in place as pages (on the host that owns the bytes).
    Origin(OriginArgs),
    /// Read-through cache tier in front of an origin or another tier.
    Cache(CacheArgs),
}

#[derive(Args)]
struct Net {
    /// Address to listen on, for example the private-network address `<private-address>:8798`.
    #[arg(long, env = "NIE_LINK_BIND")]
    bind: String,
    /// File whose first line is the bearer token (at least 32 bytes); `NIE_LINK_TOKEN` otherwise.
    #[arg(long, env = "NIE_LINK_TOKEN_FILE")]
    token_file: Option<PathBuf>,
}

#[derive(Args)]
struct OriginArgs {
    #[command(flatten)]
    net: Net,
    /// The `steamapps` directory that holds `appmanifest_<id>.acf` and `common/`.
    #[arg(long, env = "NIE_LINK_STEAMAPPS")]
    steamapps: PathBuf,
    #[arg(long, env = "NIE_LINK_APP_ID")]
    app_id: Option<u32>,
    /// Where the manifest is kept between restarts.
    #[arg(long, env = "STATE_DIRECTORY")]
    state_dir: Option<PathBuf>,
    #[arg(long, default_value_t = proto::DEFAULT_PAGE_SIZE)]
    page_size: u32,
    /// Seconds between rescans for a new Steam build or changed files.
    #[arg(long, default_value_t = 60)]
    rescan_secs: u64,
}

#[derive(Args)]
struct CacheArgs {
    #[command(flatten)]
    net: Net,
    /// Base URL of the tier above, for example `http://<private-address>:8798`.
    #[arg(long, env = "NIE_LINK_UPSTREAM")]
    upstream: String,
    #[arg(long, default_value = "upstream")]
    upstream_name: String,
    /// Directory of the cached pages.
    #[arg(long, env = "CACHE_DIRECTORY")]
    dir: PathBuf,
    #[arg(long, env = "NIE_LINK_BUDGET_GIB", default_value_t = 4.0)]
    budget_gib: f64,
}

#[derive(Args)]
struct ScanArgs {
    #[arg(long)]
    steamapps: PathBuf,
    #[arg(long)]
    app_id: Option<u32>,
    #[arg(long, default_value_t = proto::DEFAULT_PAGE_SIZE)]
    page_size: u32,
    /// Write the manifest JSON here.
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Args, Clone)]
struct Common {
    /// Configuration file (default: `link.toml` in the state directory).
    #[arg(long, env = "NIE_LINK_CONFIG")]
    config: Option<PathBuf>,
}

#[derive(Args)]
struct ConfigureArgs {
    #[command(flatten)]
    common: Common,
    /// Origin as `name=url`, for example `prd=http://<private-address>:8798`.
    #[arg(long)]
    origin: String,
    /// Cache tiers as `name=url`, add `,fill` to route unheld pages through the tier.
    #[arg(long)]
    cache: Vec<String>,
    #[arg(long)]
    token_file: Option<PathBuf>,
    /// The linked folder (default: `<state>/game/<install dir>`).
    #[arg(long)]
    root: Option<PathBuf>,
    #[arg(long, default_value_t = 8.0)]
    cache_gib: f64,
}

#[derive(Args, Clone)]
struct MountArgs {
    #[command(flatten)]
    common: Common,
    /// Record the order of first page reads into trace `NAME` (saved every few seconds).
    #[arg(long)]
    record: Option<String>,
    /// Fetch the pages of trace `NAME` into the cache while mounted.
    #[arg(long)]
    prefetch: Option<String>,
    /// Append the log to this file instead of the console.
    #[arg(long)]
    log: Option<PathBuf>,
}

#[derive(Args)]
struct PrefetchArgs {
    #[command(flatten)]
    common: Common,
    #[arg(long, default_value = "startup")]
    trace: String,
    #[arg(long, default_value_t = 16)]
    parallel: usize,
}

#[derive(Args)]
struct WarmArgs {
    #[command(flatten)]
    common: Common,
    /// Name of the cache host in the configuration.
    #[arg(long)]
    host: String,
    #[arg(long, default_value = "startup")]
    trace: String,
    #[arg(long, default_value_t = 8)]
    parallel: usize,
}

#[derive(Args)]
struct HydrateArgs {
    #[command(flatten)]
    common: Common,
    #[arg(long)]
    root: Option<PathBuf>,
    /// Files read at once (each file is itself fetched with several connections).
    #[arg(long, default_value_t = 4)]
    threads: usize,
    #[arg(long)]
    only: Vec<String>,
}

#[derive(Args)]
struct VerifyArgs {
    #[command(flatten)]
    common: Common,
    /// Folder to verify (default: the configured linked folder).
    #[arg(long)]
    root: Option<PathBuf>,
    #[arg(long, default_value_t = 16)]
    threads: usize,
    /// Only files below these path prefixes.
    #[arg(long)]
    only: Vec<String>,
    /// Do not compare with Steam's depot manifest.
    #[arg(long)]
    no_steam: bool,
    /// Write the full report here.
    #[arg(long)]
    json: Option<PathBuf>,
}

#[derive(Args)]
struct BenchArgs {
    #[command(flatten)]
    common: Common,
    #[arg(long, default_value_t = 24)]
    pages: usize,
    #[arg(long, default_value_t = 8)]
    parallel: usize,
    /// Also read this file (path relative to the linked folder) and report first-byte latency.
    #[arg(long)]
    file: Vec<String>,
    #[arg(long, default_value_t = 256)]
    limit_mb: u64,
}

fn listener(net: &Net) -> Result<(TcpListener, String)> {
    let token = server::load_token(net.token_file.as_deref())?;
    let l = TcpListener::bind(&net.bind).with_context(|| format!("bind {}", net.bind))?;
    eprintln!("nie-link: listening on {}", l.local_addr()?);
    Ok((l, token))
}

fn split_named(spec: &str) -> Result<(String, String, bool)> {
    let (name, rest) = spec
        .split_once('=')
        .with_context(|| format!("{spec}: expected name=url"))?;
    let (url, fill) = match rest.strip_suffix(",fill") {
        Some(u) => (u, true),
        None => (rest, false),
    };
    Ok((name.to_owned(), url.to_owned(), fill))
}

fn print_json(v: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(v)?);
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Serve(Serve::Origin(a)) => {
            let (l, token) = listener(&a.net)?;
            let state = a.state_dir.unwrap_or_else(state_dir);
            let origin = Origin::start(
                &a.steamapps,
                a.app_id,
                &state,
                a.page_size,
                Duration::from_secs(a.rescan_secs),
            )?;
            server::serve(&l, &token, origin);
        }
        Cmd::Serve(Serve::Cache(a)) => {
            let (l, token) = listener(&a.net)?;
            let up = Upstream::new(&a.upstream_name, &a.upstream, &token);
            let budget = (a.budget_gib * 1_073_741_824.0) as u64;
            let cache = Cache::start(up, &a.dir, budget, Duration::from_secs(60))?;
            server::serve(&l, &token, cache);
        }
        Cmd::Scan(a) => {
            let install = steam::read_install(&a.steamapps, a.app_id)?;
            let m = scan::scan(
                &install,
                &scan::ScanOptions {
                    page_size: a.page_size,
                    progress: true,
                },
                None,
            )?;
            if let Some(out) = &a.out {
                std::fs::write(out, m.to_json())?;
            }
            print_json(&serde_json::json!({
                "buildId": m.build_id, "manifestId": m.manifest_id, "steamManifest": m.steam_manifest,
                "files": m.files.len(), "pages": m.page_count(), "bytes": m.total_bytes(),
                "pageSize": m.page_size,
                "steamManifestFile": install.depot_manifest_file,
            }))?;
        }
        Cmd::Configure(a) => {
            let mut hosts = Vec::new();
            let (name, url, _) = split_named(&a.origin)?;
            hosts.push(HostCfg {
                name,
                url,
                role: Role::Origin,
                fill: false,
            });
            for c in &a.cache {
                let (name, url, fill) = split_named(c)?;
                hosts.push(HostCfg {
                    name,
                    url,
                    role: Role::Cache,
                    fill,
                });
            }
            let cfg = Config {
                root: a.root,
                cache_gib: a.cache_gib,
                token_file: a.token_file,
                hosts,
                ..Config::default()
            };
            let path = cfg.save(a.common.config.as_deref())?;
            println!("{}", path.display());
        }
        Cmd::Status(c) => {
            let cfg = Config::load(c.config.as_deref())?;
            let link = Link::open(&cfg)?;
            print_json(&link.status())?;
        }
        Cmd::Mount(a) => mount(&a)?,
        Cmd::Install(a) => install(&a)?,
        Cmd::Uninstall => uninstall()?,
        Cmd::Prefetch(a) => {
            let link = Link::open(&Config::load(a.common.config.as_deref())?)?;
            let trace = link.load_trace(&a.trace)?;
            let t0 = std::time::Instant::now();
            let (fetched, failed) = link.prefetch(&trace, a.parallel);
            print_json(&serde_json::json!({
                "trace": a.trace, "pages": trace.len(), "fetched": fetched, "failed": failed,
                "seconds": t0.elapsed().as_secs_f64(),
            }))?;
        }
        Cmd::Warm(a) => {
            let link = Link::open(&Config::load(a.common.config.as_deref())?)?;
            let trace = link.load_trace(&a.trace)?;
            let t0 = std::time::Instant::now();
            let (ok, failed) = link.warm(&a.host, &trace, a.parallel)?;
            print_json(&serde_json::json!({
                "host": a.host, "trace": a.trace, "pages": trace.len(), "ok": ok, "failed": failed,
                "seconds": t0.elapsed().as_secs_f64(),
            }))?;
        }
        Cmd::Hydrate(a) => {
            let cfg = Config::load(a.common.config.as_deref())?;
            let manifest = cfg.router()?.manifest().map_err(|e| anyhow::anyhow!("{e}"))?;
            let root = a.root.unwrap_or_else(|| cfg.root_for(&manifest.install_dir));
            let s = nie_link::hydrate::run(&root, &manifest, a.threads, &a.only, true)?;
            print_json(&s)?;
            if !s.failed.is_empty() {
                std::process::exit(1);
            }
        }
        Cmd::Verify(a) => {
            let cfg = Config::load(a.common.config.as_deref())?;
            let router = cfg.router()?;
            let manifest = router.manifest().map_err(|e| anyhow::anyhow!("{e}"))?;
            let root = a
                .root
                .unwrap_or_else(|| cfg.root_for(&manifest.install_dir));
            let steam_manifest = if a.no_steam {
                None
            } else {
                router.steam_manifest().ok()
            };
            if !a.no_steam && steam_manifest.is_none() {
                eprintln!(
                    "nie-link: no origin has Steam's depot manifest, comparing with the origin manifest only"
                );
            }
            let report = verify::run(
                &root,
                &manifest,
                &verify::Options {
                    threads: a.threads,
                    only: a.only,
                    steam_manifest,
                    progress: true,
                },
            )?;
            if let Some(p) = &a.json {
                std::fs::write(p, serde_json::to_vec_pretty(&report)?)?;
            }
            let mut shown = serde_json::to_value(&report)?;
            for key in ["extra", "missing"] {
                if let Some(list) = shown[key].as_array_mut() {
                    let n = list.len();
                    list.truncate(20);
                    if n > 20 {
                        list.push(format!("... {} more", n - 20).into());
                    }
                }
            }
            print_json(&shown)?;
            if !report.clean() {
                std::process::exit(1);
            }
        }
        Cmd::Bench(a) => {
            let cfg = Config::load(a.common.config.as_deref())?;
            let router = cfg.router()?;
            let manifest = router.manifest().map_err(|e| anyhow::anyhow!("{e}"))?;
            let mut rows = Vec::new();
            for h in &router.hosts {
                h.ping();
                rows.extend(bench::host(&h.up, &manifest, a.pages, a.parallel)?);
            }
            let dir = std::env::temp_dir().join(format!("nie-link-bench-{}", std::process::id()));
            if let Some(h) = router.hosts.first() {
                rows.push(bench::warm(&h.up, &manifest, a.pages, &dir)?);
            }
            let _ = std::fs::remove_dir_all(&dir);
            let root = cfg.root_for(&manifest.install_dir);
            for rel in &a.file {
                let path = rel.split('/').fold(root.clone(), |p, part| p.join(part));
                rows.push(bench::file(&path, a.limit_mb << 20, 1 << 20)?);
            }
            print_json(&rows)?;
        }
    }
    Ok(())
}

#[cfg(windows)]
fn mount(a: &MountArgs) -> Result<()> {
    if let Some(log) = &a.log {
        nie_link::startup::redirect_stderr(log)?;
    }
    let cfg = Config::load(a.common.config.as_deref())?;
    let link = Link::open(&cfg)?;
    let root = cfg.root_for(&link.manifest.install_dir);
    nie_link::projfs::run(link, &root, a.record.as_deref(), a.prefetch.as_deref())
}

#[cfg(not(windows))]
fn mount(_: &MountArgs) -> Result<()> {
    anyhow::bail!("mount presents the installation through Windows ProjFS; run it on Windows")
}

#[cfg(windows)]
fn install(a: &MountArgs) -> Result<()> {
    nie_link::startup::install(
        a.common.config.as_deref(),
        a.record.as_deref(),
        a.prefetch.as_deref(),
    )
}

#[cfg(not(windows))]
fn install(_: &MountArgs) -> Result<()> {
    anyhow::bail!("install registers a Windows logon start; use deploy/systemd/nie-link.service on Linux")
}

#[cfg(windows)]
fn uninstall() -> Result<()> {
    nie_link::startup::uninstall()
}

#[cfg(not(windows))]
fn uninstall() -> Result<()> {
    anyhow::bail!("uninstall removes a Windows logon start")
}
