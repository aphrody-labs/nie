//! The craft layer of nie.
//!
//! A game is not a project but a job: a [`Manifest`] names a mode, the rules, an art style, the
//! two teams and a frontend, and every piece it refers to comes from the shared engine crates.
//! [`Manifest::plan`] resolves the frontend into a cargo [`Target`]; [`Manifest::match_config`]
//! resolves the sporting side into the exact configuration the simulation plays.

mod error;
mod manifest;
mod plan;

pub use error::CraftError;
pub use manifest::{
    Art, ArtStyle, CameraStyle, Controller, Frontend, GameInfo, Manifest, Mode, SCHEMA, Series,
    TeamSpec, Teams,
};
pub use plan::{Plan, Target, TargetKind};
