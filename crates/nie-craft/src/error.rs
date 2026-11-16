use thiserror::Error;

#[derive(Debug, Error)]
pub enum CraftError {
    #[error("cannot read `{path}`: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("manifest syntax: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("manifest serialization: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error(transparent)]
    Model(#[from] nie_model::ModelError),
    #[error("manifest: {0}")]
    Invalid(String),
}
