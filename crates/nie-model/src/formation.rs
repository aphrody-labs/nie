use serde::{Deserialize, Serialize};

use crate::{ModelError, Role};

/// Outfield lines from defence to attack, for example `2-1-1`. The goalkeeper is implicit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Formation {
    lines: Vec<u8>,
}

/// A position in a formation, in the attacking frame of its team: `x` runs from the own goal
/// (0) to the opposing goal (1), `y` from the left (-1) to the right (1) touchline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    pub role: Role,
    pub x: f32,
    pub y: f32,
}

impl Formation {
    pub fn new(lines: Vec<u8>) -> Result<Self, ModelError> {
        if lines.is_empty() || lines.len() > 5 || lines.iter().any(|&n| n == 0 || n > 6) {
            return Err(ModelError::Formation(Self::render(&lines)));
        }
        Ok(Self { lines })
    }

    /// The balanced default for a given squad size: as even as possible, defence first.
    pub fn balanced(players_per_side: u8) -> Self {
        let outfield = players_per_side.saturating_sub(1).max(1);
        let lines = match outfield {
            1 => vec![1],
            2 => vec![1, 1],
            3 => vec![1, 1, 1],
            n => {
                let defence = n / 3 + u8::from(n % 3 > 0);
                let attack = (n - defence) / 2;
                vec![defence, n - defence - attack, attack]
            }
        };
        Self { lines }
    }

    pub fn outfield(&self) -> u8 {
        self.lines.iter().sum()
    }

    pub fn lines(&self) -> &[u8] {
        &self.lines
    }

    pub fn check_size(&self, players_per_side: u8) -> Result<(), ModelError> {
        let expected = players_per_side - 1;
        if self.outfield() == expected {
            Ok(())
        } else {
            Err(ModelError::FormationSize {
                formation: self.to_string(),
                fielded: self.outfield(),
                expected,
            })
        }
    }

    /// Slots in a stable order: goalkeeper first, then defence to attack, left to right.
    pub fn slots(&self) -> Vec<Slot> {
        let mut slots = vec![Slot {
            role: Role::Goalkeeper,
            x: 0.04,
            y: 0.0,
        }];
        let count = self.lines.len();
        for (index, &width) in self.lines.iter().enumerate() {
            let role = match (index, count) {
                (0, _) => Role::Defender,
                (i, c) if i + 1 == c => Role::Forward,
                _ => Role::Midfielder,
            };
            let x = 0.18 + 0.62 * (index as f32 / (count - 1).max(1) as f32);
            for k in 0..width {
                let y = if width == 1 {
                    0.0
                } else {
                    -0.7 + 1.4 * (f32::from(k) / f32::from(width - 1))
                };
                slots.push(Slot { role, x, y });
            }
        }
        slots
    }

    fn render(lines: &[u8]) -> String {
        lines
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join("-")
    }
}

impl std::fmt::Display for Formation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&Self::render(&self.lines))
    }
}

impl TryFrom<String> for Formation {
    type Error = ModelError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        let lines: Result<Vec<u8>, _> = text.split('-').map(str::parse::<u8>).collect();
        match lines {
            Ok(lines) => Self::new(lines),
            Err(_) => Err(ModelError::Formation(text)),
        }
    }
}

impl From<Formation> for String {
    fn from(formation: Formation) -> Self {
        formation.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_counts() {
        let f = Formation::try_from("2-1-1".to_owned()).unwrap();
        assert_eq!(f.outfield(), 4);
        assert_eq!(f.slots().len(), 5);
        assert_eq!(f.to_string(), "2-1-1");
        assert!(f.check_size(5).is_ok());
        assert!(f.check_size(7).is_err());
    }

    #[test]
    fn rejects_garbage() {
        assert!(Formation::try_from("x-1".to_owned()).is_err());
        assert!(Formation::try_from("0-4".to_owned()).is_err());
    }

    #[test]
    fn balanced_matches_squad_size() {
        for n in 2..=11 {
            assert_eq!(Formation::balanced(n).outfield(), n - 1);
        }
    }
}
