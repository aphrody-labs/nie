use thiserror::Error;

/// Validation and parsing failures of the data model.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModelError {
    #[error("invalid formation `{0}`: expected dash separated line sizes such as `2-1-1`")]
    Formation(String),
    #[error(
        "formation `{formation}` fields {fielded} outfield players but the match needs {expected}"
    )]
    FormationSize {
        formation: String,
        fielded: u8,
        expected: u8,
    },
    #[error("rules: {0}")]
    Rules(String),
    #[error("team `{team}`: {reason}")]
    Team { team: String, reason: String },
}
