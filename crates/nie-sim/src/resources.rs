use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use nie_input::InputCommand;
use nie_model::{DetRng, Formation, ModelError, Rules, Side, Team, generate_team};
use nie_physics::{BallParams, Pitch};

/// Everything needed to start a match.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct MatchConfig {
    pub rules: Rules,
    pub home: Team,
    pub away: Team,
}

impl MatchConfig {
    /// Two generated teams, both derived from `rules.seed`.
    pub fn generated(rules: Rules) -> Self {
        let formation = Formation::balanced(rules.players_per_side);
        Self {
            home: generate_team(rules.seed, &formation),
            away: generate_team(rules.seed.wrapping_add(1), &formation),
            rules,
        }
    }

    pub fn validate(&self) -> Result<(), ModelError> {
        self.rules.validate()?;
        self.home.validate(self.rules.players_per_side)?;
        self.away.validate(self.rules.players_per_side)
    }

    pub fn team(&self, side: Side) -> &Team {
        match side {
            Side::Home => &self.home,
            Side::Away => &self.away,
        }
    }
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct PitchRes(pub Pitch);

#[derive(Resource, Clone, Copy, Debug)]
pub struct BallTuning(pub BallParams);

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Score {
    pub home: u16,
    pub away: u16,
}

impl Score {
    pub fn add(&mut self, side: Side) {
        match side {
            Side::Home => self.home += 1,
            Side::Away => self.away += 1,
        }
    }
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct Clock {
    pub tick: u32,
    pub total: u32,
    pub rate: u32,
}

impl Clock {
    pub fn seconds(&self) -> f32 {
        self.tick as f32 / self.rate as f32
    }
}

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Playing,
    /// Play is frozen after a goal until the countdown ends.
    GoalPause {
        remaining: u32,
    },
    Finished,
}

#[derive(Resource, Clone, Debug)]
pub struct SimRng(pub DetRng);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlayerKey {
    pub side: Side,
    pub number: u8,
}

/// Commands from human or remote controllers. Players without an entry are driven by the AI.
#[derive(Resource, Clone, Debug, Default)]
pub struct HumanInputs(pub BTreeMap<PlayerKey, InputCommand>);
