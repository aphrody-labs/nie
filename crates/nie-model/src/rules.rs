use serde::{Deserialize, Serialize};

use crate::ModelError;

/// Match rules and pitch geometry. Lengths are in meters, time in seconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rules {
    /// Players per side, goalkeeper included.
    pub players_per_side: u8,
    /// Simulation ticks per second.
    pub tick_rate: u32,
    /// Regulation length of the match.
    pub match_seconds: u32,
    pub pitch_length: f32,
    pub pitch_width: f32,
    pub goal_width: f32,
    /// Seed of the match random stream (kick accuracy, kickoff side).
    pub seed: u64,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            players_per_side: 5,
            tick_rate: 60,
            match_seconds: 60,
            pitch_length: 40.0,
            pitch_width: 24.0,
            goal_width: 6.0,
            seed: 1,
        }
    }
}

impl Rules {
    pub const fn total_ticks(&self) -> u32 {
        self.tick_rate * self.match_seconds
    }

    pub fn validate(&self) -> Result<(), ModelError> {
        let bad = |msg: &str| Err(ModelError::Rules(msg.to_owned()));
        if !(2..=11).contains(&self.players_per_side) {
            return bad("players_per_side must be between 2 and 11");
        }
        if !(10..=240).contains(&self.tick_rate) {
            return bad("tick_rate must be between 10 and 240");
        }
        if self.match_seconds == 0 {
            return bad("match_seconds must be positive");
        }
        if !(self.pitch_length.is_finite()
            && self.pitch_width.is_finite()
            && self.goal_width.is_finite())
            || self.pitch_length < 20.0
            || self.pitch_width < 12.0
            || self.pitch_length < self.pitch_width
        {
            return bad("pitch must be at least 20 x 12 and longer than wide");
        }
        if self.goal_width <= 1.0 || self.goal_width >= self.pitch_width * 0.6 {
            return bad("goal_width must be between 1 m and 60% of the pitch width");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        assert_eq!(Rules::default().validate(), Ok(()));
        assert_eq!(Rules::default().total_ticks(), 3600);
    }

    #[test]
    fn rejects_nonsense() {
        let rules = Rules {
            pitch_width: 50.0,
            ..Rules::default()
        };
        assert!(rules.validate().is_err());
    }
}
