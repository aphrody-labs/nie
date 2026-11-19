use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("mounting the game source: {0}")]
    Mount(String),
    #[error("no VFS file named {stem}_<version>.cfg.bin under {dir}")]
    TableMissing { stem: String, dir: &'static str },
    #[error("decoding {path}: {message}")]
    Decode { path: String, message: String },
    #[error("the VFS holds no text language under data/common/text")]
    NoLanguages,
    #[error("language `{0}` is not shipped by this installation")]
    UnknownLanguage(String),
}

pub type Result<T> = std::result::Result<T, Error>;
