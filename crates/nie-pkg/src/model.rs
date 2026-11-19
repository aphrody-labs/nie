use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Hardware or distribution target a game shipped on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Ds,
    #[serde(rename = "3ds")]
    N3ds,
    Wii,
    Switch,
    Switch2,
    Ps4,
    Ps5,
    Xbox,
    Pc,
    Mobile,
}

impl Platform {
    pub const ALL: [Platform; 10] = [
        Platform::Ds,
        Platform::N3ds,
        Platform::Wii,
        Platform::Switch,
        Platform::Switch2,
        Platform::Ps4,
        Platform::Ps5,
        Platform::Xbox,
        Platform::Pc,
        Platform::Mobile,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Platform::Ds => "ds",
            Platform::N3ds => "3ds",
            Platform::Wii => "wii",
            Platform::Switch => "switch",
            Platform::Switch2 => "switch2",
            Platform::Ps4 => "ps4",
            Platform::Ps5 => "ps5",
            Platform::Xbox => "xbox",
            Platform::Pc => "pc",
            Platform::Mobile => "mobile",
        }
    }

    pub fn parse(s: &str) -> Option<Platform> {
        Platform::ALL.into_iter().find(|p| p.as_str() == s)
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A capability NIE can offer for a game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    DumpVerify,
    AssetExtract,
    TextExtract,
    TranslationPatch,
    SaveEdit,
    TeamBuilder,
    WikiData,
    ModLoader,
}

impl Feature {
    pub const ALL: [Feature; 8] = [
        Feature::DumpVerify,
        Feature::AssetExtract,
        Feature::TextExtract,
        Feature::TranslationPatch,
        Feature::SaveEdit,
        Feature::TeamBuilder,
        Feature::WikiData,
        Feature::ModLoader,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Feature::DumpVerify => "dump_verify",
            Feature::AssetExtract => "asset_extract",
            Feature::TextExtract => "text_extract",
            Feature::TranslationPatch => "translation_patch",
            Feature::SaveEdit => "save_edit",
            Feature::TeamBuilder => "team_builder",
            Feature::WikiData => "wiki_data",
            Feature::ModLoader => "mod_loader",
        }
    }
}

/// Support level of a feature for one game.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Support {
    #[default]
    None,
    Planned,
    Partial,
    Full,
}

impl Support {
    pub fn as_str(self) -> &'static str {
        match self {
            Support::None => "none",
            Support::Planned => "planned",
            Support::Partial => "partial",
            Support::Full => "full",
        }
    }
}

/// One release of a game on one platform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Release {
    pub platform: Platform,
    pub year: u16,
    #[serde(default)]
    pub regions: Vec<String>,
    /// Dump format the store expects (`nds-rom`, `3ds-cart`, `wii-disc`, `steam-depot`, ...).
    pub format: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steam_app_id: Option<u32>,
    /// `false` until the release data is checked against a primary source.
    #[serde(default)]
    pub verified: bool,
}

/// A tool, emulator, wiki or store around a game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ecosystem {
    pub kind: String,
    pub name: String,
    pub url: String,
}

/// Multilingual text map (`fr`, `en`, `ja`, `es`) stored in `games.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalizedText {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fr: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub en: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ja: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub es: String,
}

impl LocalizedText {
    pub fn pick(&self, lang: &str) -> &str {
        let base = lang.split(['-', '_']).next().unwrap_or(lang);
        match base {
            "en" => {
                if !self.en.is_empty() {
                    &self.en
                } else if !self.fr.is_empty() {
                    &self.fr
                } else if !self.es.is_empty() {
                    &self.es
                } else {
                    &self.ja
                }
            }
            "ja" => {
                if !self.ja.is_empty() {
                    &self.ja
                } else if !self.en.is_empty() {
                    &self.en
                } else if !self.fr.is_empty() {
                    &self.fr
                } else {
                    &self.es
                }
            }
            "es" => {
                if !self.es.is_empty() {
                    &self.es
                } else if !self.en.is_empty() {
                    &self.en
                } else if !self.fr.is_empty() {
                    &self.fr
                } else {
                    &self.ja
                }
            }
            _ => {
                if !self.fr.is_empty() {
                    &self.fr
                } else if !self.en.is_empty() {
                    &self.en
                } else if !self.es.is_empty() {
                    &self.es
                } else {
                    &self.ja
                }
            }
        }
    }
}

