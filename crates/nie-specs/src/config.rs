use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct Sources {
    #[serde(default)]
    pub spec: Vec<Spec>,
    #[serde(default)]
    pub host: Vec<Host>,
}

#[derive(Debug, Deserialize)]
pub struct Spec {
    pub id: String,
    pub title: String,
    /// Official landing page, linked from the generated document.
    pub official: String,
    /// Where nie uses it.
    #[serde(default)]
    pub used_by: String,
    pub version: Version,
    /// Latest commit of the upstream repository (optionally restricted to one path).
    pub commit: Option<Commit>,
    #[serde(default)]
    pub fact: Vec<Fact>,
}

/// Version read from an official page: the capture groups (joined with ".") are the label, a group named `date` is the date.
#[derive(Debug, Deserialize)]
pub struct Version {
    pub url: String,
    pub pattern: String,
    #[serde(default)]
    pub mode: Mode,
}

#[derive(Debug, Deserialize)]
pub struct Commit {
    pub repo: String,
    pub path: Option<String>,
    pub branch: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Fact {
    pub key: String,
    pub what: String,
    /// Page the fact is read from; also the link shown next to it.
    pub url: String,
    #[serde(default)]
    pub mode: Mode,
    /// Regular expression; the capture groups, joined with `join`, are the value.
    pub pattern: Option<String>,
    #[serde(default)]
    pub join: String,
    /// `owner/repo`: the value is the latest GitHub release tag and its date instead.
    pub release_repo: Option<String>,
}

#[derive(
    Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Tags, scripts and styles removed, entities decoded, whitespace collapsed.
    #[default]
    Text,
    /// The bytes as served (Markdown, headers).
    Raw,
}

#[derive(Debug, Deserialize)]
pub struct Host {
    pub name: String,
    /// `linux` or `windows`.
    pub os: String,
    /// ssh alias; absent for a host that is only collected locally.
    pub ssh: Option<String>,
}

impl Sources {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| path.display().to_string())?;
        toml::from_str(&text).with_context(|| path.display().to_string())
    }
}
