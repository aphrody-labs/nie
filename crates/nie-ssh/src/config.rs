//! Target parsing and `~/.ssh/config` resolution, jump hosts included.

use crate::{Error, Result};
use ssh2_config::{ParseRule, SshConfig};
use std::path::{Path, PathBuf};
use std::time::Duration;

const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_IDENTITIES: [&str; 3] = ["id_ed25519", "id_ecdsa", "id_rsa"];

/// Where to connect and how, before `~/.ssh/config` is applied.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Target {
    pub user: Option<String>,
    pub host: String,
    pub port: Option<u16>,
    /// Apply `~/.ssh/config` to this host (rclone remotes carry their own settings).
    pub use_config: bool,
    /// Identity files; empty means the ones of `~/.ssh/config`, then `id_ed25519`, `id_ecdsa`, `id_rsa`.
    pub identity_files: Vec<PathBuf>,
    /// `known_hosts` files; `None` means `~/.ssh/known_hosts` and `/etc/ssh/ssh_known_hosts`.
    pub known_hosts: Option<PathBuf>,
    /// Extra jump hosts (`[user@]host[:port]`), before the ones of `ProxyJump`.
    pub jumps: Vec<String>,
}

impl Target {
    /// `[user@]host[:port]` with `[v6]` brackets, `~/.ssh/config` applied.
    pub fn parse(authority: &str) -> Result<Self> {
        let (user, host, port) = split_authority(authority)?;
        Ok(Self {
            user,
            host,
            port,
            use_config: true,
            ..Self::default()
        })
    }
}

/// Splits `[user@]host[:port]`, with `[v6]` brackets.
pub fn split_authority(authority: &str) -> Result<(Option<String>, String, Option<u16>)> {
    let (user, host_port) = match authority.rsplit_once('@') {
        Some((user, rest)) if !user.is_empty() => (Some(user.to_owned()), rest),
        Some(_) => return Err(Error::new("invalid SSH user")),
        None => (None, authority),
    };
    let invalid_port = || Error::new("invalid SSH port");
    let (host, port) = if let Some(bracketed) = host_port.strip_prefix('[') {
        let (host, rest) = bracketed
            .split_once(']')
            .ok_or_else(|| Error::new("invalid SSH host"))?;
        let port = match rest.strip_prefix(':') {
            Some(port) => Some(port.parse().map_err(|_| invalid_port())?),
            None if rest.is_empty() => None,
            None => return Err(Error::new("invalid SSH host")),
        };
        (host.to_owned(), port)
    } else {
        match host_port.rsplit_once(':') {
            Some((host, port)) => (
                host.to_owned(),
                Some(port.parse().map_err(|_| invalid_port())?),
            ),
            None => (host_port.to_owned(), None),
        }
    };
    if host.is_empty() || host.starts_with('-') {
        return Err(Error::new("invalid SSH host"));
    }
    Ok((user, host, port))
}

/// The home directory of the current user, from the environment.
pub fn home_dir() -> Option<PathBuf> {
    ["HOME", "USERPROFILE"]
        .iter()
        .find_map(|key| std::env::var_os(key).filter(|v| !v.is_empty()))
        .map(PathBuf::from)
}

fn expand_tilde(path: &Path, home: &Path) -> PathBuf {
    match path.strip_prefix("~") {
        Ok(rest) => home.join(rest),
        Err(_) => path.to_path_buf(),
    }
}

/// One SSH endpoint with `ssh_config` applied.
#[derive(Clone, Debug)]
pub struct Hop {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub identities: Vec<PathBuf>,
    pub timeout: Duration,
}

fn default_user() -> Result<String> {
    ["USER", "USERNAME", "LOGNAME"]
        .iter()
        .find_map(|key| std::env::var(key).ok().filter(|v| !v.is_empty()))
        .ok_or_else(|| Error::new("SSH user unknown: use user@host or User in ~/.ssh/config"))
}

fn load_config(home: &Path) -> Result<Option<SshConfig>> {
    let path = home.join(".ssh").join("config");
    let Ok(file) = std::fs::File::open(&path) else {
        return Ok(None);
    };
    SshConfig::default()
        .home_dir(home)
        .parse(
            &mut std::io::BufReader::new(file),
            ParseRule::ALLOW_UNKNOWN_FIELDS | ParseRule::ALLOW_UNSUPPORTED_FIELDS,
        )
        .map(Some)
        .map_err(|e| Error::new(format!("{}: {e}", path.display())))
}