/// Multilingual bullet list (`fr`, `en`, `ja`, `es`) stored in `games.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalizedList {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fr: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub en: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ja: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub es: Vec<String>,
}

impl LocalizedList {
    pub fn pick(&self, lang: &str) -> &[String] {
        let base = lang.split(['-', '_']).next().unwrap_or(lang);
        match base {
            "en" => {
                if !self.en.is_empty() {
                    &self.en
                } else if !self.fr.is_empty() {
                    &self.fr
                } else if !self.es.is_empty() {
                    &self.es
                } else {
                    &self.ja
                }
            }
            "ja" => {
                if !self.ja.is_empty() {
                    &self.ja
                } else if !self.en.is_empty() {
                    &self.en
                } else {
                    &self.fr
                }
            }
            "es" => {
                if !self.es.is_empty() {
                    &self.es
                } else if !self.en.is_empty() {
                    &self.en
                } else {
                    &self.fr
                }
            }
            _ => {
                if !self.fr.is_empty() {
                    &self.fr
                } else if !self.en.is_empty() {
                    &self.en
                } else if !self.es.is_empty() {
                    &self.es
                } else {
                    &self.ja
                }
            }
        }
    }
}

/// Community wiki endpoint associated with a game (EN, FR, ES, JA).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityWikiSource {
    pub lang: String,
    pub name: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub api_url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kind: String,
}

/// Modding hub / GameBanana / Inazuma Mods reference associated with a game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModHubRef {
    pub platform: String,
    pub name: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gamebanana_id: Option<u32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub api_url: String,
}

/// Internal wiki route link attached to a game entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameLink {
    pub href: String,
    #[serde(default)]
    pub label: LocalizedText,
}

/// Saga timeline era group metadata stored in `games.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EraInfo {
    pub id: String,
    #[serde(default)]
    pub label: LocalizedText,
    #[serde(default)]
    pub description: LocalizedText,
}

/// Per-game manifest (unified single source of truth for Rust, C23, OpenAPI, CLI, TUI, GUI, Web).
/// Note: scalar fields precede table/array fields so `toml::to_string` always roundtrips cleanly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub slug: String,
    #[serde(default)]
    pub order: u32,
    pub title: String,
    pub series: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub era: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub status: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Bumped whenever the manifest changes; drives `update`.
    pub revision: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub release_year: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_date_ja: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_date_eu: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub protagonist: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub developer: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub thumbnail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub official_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_filter_key: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_series_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub platforms_display: Vec<String>,
    #[serde(default)]
    pub titles: LocalizedText,
    #[serde(default)]
    pub status_detail: LocalizedText,
    #[serde(default)]
    pub summaries: LocalizedText,
    #[serde(default)]
    pub highlights: LocalizedList,
    #[serde(default)]
    pub features: BTreeMap<Feature, Support>,
    pub releases: Vec<Release>,
    #[serde(default)]
    pub ecosystem: Vec<Ecosystem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub internal_links: Vec<GameLink>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub community_wikis: Vec<CommunityWikiSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mod_hubs: Vec<ModHubRef>,
}

impl Game {
    pub fn support(&self, feature: Feature) -> Support {
        self.features.get(&feature).copied().unwrap_or_default()
    }

    pub fn release(&self, platform: Platform) -> Option<&Release> {
        self.releases.iter().find(|r| r.platform == platform)
    }

    pub fn platforms(&self) -> impl Iterator<Item = Platform> + '_ {
        self.releases.iter().map(|r| r.platform)
    }

    pub fn matches(&self, key: &str) -> bool {
        self.id == key || self.aliases.iter().any(|a| a == key)
    }
}
