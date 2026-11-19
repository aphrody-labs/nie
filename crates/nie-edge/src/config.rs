//! Edge configuration: a TOML file (default `/etc/nie/edge.toml`, or `--config`, or
//! `NIE_EDGE_CONFIG`). Every key has a default that matches the production host, so an absent
//! file starts an edge on `127.0.0.1:8798` in front of `127.0.0.1:8796` and `127.0.0.1:8797`.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::route::Limited;

/// Default configuration path.
pub const DEFAULT_PATH: &str = "/etc/nie/edge.toml";

/// The whole configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Keys of pingora's `ServerConf` (`threads`, `pid_file`, `upgrade_sock`, `daemon`, ...).
    pub server: toml::Table,
    /// The two upstream servers.
    pub upstreams: Upstreams,
    /// Listeners.
    pub listen: Listen,
    /// Per-family rate and concurrency limits.
    pub limits: Limits,
    /// Response cache.
    pub cache: Cache,
    /// One JSON line per request on stdout.
    pub access_log: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: toml::Table::new(),
            upstreams: Upstreams::default(),
            listen: Listen::default(),
            limits: Limits::default(),
            cache: Cache::default(),
            access_log: true,
        }
    }
}

/// `nie serve` and `nie-model-serve`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Upstreams {
    /// `nie serve`.
    pub core: Upstream,
    /// `nie-model-serve`.
    pub media: Upstream,
}

impl Default for Upstreams {
    fn default() -> Self {
        Self {
            core: Upstream::default(),
            media: Upstream {
                addr: "127.0.0.1:8797".into(),
                ..Upstream::default()
            },
        }
    }
}

/// One upstream server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Upstream {
    /// `host:port` of the server (plain HTTP, loopback).
    pub addr: String,
    /// Path of the health probe: 200 means the upstream takes traffic.
    pub health_path: String,
    /// Seconds between probes.
    pub health_interval_secs: u64,
    /// Seconds to wait for an upstream answer (decode work can be slow).
    pub read_timeout_secs: u64,
}

impl Default for Upstream {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1:8796".into(),
            health_path: "/health".into(),
            health_interval_secs: 5,
            read_timeout_secs: 120,
        }
    }
}

/// Listeners.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Listen {
    /// Plain HTTP listeners (loopback: smoke tests, local consumers).
    pub http: Vec<String>,
    /// The public listener: TLS with a Cloudflare Origin CA certificate and mutual TLS.
    pub tls: Option<Tls>,
}

impl Default for Listen {
    fn default() -> Self {
        Self {
            http: vec!["127.0.0.1:8798".into()],
            tls: None,
        }
    }
}

/// The TLS listener Cloudflare connects to.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tls {
    /// Cloudflare proxyable origin port: 2053, 2083, 2087, 2096 or 8443.
    pub addr: String,
    /// Origin CA certificate chain (PEM).
    pub cert: PathBuf,
    /// Its private key (PEM).
    pub key: PathBuf,
    /// CA that signs the client certificate Cloudflare presents (Authenticated Origin Pulls):
    /// connections without a certificate of this CA are refused during the handshake.
    pub client_ca: PathBuf,
    /// Host names served on this listener; any other `Host` gets 421. Empty accepts all.
    #[serde(default)]
    pub hosts: Vec<String>,
    /// Offer HTTP/2.
    #[serde(default = "yes")]
    pub h2: bool,
}

const fn yes() -> bool {
    true
}

/// A rate and concurrency rule. Zero disables the matching limit.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rule {
    /// Requests per second and client.
    pub rate_per_sec: u32,
    /// Requests in flight per client.
    pub client_inflight: u32,
    /// Requests in flight on the whole edge.
    pub global_inflight: u32,
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            rate_per_sec: 20,
            client_inflight: 4,
            global_inflight: 16,
        }
    }
}

/// Limits of the decode-heavy families.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Limits {
    /// `/model-*`.
    pub model: Rule,
    /// `/export`.
    pub export: Rule,
    /// `/video`.
    pub video: Rule,
    /// Wiki search.
    pub wiki_search: Rule,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            model: Rule {
                rate_per_sec: 10,
                client_inflight: 2,
                global_inflight: 6,
            },
            export: Rule {
                rate_per_sec: 5,
                client_inflight: 2,
                global_inflight: 4,
            },
            video: Rule {
                rate_per_sec: 40,
                client_inflight: 8,
                global_inflight: 16,
            },
            wiki_search: Rule {
                rate_per_sec: 20,
                client_inflight: 8,
                global_inflight: 32,
            },
        }
    }
}

