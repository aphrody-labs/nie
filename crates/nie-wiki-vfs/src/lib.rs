//! Wiki pages of Inazuma Eleven: Victory Road computed from the installation's VFS only.
//!
//! Every record is a value read from a `data/common/gamedata` table plus its decoded label from
//! `data/common/text/<lang>/*.cfg.bin`, with the VFS path, table and entry index that back it
//! (see `docs/wiki.md`). A field with no source is absent; families the VFS gives no source for
//! are named by [`UNSOURCED`]. Tables are decoded with the `nie-formats` and `nie-data` readers.

mod error;
pub mod html;
mod index;
mod model;
mod text;
mod wiki;

pub use error::{Error, Result};
pub use model::{
    AffiliatedCharacter, Character, Coded, Emblem, Form, Id, LangText, Language, LearnedSkill,
    Localized, NoSource, Provenance, Skill, Team, TeamRef, UNSOURCED,
};
pub use wiki::{Page, Query, Source, Wiki};
