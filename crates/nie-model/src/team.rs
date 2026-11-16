use serde::{Deserialize, Serialize};

use crate::{Formation, ModelError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Home,
    Away,
}

impl Side {
    pub const BOTH: [Side; 2] = [Side::Home, Side::Away];

    pub const fn opponent(self) -> Self {
        match self {
            Side::Home => Side::Away,
            Side::Away => Side::Home,
        }
    }

    /// Direction of attack along the pitch x axis: the home side attacks towards +x.
    pub const fn attack_dir(self) -> f32 {
        match self {
            Side::Home => 1.0,
            Side::Away => -1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Goalkeeper,
    Defender,
    Midfielder,
    Forward,
}

/// Skill ratings from 1 to 100.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attributes {
    pub pace: u8,
    pub power: u8,
    pub control: u8,
    pub vision: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    pub number: u8,
    pub name: String,
    pub role: Role,
    pub attributes: Attributes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Colors {
    pub primary: [u8; 3],
    pub secondary: [u8; 3],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Team {
    pub name: String,
    pub colors: Colors,
    pub formation: Formation,
    /// Starting players in formation slot order (goalkeeper first).
    pub players: Vec<Player>,
}

impl Team {
    pub fn validate(&self, players_per_side: u8) -> Result<(), ModelError> {
        let fail = |reason: String| ModelError::Team {
            team: self.name.clone(),
            reason,
        };
        self.formation.check_size(players_per_side)?;
        if self.players.len() != usize::from(players_per_side) {
            return Err(fail(format!(
                "has {} players, expected {players_per_side}",
                self.players.len()
            )));
        }
        let keepers = self
            .players
            .iter()
            .filter(|p| p.role == Role::Goalkeeper)
            .count();
        if keepers != 1 || self.players[0].role != Role::Goalkeeper {
            return Err(fail(
                "needs exactly one goalkeeper, listed first".to_owned(),
            ));
        }
        let mut numbers: Vec<u8> = self.players.iter().map(|p| p.number).collect();
        numbers.sort_unstable();
        numbers.dedup();
        if numbers.len() != self.players.len() {
            return Err(fail("shirt numbers must be unique".to_owned()));
        }
        Ok(())
    }
}
