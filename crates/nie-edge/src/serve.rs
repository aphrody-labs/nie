//! Server assembly: pingora `Server`, the proxy service with its listeners, and the health
//! check services of both upstreams.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use pingora_core::listeners::tls::TlsSettings;
use pingora_core::server::Server;
use pingora_core::server::configuration::{Opt, ServerConf};
use pingora_core::services::background::background_service;
use pingora_http::RequestHeader;
use pingora_load_balancing::LoadBalancer;
use pingora_load_balancing::health_check::HttpHealthCheck;
use pingora_load_balancing::selection::RoundRobin;
use pingora_proxy::http_proxy_service;

use crate::cache::CacheBackend;
use crate::config::{Config, Tls, Upstream};
use crate::limits::Limits;
use crate::proxy::{Edge, Pool};

/// Command-line switches of `serve` that pingora reads.
#[derive(Debug, Clone, Copy, Default)]
pub struct ServeOpts {
    /// Take the listening sockets over from the running instance (graceful reload).
    pub upgrade: bool,
    /// Detach from the terminal; pid file and upgrade socket come from `[server]`.
    pub daemon: bool,
}

/// The pingora `ServerConf` of a configuration.
pub fn server_conf(config: &Config) -> Result<ServerConf> {
    config
        .server
        .clone()
        .try_into()
        .context("[server] is not a valid pingora ServerConf")
}

fn pool(up: &Upstream) -> Result<(Pool, impl pingora_core::services::ServiceWithDependents + use<>)> {
    let mut lb: LoadBalancer<RoundRobin> = LoadBalancer::try_from_iter([up.addr.as_str()])
        .with_context(|| format!("upstream {}", up.addr))?;
    let mut check = HttpHealthCheck::new(&up.addr, false);
    check.req = RequestHeader::build("GET", up.health_path.as_bytes(), None)
        .map_err(|e| anyhow!("health path {}: {e}", up.health_path))?;
    check
        .req
        .append_header("Host", up.addr.as_str())
        .map_err(|e| anyhow!("health host: {e}"))?;
    check.consecutive_failure = 2;
    lb.set_health_check(Box::new(check));
    lb.health_check_frequency = Some(Duration::from_secs(up.health_interval_secs.max(1)));
    let service = background_service(&format!("health {}", up.addr), lb);
    let pool = Pool {
        lb: service.task(),
        addr: up.addr.clone(),
        read_timeout: Duration::from_secs(up.read_timeout_secs.max(1)),
    };
    Ok((pool, service))
}

/// The mutual-TLS listener settings: the Origin CA certificate, and clients accepted only with a
/// certificate signed by `client_ca`.
pub fn tls_settings(tls: &Tls) -> Result<TlsSettings> {
    use pingora_core::tls::{
        RootCertStore, WebPkiClientVerifier, install_default_crypto_provider,
        load_ca_file_into_store,
    };
    install_default_crypto_provider();
    let mut roots = RootCertStore::empty();
    load_ca_file_into_store(&tls.client_ca, &mut roots)
        .map_err(|e| anyhow!("client_ca {}: {e}", tls.client_ca.display()))?;
    if roots.is_empty() {
        return Err(anyhow!(
            "client_ca {} holds no certificate",
            tls.client_ca.display()
        ));
    }
    let verifier = WebPkiClientVerifier::builder(Arc::new(roots))
        .build()
        .context("client certificate verifier")?;
    let cert = tls.cert.to_str().context("cert path")?;
    let key = tls.key.to_str().context("key path")?;
    let mut settings =
        TlsSettings::intermediate(cert, key).map_err(|e| anyhow!("origin certificate: {e}"))?;
    settings.set_client_cert_verifier(verifier);
    if tls.h2 {
        settings.enable_h2();
    }
    Ok(settings)
}

/// Run the edge until it is told to stop. Never returns.
pub fn run(config: &Config, opts: ServeOpts) -> Result<()> {
    let conf = server_conf(config)?;
    let opt = Opt {
        upgrade: opts.upgrade,
        daemon: opts.daemon,
        ..Opt::default()
    };
    let mut server = Server::new_with_opt_and_conf(opt, conf);
    server.bootstrap();

    let (core, core_health) = pool(&config.upstreams.core)?;
    let (media, media_health) = pool(&config.upstreams.media)?;
    let edge = Edge {
        core,
        media,
        limits: Limits::new(&config.limits),
        cache: config
            .cache
            .enabled
            .then(|| CacheBackend::leak(&config.cache)),
        tls_hosts: config
            .listen
            .tls
            .as_ref()
            .map(|t| t.hosts.iter().map(|h| h.to_ascii_lowercase()).collect())
            .unwrap_or_default(),
        access_log: config.access_log,
    };

    let mut service = http_proxy_service(&server.configuration, edge);
    for addr in &config.listen.http {
        service.add_tcp(addr);
    }
    if let Some(tls) = &config.listen.tls {
        service.add_tls_with_settings(&tls.addr, None, tls_settings(tls)?);
    }
    server.add_service(service);
    server.add_service(core_health);
    server.add_service(media_health);
    server.run_forever()
}
