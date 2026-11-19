//! `nie-edge`: serve, check, reload, route, print-config.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use nie_edge::config::Config;
use nie_edge::route;

#[derive(Parser)]
#[command(name = "nie-edge", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,
    #[command(flatten)]
    common: Common,
}

#[derive(Args, Clone)]
struct Common {
    /// Configuration file (default: $NIE_EDGE_CONFIG, then /etc/nie/edge.toml when present).
    #[arg(short, long, global = true, env = "NIE_EDGE_CONFIG")]
    config: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the edge (default).
    Serve {
        /// Take the sockets over from the running instance (used by `reload`).
        #[arg(short, long)]
        upgrade: bool,
        /// Detach; pid file and upgrade socket come from [server].
        #[arg(short, long)]
        daemon: bool,
    },
    /// Validate the configuration, the certificates and the client CA, then exit.
    Check,
    /// Replace the running instance without dropping connections.
    Reload {
        /// Seconds to wait for the new instance.
        #[arg(long, default_value_t = 30)]
        timeout: u64,
    },
    /// Print where a request path goes and which limit family it falls in.
    Route {
        /// Request path, for example /model-full/x.glb
        path: String,
        /// Query string, without the question mark.
        #[arg(short, long)]
        query: Option<String>,
    },
    /// Print the effective configuration as TOML.
    PrintConfig,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    let config_path = cli.common.config.as_deref();
    match cli.command.unwrap_or(Cmd::Serve {
        upgrade: false,
        daemon: false,
    }) {
        Cmd::Route { path, query } => {
            let target = route::classify(&path);
            let family = route::limited(&path, query.as_deref()).map(route::Limited::name);
            println!(
                "{}",
                serde_json::json!({"path": path, "target": format!("{target:?}").to_ascii_lowercase(), "limit": family})
            );
            Ok(())
        }
        Cmd::PrintConfig => {
            print!("{}", toml::to_string_pretty(&Config::load(config_path)?)?);
            Ok(())
        }
        Cmd::Check => check(&Config::load(config_path)?),
        Cmd::Serve { upgrade, daemon } => serve(&Config::load(config_path)?, upgrade, daemon),
        Cmd::Reload { timeout } => reload(&Config::load(config_path)?, config_path, timeout),
    }
}

#[cfg(unix)]
fn check(config: &Config) -> Result<()> {
    nie_edge::serve::server_conf(config)?;
    if let Some(tls) = &config.listen.tls {
        nie_edge::serve::tls_settings(tls)?;
        let loaded = pingora_core::tls::load_certs_and_key_files(
            &tls.cert.to_string_lossy(),
            &tls.key.to_string_lossy(),
        );
        match loaded {
            Ok(Some(_)) => {}
            Ok(None) => anyhow::bail!(
                "no certificate or key in {} / {}",
                tls.cert.display(),
                tls.key.display()
            ),
            Err(e) => anyhow::bail!("origin certificate: {e}"),
        }
    }
    println!("ok");
    Ok(())
}

#[cfg(unix)]
fn serve(config: &Config, upgrade: bool, daemon: bool) -> Result<()> {
    nie_edge::serve::run(config, nie_edge::serve::ServeOpts { upgrade, daemon })
}

#[cfg(unix)]
fn reload(config: &Config, path: Option<&std::path::Path>, timeout: u64) -> Result<()> {
    nie_edge::reload::run(config, path, std::time::Duration::from_secs(timeout))
}

#[cfg(not(unix))]
fn check(config: &Config) -> Result<()> {
    config.validate()?;
    println!("ok (configuration only: pingora runs on unix)");
    Ok(())
}

#[cfg(not(unix))]
fn serve(_: &Config, _: bool, _: bool) -> Result<()> {
    anyhow::bail!("nie-edge serves on unix only")
}

#[cfg(not(unix))]
fn reload(_: &Config, _: Option<&std::path::Path>, _: u64) -> Result<()> {
    anyhow::bail!("nie-edge serves on unix only")
}