fn resolve_hop(
    config: Option<&SshConfig>,
    home: &Path,
    host: &str,
    port: Option<u16>,
    user: Option<&str>,
    explicit_identities: &[PathBuf],
) -> Result<(Hop, Vec<String>)> {
    let params = config.map(|c| c.query(host));
    let from_config = |f: fn(&ssh2_config::HostParams) -> Option<String>| {
        params.as_ref().and_then(f).filter(|v| !v.is_empty())
    };
    let user = match user {
        Some(user) => user.to_owned(),
        None => match from_config(|p| p.user.clone()) {
            Some(user) => user,
            None => default_user()?,
        },
    };
    let mut identities: Vec<PathBuf> = explicit_identities.to_vec();
    if identities.is_empty() {
        identities = params
            .as_ref()
            .and_then(|p| p.identity_file.clone())
            .unwrap_or_default()
            .iter()
            .map(|p| expand_tilde(p, home))
            .collect();
    }
    if identities.is_empty() {
        identities = DEFAULT_IDENTITIES
            .iter()
            .map(|name| home.join(".ssh").join(name))
            .collect();
    }
    identities.retain(|p| p.is_file());
    let hop = Hop {
        host: from_config(|p| p.host_name.clone()).unwrap_or_else(|| host.to_owned()),
        port: port
            .or_else(|| params.as_ref().and_then(|p| p.port))
            .unwrap_or(22),
        user,
        identities,
        timeout: params
            .as_ref()
            .and_then(|p| p.connect_timeout)
            .unwrap_or(DEFAULT_CONNECT_TIMEOUT),
    };
    let jumps = params
        .and_then(|p| p.proxy_jump)
        .unwrap_or_default()
        .into_iter()
        .filter(|j| !j.eq_ignore_ascii_case("none"))
        .collect();
    Ok((hop, jumps))
}

/// Jump hosts then the target, each resolved through `ssh_config`.
pub fn resolve_chain(target: &Target, home: &Path) -> Result<Vec<Hop>> {
    let config = if target.use_config {
        load_config(home)?
    } else {
        None
    };
    let (hop, config_jumps) = resolve_hop(
        config.as_ref(),
        home,
        &target.host,
        target.port,
        target.user.as_deref(),
        &target.identity_files,
    )?;
    let jumps = target.jumps.iter().cloned().chain(config_jumps);
    let mut chain = Vec::new();
    for jump in jumps {
        let jump = jump.strip_prefix("ssh://").unwrap_or(&jump);
        let (user, host, port) = split_authority(jump)?;
        chain.push(resolve_hop(config.as_ref(), home, &host, port, user.as_deref(), &[])?.0);
    }
    chain.push(hop);
    Ok(chain)
}

/// The `known_hosts` files consulted for `target`.
pub fn default_known_hosts(target: &Target, home: &Path) -> Vec<PathBuf> {
    match &target.known_hosts {
        Some(path) => vec![path.clone()],
        None => vec![
            home.join(".ssh").join("known_hosts"),
            PathBuf::from("/etc/ssh/ssh_known_hosts"),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authority_splits_user_host_port_and_v6() {
        assert_eq!(
            split_authority("deploy@prd:2200").unwrap(),
            (Some("deploy".into()), "prd".into(), Some(2200))
        );
        assert_eq!(
            split_authority("[::1]:22").unwrap(),
            (None, "::1".into(), Some(22))
        );
        assert_eq!(split_authority("prd").unwrap(), (None, "prd".into(), None));
        assert!(split_authority("-oProxyCommand=x").is_err());
        assert!(split_authority("@host").is_err());
        assert!(split_authority("host:notaport").is_err());
    }

    #[test]
    fn ssh_config_supplies_host_name_port_user_identity_and_jumps() {
        let home = tempfile::tempdir().unwrap();
        let ssh = home.path().join(".ssh");
        std::fs::create_dir_all(&ssh).unwrap();
        std::fs::write(ssh.join("deploy_key"), "k").unwrap();
        std::fs::write(
            ssh.join("config"),
            "Host jump\n  HostName 192.0.2.1\n  User jump\nHost prd\n  HostName 10.0.0.7\n  User deploy\n  Port 2222\n  IdentityFile ~/.ssh/deploy_key\n  ProxyJump jump\n  ConnectTimeout 7\n",
        )
        .unwrap();
        let chain = resolve_chain(&Target::parse("prd").unwrap(), home.path()).unwrap();
        assert_eq!(chain.len(), 2);
        assert_eq!(
            (chain[0].host.as_str(), chain[0].user.as_str(), chain[0].port),
            ("192.0.2.1", "jump", 22)
        );
        assert_eq!(
            (chain[1].host.as_str(), chain[1].user.as_str(), chain[1].port),
            ("10.0.0.7", "deploy", 2222)
        );
        assert_eq!(chain[1].identities, vec![ssh.join("deploy_key")]);
        assert_eq!(chain[1].timeout, Duration::from_secs(7));
        let overridden = resolve_chain(&Target::parse("other@prd:2200").unwrap(), home.path()).unwrap();
        assert_eq!(
            (overridden[1].user.as_str(), overridden[1].port),
            ("other", 2200)
        );
    }

    #[test]
    fn explicit_jumps_come_before_config_jumps() {
        let home = tempfile::tempdir().unwrap();
        let mut target = Target::parse("u@target").unwrap();
        target.jumps = vec!["j@edge:2201".into()];
        let chain = resolve_chain(&target, home.path()).unwrap();
        assert_eq!(chain.len(), 2);
        assert_eq!((chain[0].host.as_str(), chain[0].port), ("edge", 2201));
        assert_eq!(chain[1].host, "target");
    }
}
