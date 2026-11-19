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
pub mod pages;
mod tables;
#[cfg(all(feature = "testing", not(target_arch = "wasm32")))]
pub mod testing;
mod text;
mod wiki;

pub use error::{Error, Result};
pub use model::{
    AffiliatedCharacter, CatalogEntry, Character, CharacterClass, ClassCount, ClassInfo, ClassRole,
    Coded, Emblem, Form, Id, LangText, Language, LearnedSkill, Localized, NoSource, Provenance,
    RefItem, RefSlice, SeriesRef, Skill, SkillFamily, TableInfo, TableRecord, Team, TeamRef,
    UNSOURCED,
};
pub use wiki::{Page, Query, Source, Wiki};

#[cfg(feature = "crlib")]
#[allow(unsafe_code)]
mod abi_generated;
