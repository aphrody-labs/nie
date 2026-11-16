use bevy_ecs::prelude::*;
use bevy_math::Vec2;
use nie_input::InputCommand;
use nie_model::{Attributes, Role, Side};
use nie_physics::{BallState, Body};

/// Identity and ratings of a player on the pitch.
#[derive(Component, Clone, Debug)]
pub struct Footballer {
    pub side: Side,
    pub number: u8,
    pub name: String,
    pub role: Role,
    pub attributes: Attributes,
    /// Formation slot in world coordinates.
    pub anchor: Vec2,
    /// Starting position for a kickoff, inside the own half.
    pub kickoff: Vec2,
}

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct PlayerBody(pub Body);

/// The command the player acts on during the current tick.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Command(pub InputCommand);

/// Ticks left before the player may kick again.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Cooldown(pub u16);

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct BallBody(pub BallState);
