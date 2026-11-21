//! `nie-deploy`: the process supervisor of the nie stack (no init system, no container runtime),
//! its installer over SSH and the Cloudflare ingress steps.

mod cloudflare;
#[cfg(unix)]
mod config;
mod host;
mod keys;
#[cfg(unix)]
mod logs;
mod nftables;
#[cfg(unix)]
mod supervise;
#[cfg(unix)]
mod up;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};

use cloudflare::{Params, Step};

#[derive(Parser)]
#[command(name = "nie-deploy", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the services of `--config` under the supervisor: own session and user per service,
    /// restart with backoff, rotated logs, health probes. One instance per host.
    #[cfg(unix)]
    Supervise {
        /// Configuration file.
        #[arg(long, default_value = config::INSTALLED)]
        config: PathBuf,
        /// Start the supervisor in a new session and return.
        #[arg(long)]
        detach: bool,
    },
    /// Install binaries and configuration, start the supervisor detached and wait for health;
    /// idempotent. With `--host` the binaries of `--bin-dir` and the configuration go over SSH.
    #[cfg(unix)]
    Up {
        /// SSH destination (`[user@]host`, resolved through `~/.ssh/config`); this host when absent.
        #[arg(long)]
        host: Option<String>,
        /// Configuration to install (`-` reads stdin); default: the one embedded in this binary.
        #[arg(long)]
        config: Option<PathBuf>,
        /// Directory with the binaries to ship (default: the directory of this executable).
        #[arg(long)]
        bin_dir: Option<PathBuf>,
        /// Do not install the start-at-boot hook (cron.d or rc.local).
        #[arg(long)]
        no_boot_hook: bool,
    },
    /// The supervisor and every service with a live health probe.
    #[cfg(unix)]
    Status {
        /// SSH destination; this host when absent.
        #[arg(long)]
        host: Option<String>,
        /// Print JSON.
        #[arg(long)]
        json: bool,
    },
    /// The Cloudflare ingress of nie-edge: plan, apply, rollback, status.
    Cloudflare {
        #[command(subcommand)]
        action: Action,
    },
}

#[derive(Args, Clone)]
struct Target {
    /// Public hostname.
    #[arg(long, env = "NIE_HOSTNAME", default_value = "nie.aphrody.app")]
    hostname: String,
    /// Zone name (default: the last two labels of the hostname).
    #[arg(long)]
    zone: Option<String>,
    /// Origin TLS port (Cloudflare proxyable: 2053, 2083, 2087, 2096, 8443).
    #[arg(long, default_value_t = 2053)]
    port: u16,
    /// Public IPv4 of the origin (default: the address of the default route).
    #[arg(long)]
    origin_ip: Option<String>,
    /// Directory of edge.toml and the certificates.
    #[arg(long, default_value = "/etc/nie/edge")]
    edge_dir: PathBuf,
    /// Origin CA validity in days.
    #[arg(long, default_value_t = 1095)]
    validity_days: u32,
    /// Cloudflare API base.
    #[arg(long, default_value = cloudflare::API_BASE)]
    api_base: String,
    /// nftables table to extend instead of creating one (e.g. "inet fleet").
    #[arg(long, env = "NIE_FIREWALL_TABLE")]
    firewall_table: Option<String>,
}

impl Target {
    fn params(&self) -> Result<Params> {
        Ok(Params {
            zone_name: self.zone.clone().unwrap_or_else(|| cloudflare::zone_of(&self.hostname)),
            hostname: self.hostname.clone(),
            port: self.port,
            origin_ip: match &self.origin_ip {
                Some(ip) => ip.clone(),
                None => cloudflare::discover_origin_ip()?,
            },
            edge_dir: self.edge_dir.clone(),
            validity_days: self.validity_days,
            api_base: self.api_base.clone(),
            firewall_table: self.firewall_table.clone(),
        })
    }
}

