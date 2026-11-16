//! Player input as a small quantized command.
//!
//! Humans, AI controllers and remote peers all produce the same [`InputCommand`], so the
//! simulation has one input path and the netcode only moves five bytes per player and tick.

use serde::{Deserialize, Serialize};

/// What one player wants to do during one tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputCommand {
    /// Movement intent, -127 to 127 per axis.
    pub move_x: i8,
    pub move_y: i8,
    /// Kick direction; zero means "along the movement".
    pub aim_x: i8,
    pub aim_y: i8,
    /// Kick strength, 0 for no kick to 255 for the hardest kick.
    pub kick: u8,
}

impl InputCommand {
    pub const IDLE: Self = Self {
        move_x: 0,
        move_y: 0,
        aim_x: 0,
        aim_y: 0,
        kick: 0,
    };

    /// Quantizes analog stick values in `[-1, 1]` and a kick charge in `[0, 1]`.
    pub fn from_analog(move_axis: [f32; 2], aim_axis: [f32; 2], kick: f32) -> Self {
        let q = |v: f32| (v.clamp(-1.0, 1.0) * 127.0).round() as i8;
        Self {
            move_x: q(move_axis[0]),
            move_y: q(move_axis[1]),
            aim_x: q(aim_axis[0]),
            aim_y: q(aim_axis[1]),
            kick: (kick.clamp(0.0, 1.0) * 255.0).round() as u8,
        }
    }

    /// Movement vector with length at most 1.
    pub fn movement(&self) -> [f32; 2] {
        unit_clamped(self.move_x, self.move_y)
    }

    /// Kick direction, falling back to the movement direction. Length is at most 1.
    pub fn aim(&self) -> [f32; 2] {
        if self.aim_x == 0 && self.aim_y == 0 {
            self.movement()
        } else {
            unit_clamped(self.aim_x, self.aim_y)
        }
    }

    /// Kick strength in `[0, 1]`.
    pub fn kick_strength(&self) -> f32 {
        f32::from(self.kick) / 255.0
    }

    pub fn to_bytes(self) -> [u8; 5] {
        [
            self.move_x as u8,
            self.move_y as u8,
            self.aim_x as u8,
            self.aim_y as u8,
            self.kick,
        ]
    }

    pub fn from_bytes(b: [u8; 5]) -> Self {
        Self {
            move_x: b[0] as i8,
            move_y: b[1] as i8,
            aim_x: b[2] as i8,
            aim_y: b[3] as i8,
            kick: b[4],
        }
    }
}

fn unit_clamped(x: i8, y: i8) -> [f32; 2] {
    let (x, y) = (f32::from(x) / 127.0, f32::from(y) / 127.0);
    let len_sq = x * x + y * y;
    if len_sq > 1.0 {
        let len = len_sq.sqrt();
        [x / len, y / len]
    } else {
        [x, y]
    }
}

/// A keyboard layout, with key names as spelled by the frontend (`"KeyW"`, `"ArrowUp"`, ...).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyScheme {
    pub name: String,
    pub up: String,
    pub down: String,
    pub left: String,
    pub right: String,
    pub kick: String,
}

impl KeyScheme {
    pub fn wasd() -> Self {
        Self::of("wasd", ["KeyW", "KeyS", "KeyA", "KeyD", "Space"])
    }

    pub fn arrows() -> Self {
        Self::of(
            "arrows",
            ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Enter"],
        )
    }

    fn of(name: &str, keys: [&str; 5]) -> Self {
        let [up, down, left, right, kick] = keys.map(str::to_owned);
        Self {
            name: name.to_owned(),
            up,
            down,
            left,
            right,
            kick,
        }
    }

    /// Builds a command from the pressed state of the scheme's keys.
    pub fn command(
        &self,
        up: bool,
        down: bool,
        left: bool,
        right: bool,
        kick: bool,
    ) -> InputCommand {
        let axis = |neg: bool, pos: bool| f32::from(i8::from(pos) - i8::from(neg));
        InputCommand::from_analog(
            [axis(left, right), axis(down, up)],
            [0.0, 0.0],
            if kick { 0.8 } else { 0.0 },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_round_trip() {
        let c = InputCommand::from_analog([0.5, -1.0], [1.0, 0.2], 0.7);
        assert_eq!(InputCommand::from_bytes(c.to_bytes()), c);
    }

    #[test]
    fn diagonal_is_normalized() {
        let c = InputCommand::from_analog([1.0, 1.0], [0.0, 0.0], 0.0);
        let [x, y] = c.movement();
        assert!((x * x + y * y - 1.0).abs() < 1e-3);
    }

    #[test]
    fn aim_falls_back_to_movement() {
        let c = InputCommand::from_analog([1.0, 0.0], [0.0, 0.0], 1.0);
        assert_eq!(c.aim(), c.movement());
    }

    #[test]
    fn scheme_maps_keys() {
        let c = KeyScheme::wasd().command(false, false, false, true, true);
        assert_eq!(c.move_x, 127);
        assert!(c.kick > 0);
    }
}
