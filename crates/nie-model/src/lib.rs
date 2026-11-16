//! Data model shared by every nie crate: teams, players, formations, match rules and a
//! deterministic generator that produces original teams and players from a seed.
//!
//! The crate has no engine dependency so the manifest layer, the simulation, the frontends and
//! the network layer all agree on one vocabulary.

mod error;
mod formation;
mod generate;
mod rng;
mod rules;
mod team;

pub use error::ModelError;
pub use formation::{Formation, Slot};
pub use generate::generate_team;
pub use rng::DetRng;
pub use rules::Rules;
pub use team::{Attributes, Colors, Player, Role, Side, Team};
