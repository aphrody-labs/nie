//! `nie.toml`: the manifest of one app.

use anyhow::{Context as _, bail};
use serde::{Deserialize, Serialize};

/// The manifest of an app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppManifest {
    /// Identity of the app.
    pub app: App,
    /// Build targets.
    pub targets: Targets,
    /// Plugins the app uses.
    #[serde(default)]
    pub plugins: Plugins,
    /// C ABI exports (library apps).
    #[serde(default)]
    pub abi: Abi,
}

/// `[app]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct App {
    /// Package name: lowercase ASCII letters, digits and `-`.
    pub name: String,
    /// Template the app was scaffolded from.
    pub template: String,
}

/// `[targets]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Targets {
    /// Rust target triples the app builds for.
    pub list: Vec<String>,
}

/// `[plugins]`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plugins {
    /// Plugin ids from `nie plugins`.
    #[serde(default, rename = "use")]
    pub used: Vec<String>,
}

/// `[abi]`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Abi {
    /// Contract file, relative to the app, when the app exports a C ABI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<String>,
    /// Exported C symbols.
    #[serde(default)]
    pub exports: Vec<String>,
}

impl AppManifest {
    /// Parses and validates a manifest.
    ///
    /// # Errors
    /// On malformed TOML or a rule violation.
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        let m: Self = toml::from_str(text).context("nie.toml")?;
        m.validate()?;
        Ok(m)
    }

    /// Serialises the manifest.
    ///
    /// # Errors
    /// When the manifest cannot be written as TOML.
    pub fn render(&self) -> anyhow::Result<String> {
        Ok(toml::to_string(self)?)
    }

    /// Checks the rules of the manifest.
    ///
    /// # Errors
    /// On an invalid name, template or target list.
    pub fn validate(&self) -> anyhow::Result<()> {
        valid_name(&self.app.name)?;
        if crate::Template::parse(&self.app.template).is_none() {
            bail!("unknown template `{}`", self.app.template);
        }
        if self.targets.list.is_empty() {
            bail!("[targets] list is empty");
        }
        if self.targets.list.iter().any(|t| t.split('-').count() < 3) {
            bail!("[targets] list holds a value that is not a target triple");
        }
        Ok(())
    }
}

/// Checks an app name.
///
/// # Errors
/// When the name is empty, starts with a digit or `-`, or holds a character outside `a-z0-9-`.
pub fn valid_name(name: &str) -> anyhow::Result<()> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && !name.ends_with('-')
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !ok {
        bail!("invalid app name `{name}`: lowercase letters, digits and `-`, starting with a letter");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(valid_name("my-app2").is_ok());
        for bad in ["", "2x", "-x", "x-", "X", "a_b", "a b"] {
            assert!(valid_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn round_trip() {
        let m = AppManifest {
            app: App { name: "x".into(), template: "lib".into() },
            targets: Targets { list: vec!["x86_64-unknown-linux-gnu".into()] },
            plugins: Plugins::default(),
            abi: Abi { contract: Some("abi.json".into()), exports: vec!["nie_x_add".into()] },
        };
        assert_eq!(AppManifest::parse(&m.render().unwrap()).unwrap(), m);
    }

    #[test]
    fn refuses_unknown_template_and_empty_targets() {
        let t = "[app]\nname=\"x\"\ntemplate=\"nope\"\n[targets]\nlist=[\"a-b-c\"]\n";
        assert!(AppManifest::parse(t).is_err());
        let t = "[app]\nname=\"x\"\ntemplate=\"cli\"\n[targets]\nlist=[]\n";
        assert!(AppManifest::parse(t).is_err());
    }
}
