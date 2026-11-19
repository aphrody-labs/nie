//! Client configuration: `link.toml` in the state directory.
use crate::client::{Host, Role, Router, Upstream};
use crate::server::load_token;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostCfg {
    pub name: String,
    pub url: String,
    pub role: Role,
    /// Route pages that no tier holds through this cache so it fills.
    #[serde(default)]
    pub fill: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// The folder that shows the installation (the ProjFS virtualization root).
    pub root: Option<PathBuf>,
    /// Page cache directory (default `<state>/pages`).
    pub cache_dir: Option<PathBuf>,
    pub cache_gib: f64,
    pub token_file: Option<PathBuf>,
    pub token: Option<String>,
    #[serde(rename = "host")]
    pub hosts: Vec<HostCfg>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            root: None,
            cache_dir: None,
            cache_gib: 8.0,
            token_file: None,
            token: None,
            hosts: Vec::new(),
        }
    }
}

/// Where the client keeps its configuration, page cache, manifest copy and traces.
#[must_use]
pub fn state_dir() -> PathBuf {
    let var = |k: &str| std::env::var_os(k).map(PathBuf::from);
    var("NIE_LINK_DIR")
        .or_else(|| var("LOCALAPPDATA").map(|d| d.join("nie").join("link")))
        .or_else(|| var("XDG_STATE_HOME").map(|d| d.join("nie-link")))
        .or_else(|| var("HOME").map(|d| d.join(".local").join("state").join("nie-link")))
        .unwrap_or_else(|| PathBuf::from("nie-link"))
}

impl Config {
    #[must_use]
    pub fn default_path() -> PathBuf {
        state_dir().join("link.toml")
    }

    pub fn load(path: Option<&Path>) -> Result<Self> {
        let path = path.map_or_else(Self::default_path, Path::to_path_buf);
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("{} (create it with `nie-link configure`)", path.display()))?;
        let cfg: Self = toml::from_str(&text).with_context(|| path.display().to_string())?;
        if cfg.hosts.is_empty() {
            bail!("{}: no [[host]] entry", path.display());
        }
        Ok(cfg)
    }

    pub fn save(&self, path: Option<&Path>) -> Result<PathBuf> {
        let path = path.map_or_else(Self::default_path, Path::to_path_buf);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, toml::to_string_pretty(self)?)?;
        Ok(path)
    }

    pub fn token(&self) -> Result<String> {
        match (&self.token, &self.token_file) {
            (Some(t), _) if t.len() >= 32 => Ok(t.clone()),
            (_, Some(f)) => load_token(Some(f)),
            _ => load_token(None),
        }
    }

    pub fn router(&self) -> Result<Router> {
        let token = self.token()?;
        Ok(Router::new(
            self.hosts
                .iter()
                .map(|h| Host::new(Upstream::new(&h.name, &h.url, &token), h.role, h.fill))
                .collect(),
        ))
    }

    #[must_use]
    pub fn cache_dir(&self) -> PathBuf {
        self.cache_dir.clone().unwrap_or_else(|| state_dir().join("pages"))
    }

    #[must_use]
    pub fn cache_bytes(&self) -> u64 {
        (self.cache_gib * 1_073_741_824.0) as u64
    }

    /// The installation folder: configured, else `<state>/game/<install dir of the build>`.
    #[must_use]
    pub fn root_for(&self, install_dir: &str) -> PathBuf {
        self.root.clone().unwrap_or_else(|| state_dir().join("game").join(install_dir))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hosts_and_defaults() {
        let cfg: Config = toml::from_str(
            "cache_gib = 2.5\n[[host]]\nname = \"prd\"\nurl = \"http://10.0.0.5:8798\"\nrole = \"origin\"\n[[host]]\nname = \"fr1\"\nurl = \"http://10.0.0.1:8798\"\nrole = \"cache\"\nfill = true\n",
        )
        .unwrap();
        assert_eq!(cfg.hosts.len(), 2);
        assert_eq!(cfg.hosts[1].role, Role::Cache);
        assert!(cfg.hosts[1].fill && !cfg.hosts[0].fill);
        assert_eq!(cfg.cache_bytes(), 2_684_354_560);
        let back: Config = toml::from_str(&toml::to_string_pretty(&cfg).unwrap()).unwrap();
        assert_eq!(back.hosts[0].name, "prd");
    }
}