#[derive(Subcommand)]
enum Action {
    /// Print the plan: every call, file and rollback. Offline.
    Plan(Target),
    /// Run steps in order (all, or --step) with CLOUDFLARE_API_TOKEN.
    Apply {
        #[command(flatten)]
        target: Target,
        /// Only this step.
        #[arg(long, value_enum)]
        step: Option<Step>,
        /// Stop before the DNS switch.
        #[arg(long)]
        before_dns: bool,
    },
    /// Undo one step.
    Rollback {
        #[command(flatten)]
        target: Target,
        /// Step to undo.
        #[arg(long, value_enum)]
        step: Step,
    },
    /// Read-only state of every step.
    Status(Target),
    /// Write a self-signed origin certificate, a client CA and a client certificate for local tests.
    TestCerts {
        /// Output directory.
        dir: PathBuf,
        /// Host name of the origin certificate.
        #[arg(long, default_value = "nie.aphrody.app")]
        hostname: String,
    },
}

fn token() -> Result<Option<String>> {
    Ok(std::env::var("CLOUDFLARE_API_TOKEN").ok().filter(|t| !t.is_empty()))
}

fn need_token() -> Result<String> {
    token()?.context("CLOUDFLARE_API_TOKEN is not set")
}

fn run_cloudflare(action: Action) -> Result<()> {
    match action {
        Action::Plan(target) => {
            print!("{}", cloudflare::plan(&target.params()?));
        }
        Action::Apply { target, step, before_dns } => {
            let params = target.params()?;
            let token = token()?;
            let steps: Vec<Step> = match step {
                Some(s) => vec![s],
                None => Step::ALL
                    .into_iter()
                    .filter(|s| !(before_dns && *s == Step::Dns))
                    .collect(),
            };
            for s in steps {
                let report = cloudflare::apply(&params, s, token.clone())
                    .with_context(|| format!("step {}", s.name()))?;
                println!("{:<12} {report}", s.name());
            }
        }
        Action::Rollback { target, step } => {
            let report = cloudflare::rollback(&target.params()?, step, Some(need_token()?))?;
            println!("{:<12} {report}", step.name());
        }
        Action::Status(target) => {
            let status = cloudflare::status(&target.params()?, Some(need_token()?))?;
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        Action::TestCerts { dir, hostname } => {
            std::fs::create_dir_all(&dir)?;
            let (cert, key) = cloudflare::self_signed_origin(&hostname)?;
            let auth = cloudflare::client_auth(&hostname)?;
            for (name, content) in [
                ("origin.pem", cert),
                ("origin.key", key),
                ("origin-pull-ca.pem", auth.ca_pem),
                ("client.pem", auth.chain_pem),
                ("client.key", auth.key_pem),
            ] {
                std::fs::write(dir.join(name), content)?;
            }
            println!("written to {}", dir.display());
        }
    }
    Ok(())
}

#[cfg(unix)]
fn supervise(config_path: &std::path::Path, detach: bool) -> Result<()> {
    let config = config::Config::load(config_path)?;
    if detach {
        if let Some(pid) = supervise::running_pid(&config.supervisor) {
            println!("supervisor pid {pid} already runs");
            return Ok(());
        }
        let exe = std::env::current_exe()?;
        let pid = supervise::spawn_detached(&exe, config_path, &config.supervisor)?;
        println!("supervisor pid {pid}");
        return Ok(());
    }
    supervise::install_signal_handlers()?;
    supervise::run(&config, config_path, &supervise::SIGNALS)
}

fn main() -> Result<()> {
    nie_tls::install();
    match Cli::parse().command {
        #[cfg(unix)]
        Cmd::Supervise { config, detach } => supervise(&config, detach),
        #[cfg(unix)]
        Cmd::Up { host, config, bin_dir, no_boot_hook } => match host {
            Some(host) => up::up_remote(&host, config.as_deref(), bin_dir.as_deref(), !no_boot_hook),
            None => up::up_local(config.as_deref(), !no_boot_hook),
        },
        #[cfg(unix)]
        Cmd::Status { host, json } => up::status(host.as_deref(), json),
        Cmd::Cloudflare { action } => run_cloudflare(action),
    }
}
