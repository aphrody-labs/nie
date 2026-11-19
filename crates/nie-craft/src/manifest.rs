use std::path::Path;

use nie_model::{Formation, Rules, Side, Team, generate_team};
use nie_sim::{MatchConfig, MatchReport, run_headless};
use serde::{Deserialize, Serialize};

use crate::CraftError;

/// Version of the manifest format understood by this crate.
pub const SCHEMA: u32 = 1;

/// A football game, as data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub game: GameInfo,
    #[serde(default)]
    pub rules: Rules,
    #[serde(default)]
    pub art: Art,
    pub teams: Teams,
    /// Required when `game.mode` is `series`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series: Option<Series>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameInfo {
    /// Lowercase identifier: letters, digits and dashes.
    pub id: String,
    pub name: String,
    pub version: String,
    pub mode: Mode,
    pub frontend: Frontend,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// One regulation match.
    Exhibition,
    /// Several matches in a row on consecutive seeds; the series winner has the most wins.
    Series,
}

/// Where the game runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Frontend {
    /// Native 3D game on Bevy.
    Bevy3d,
    /// Native GUI manager and editor.
    Studio,
    /// The Bevy game compiled to WebAssembly and drawn on a WebGPU canvas.
    BevyWeb,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Series {
    pub matches: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Art {
    pub style: ArtStyle,
    pub camera: CameraStyle,
    /// Pitch grass color, RGB.
    pub pitch_color: [u8; 3],
}

impl Default for Art {
    fn default() -> Self {
        Self {
            style: ArtStyle::Lowpoly,
            camera: CameraStyle::Broadcast,
            pitch_color: [46, 125, 62],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtStyle {
    /// Faceted primitives with matte shading.
    Lowpoly,
    /// Unlit flat colors.
    Flat,
    /// Glossy, saturated materials.
    Toon,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CameraStyle {
    /// Raised side view following the ball.
    Broadcast,
    /// Straight down over the pitch.
    TopDown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Teams {
    pub home: TeamSpec,
    pub away: TeamSpec,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct TeamSpec {
    /// Overrides the generated team name.
    pub name: Option<String>,
    /// Generator seed; defaults to the rules seed (home) or the rules seed plus one (away).
    pub seed: Option<u64>,
    /// Outfield lines such as `2-1-1`; defaults to a balanced formation.
    pub formation: Option<Formation>,
    pub controller: Controller,
}

impl Default for TeamSpec {
    fn default() -> Self {
        Self {
            name: None,
            seed: None,
            formation: None,
            controller: Controller::Ai,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Controller {
    Ai,
    /// Driven by the frontend input. Headless runs always use the AI.
    Human,
}

impl Manifest {
    pub fn from_toml(text: &str) -> Result<Self, CraftError> {
        let manifest: Self = toml::from_str(text)?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn load(path: &Path) -> Result<Self, CraftError> {
        let text = std::fs::read_to_string(path).map_err(|source| CraftError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_toml(&text)
    }

    pub fn to_toml(&self) -> Result<String, CraftError> {
        Ok(toml::to_string_pretty(self)?)
    }

    pub fn validate(&self) -> Result<(), CraftError> {
        let invalid = |msg: String| Err(CraftError::Invalid(msg));
        if self.schema != SCHEMA {
            return invalid(format!(
                "schema {} is not supported, expected {SCHEMA}",
                self.schema
            ));
        }
        let id = &self.game.id;
        if id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return invalid(format!(
                "game.id `{id}` must be lowercase letters, digits and dashes"
            ));
        }
        match (self.game.mode, &self.series) {
            (Mode::Series, None) => {
                return invalid("mode `series` needs a [series] table".to_owned());
            }
            (Mode::Series, Some(s)) if !(1..=21).contains(&s.matches) => {
                return invalid("series.matches must be between 1 and 21".to_owned());
            }
            (Mode::Exhibition, Some(_)) => {
                return invalid("[series] is only valid with mode `series`".to_owned());
            }
            _ => {}
        }
        self.match_config(0)?.validate()?;
        Ok(())
    }

    /// The configuration of match number `index` (0 for an exhibition). Matches of a series run
    /// on consecutive seeds, so every match is different yet fully reproducible.
    pub fn match_config(&self, index: u32) -> Result<MatchConfig, CraftError> {
        let mut rules = self.rules.clone();
        rules.seed = rules.seed.wrapping_add(u64::from(index));
        let build = |side: Side, spec: &TeamSpec, default_seed: u64| -> Team {
            let formation = spec
                .formation
                .clone()
                .unwrap_or_else(|| Formation::balanced(rules.players_per_side));
            let mut team = generate_team(spec.seed.unwrap_or(default_seed), &formation);
            if let Some(name) = &spec.name {
                team.name = name.clone();
            }
            if side == Side::Away {
                // Keep kits apart when both teams roll similar colors.
                team.colors.primary = team.colors.primary.map(|c| 255 - c / 2);
            }
            team
        };
        let config = MatchConfig {
            home: build(Side::Home, &self.teams.home, rules.seed),
            away: build(Side::Away, &self.teams.away, rules.seed.wrapping_add(1)),
            rules,
        };
        Ok(config)
    }

    /// Number of matches a run plays.
    pub fn match_count(&self) -> u32 {
        self.series.as_ref().map_or(1, |s| u32::from(s.matches))
    }

    /// Plays every match of the manifest headless, the AI controlling both teams. `ticks` caps
    /// each match; `None` plays the regulation length.
    pub fn play(&self, ticks: Option<u32>) -> Result<Vec<MatchReport>, CraftError> {
        (0..self.match_count())
            .map(|index| Ok(run_headless(self.match_config(index)?, ticks)))
            .collect()
    }

    /// Stable fingerprint of the manifest content, shared by netcode peers to prove they run
    /// the same game.
    pub fn fingerprint(&self) -> Result<u64, CraftError> {
        let canonical = self.to_toml()?;
        let mut hash = 0xCBF2_9CE4_8422_2325_u64;
        for byte in canonical.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
        }
        Ok(hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) const SAMPLE: &str = r#"
schema = 1

[game]
id = "test-cup"
name = "Test Cup"
version = "0.1.0"
mode = "exhibition"
frontend = "bevy3d"

[rules]
seed = 3

[teams.home]
controller = "human"

[teams.away]
formation = "1-2-1"
"#;

    #[test]
    fn parses_with_defaults() {
        let m = Manifest::from_toml(SAMPLE).unwrap();
        assert_eq!(m.rules.players_per_side, 5);
        assert_eq!(m.art, Art::default());
        assert_eq!(m.teams.home.controller, Controller::Human);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let text = SAMPLE.replace("[rules]", "[rules]\nunknown = 1");
        assert!(Manifest::from_toml(&text).is_err());
    }

    #[test]
    fn series_needs_its_table() {
        let text = SAMPLE.replace("exhibition", "series");
        assert!(Manifest::from_toml(&text).is_err());
        let ok = format!("{text}\n[series]\nmatches = 3\n");
        assert_eq!(Manifest::from_toml(&ok).unwrap().match_count(), 3);
    }

    #[test]
    fn wrong_formation_size_is_rejected() {
        let text = SAMPLE.replace("1-2-1", "3-3-3");
        assert!(Manifest::from_toml(&text).is_err());
    }

    #[test]
    fn toml_round_trip_keeps_fingerprint() {
        let m = Manifest::from_toml(SAMPLE).unwrap();
        let again = Manifest::from_toml(&m.to_toml().unwrap()).unwrap();
        assert_eq!(m, again);
        assert_eq!(m.fingerprint().unwrap(), again.fingerprint().unwrap());
    }

    #[test]
    fn series_matches_use_distinct_seeds() {
        let m = Manifest::from_toml(SAMPLE).unwrap();
        assert_ne!(
            m.match_config(0).unwrap().home,
            m.match_config(1).unwrap().home
        );
    }
}
