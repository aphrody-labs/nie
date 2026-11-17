//! IEVR hissatsu cut-in metadata and served-asset checks.
//!
//! The JSON files are generated from the game VFS. Parsing and lookup are
//! implemented here in Rust so callers do not import the Azalee TypeScript package.

use std::string::String;
use std::vec::Vec;

/// Localized name triplet used by a skill cut-in entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(
    any(feature = "serde", feature = "data"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct LocalizedSkillName {
    /// French label.
    pub fr: String,
    /// English label.
    pub en: String,
    /// Japanese label.
    pub ja: String,
}

/// VFS assets associated with one hissatsu cut-in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(
    any(feature = "serde", feature = "data"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct SkillCutin {
    /// Event identifier used by the model and effects paths.
    pub event_id_name: String,
    /// G4MG model path.
    pub model_g4mg: String,
    /// G4MD model path.
    pub model_g4md: String,
    /// G4TX texture path.
    pub texture_g4tx: String,
    /// Sound configuration path.
    pub sound_cfg: String,
    /// Effects directory.
    pub effects_dir: String,
    /// Language/path pairs for telop textures.
    pub telop_by_lang: Vec<Vec<String>>,
}

/// Skill identity and gameplay values enriched with optional cut-in assets.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(
    any(feature = "serde", feature = "data"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct SkillCutinEntry {
    /// Internal skill code, such as `whs01230`.
    pub skill_id_str: String,
    /// Hexadecimal skill identifier.
    pub skill_id: String,
    /// Event identifier name.
    pub event_id_name: String,
    /// Raw element identifier.
    pub element: i64,
    /// Localized element label.
    pub element_name: LocalizedSkillName,
    /// Raw category identifier.
    pub category: i64,
    /// Localized category label.
    pub category_name: LocalizedSkillName,
    /// Minimum power.
    pub power_min: i64,
    /// Maximum power.
    pub power_max: i64,
    /// TP cost.
    pub consume_tp: i64,
    /// Foul rate.
    pub foul_rate: i64,
    /// Growth type.
    pub growth_type: i64,
    /// Recast time.
    pub recast_time: i64,
    /// Partner type.
    pub partner_type: i64,
    /// Optional cut-in assets.
    pub cutin: Option<SkillCutin>,
}

/// Parsed cut-in catalog with code and served-event lookup.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillCutinCatalog {
    /// Number declared by the generated source metadata.
    pub skill_count: usize,
    /// Number of entries that carry cut-in metadata.
    pub cutin_count: usize,
    /// All generated skill entries in source order.
    pub skills: Vec<SkillCutinEntry>,
    served_events: Vec<String>,
}

impl SkillCutinCatalog {
    /// Creates a catalog from already parsed entries and served event names.
    #[must_use]
    pub fn new(
        skill_count: usize,
        cutin_count: usize,
        skills: Vec<SkillCutinEntry>,
        served_events: Vec<String>,
    ) -> Self {
        Self {
            skill_count,
            cutin_count,
            skills,
            served_events,
        }
    }

    /// Finds an entry by its exact internal code.
    #[must_use]
    pub fn get_skill_cutin(&self, code: &str) -> Option<&SkillCutinEntry> {
        self.skills.iter().find(|skill| skill.skill_id_str == code)
    }

    /// Returns whether a model and texture are present for an event.
    #[must_use]
    pub fn cutin_has_assets(&self, event_id_name: Option<&str>) -> bool {
        event_id_name.is_some_and(|event| {
            !event.is_empty() && self.served_events.iter().any(|served| served == event)
        })
    }

    /// Returns the served event names in generated source order.
    #[must_use]
    pub fn served_events(&self) -> &[String] {
        &self.served_events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_is_exact_and_empty_events_are_not_served() {
        let skill = SkillCutinEntry {
            skill_id_str: "whs01230".into(),
            ..SkillCutinEntry::default()
        };
        let catalog = SkillCutinCatalog::new(1, 1, vec![skill], vec!["ev60_00010".into()]);
        assert!(catalog.get_skill_cutin("whs01230").is_some());
        assert!(catalog.get_skill_cutin("WHS01230").is_none());
        assert!(!catalog.cutin_has_assets(None));
        assert!(!catalog.cutin_has_assets(Some("")));
        assert!(catalog.cutin_has_assets(Some("ev60_00010")));
    }
}
