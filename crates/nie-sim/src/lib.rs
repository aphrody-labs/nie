//! Football match simulation on Bevy ECS.
//!
//! [`MatchPlugin`] puts a match into a Bevy [`App`]: one ball entity and one entity per
//! footballer, advanced one fixed tick at a time by the [`SimTick`] schedule. The schedule has
//! no dependency on wall-clock time or rendering, so the same plugin runs
//!
//! - headless in CI through [`run_headless`], with a score and a checksum that must never change,
//! - inside the Bevy 3D game, driven from a fixed timestep,
//! - inside lockstep netplay, where only inputs cross the wire.

mod components;
mod plugin;
mod report;
mod resources;
mod systems;

pub use components::{BallBody, Command, Cooldown, Footballer, PlayerBody};
pub use plugin::{MatchPlugin, SimTick, run_headless};
pub use report::{GoalEvent, MatchReport, checksum};
pub use resources::{Clock, HumanInputs, MatchConfig, Phase, PitchRes, PlayerKey, Score, SimRng};