impl Limits {
    /// The rule of a family.
    #[must_use]
    pub const fn rule(&self, family: Limited) -> Rule {
        match family {
            Limited::Model => self.model,
            Limited::Export => self.export,
            Limited::Video => self.video,
            Limited::WikiSearch => self.wiki_search,
        }
    }
}

/// Response cache (memory). Only `200` answers marked `Cache-Control: immutable` are stored.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Cache {
    /// Cache on or off.
    pub enabled: bool,
    /// Total size, in MiB, before the least recently used objects leave.
    pub capacity_mb: usize,
    /// Largest cached object, in MiB.
    pub max_object_mb: usize,
}

impl Default for Cache {
    fn default() -> Self {
        Self {
            enabled: true,
            capacity_mb: 256,
            max_object_mb: 16,
        }
    }
}

impl Config {
    /// Load the file at `path`, else `$NIE_EDGE_CONFIG`, else [`DEFAULT_PATH`] when it exists,
    /// else the defaults.
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let env = std::env::var_os("NIE_EDGE_CONFIG").map(PathBuf::from);
        let chosen = path.map(Path::to_path_buf).or(env).or_else(|| {
            Path::new(DEFAULT_PATH)
                .exists()
                .then(|| DEFAULT_PATH.into())
        });
        let config = match chosen {
            Some(p) => {
                let text =
                    std::fs::read_to_string(&p).with_context(|| format!("read {}", p.display()))?;
                toml::from_str(&text).with_context(|| format!("parse {}", p.display()))?
            }
            None => Self::default(),
        };
        config.validate()?;
        Ok(config)
    }

    /// Check addresses and the Cloudflare origin ports.
    pub fn validate(&self) -> Result<()> {
        for addr in &self.listen.http {
            addr.parse::<SocketAddr>()
                .with_context(|| format!("listen.http {addr}"))?;
        }
        if let Some(tls) = &self.listen.tls {
            let port = tls
                .addr
                .parse::<SocketAddr>()
                .with_context(|| format!("listen.tls.addr {}", tls.addr))?
                .port();
            if !CLOUDFLARE_PORTS.contains(&port) {
                bail!("listen.tls.addr port {port} is not a Cloudflare proxyable HTTPS port");
            }
        }
        for (name, up) in [
            ("core", &self.upstreams.core),
            ("media", &self.upstreams.media),
        ] {
            up.addr
                .parse::<SocketAddr>()
                .with_context(|| format!("upstreams.{name}.addr {}", up.addr))?;
            if !up.health_path.starts_with('/') {
                bail!("upstreams.{name}.health_path must start with /");
            }
        }
        Ok(())
    }
}

/// HTTPS ports Cloudflare proxies to an origin.
pub const CLOUDFLARE_PORTS: &[u16] = &[443, 2053, 2083, 2087, 2096, 8443];

impl Default for Tls {
    fn default() -> Self {
        Self {
            addr: "0.0.0.0:8443".into(),
            cert: "/etc/nie/edge/origin.pem".into(),
            key: "/etc/nie/edge/origin.key".into(),
            client_ca: "/etc/nie/edge/origin-pull-ca.pem".into(),
            hosts: vec!["nie.aphrody.app".into()],
            h2: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_validate_and_match_production_ports() {
        let c = Config::default();
        c.validate().unwrap();
        assert_eq!(c.upstreams.core.addr, "127.0.0.1:8796");
        assert_eq!(c.listen.http, ["127.0.0.1:8798"]);
        assert!(c.listen.tls.is_none());
    }

    #[test]
    fn media_defaults_are_set_in_the_file_only() {
        let c: Config = toml::from_str(
            r#"
            [upstreams.media]
            addr = "127.0.0.1:8797"
            [listen.tls]
            addr = "0.0.0.0:8443"
            cert = "a"
            key = "b"
            client_ca = "c"
            [limits.export]
            rate_per_sec = 1
            "#,
        )
        .unwrap();
        c.validate().unwrap();
        assert_eq!(c.upstreams.media.addr, "127.0.0.1:8797");
        assert_eq!(c.limits.export.rate_per_sec, 1);
        assert_eq!(c.limits.export.client_inflight, 4);
        assert!(c.listen.tls.unwrap().h2);
    }

    #[test]
    fn rejects_unknown_keys_and_bad_ports() {
        assert!(toml::from_str::<Config>("bogus = 1").is_err());
        let c: Config = toml::from_str(
            r#"
            [listen.tls]
            addr = "0.0.0.0:8080"
            cert = "a"
            key = "b"
            client_ca = "c"
            "#,
        )
        .unwrap();
        assert!(c.validate().is_err());
    }
}
