//! The renderer-independent model of the nie file explorer (`nie fm`).
//!
//! Everything here is plain data and pure functions: the VFS tree, the keymap, the fuzzy search,
//! the navigation state machine, the previewers and the BLAKE3 thumbnail cache. A renderer (the
//! ratatui one of `nie-tui`, a GPU window, the GPUI twin) only draws a [`model::View`] and a
//! [`preview::Preview`] and translates its input events into [`keys::Key`]s. The model never
//! touches a terminal, a window or an async runtime.
//!
//! Designs cited (no code copied): yazi's three-column manager, its key notation and its async
//! preview pipeline (MIT, sxyazi/yazi); tree-sitter-highlight colours the read-only
//! view and `nucleo` picker (MPL-2.0, taken as crates.io dependencies); spacedrive's
//! content-addressed thumbnail cache `pvcache` (Apache-2.0). See `docs/specs/terminal-sources.md`.

#[cfg(any(test, feature = "fm-fixture"))]
pub mod fixture;
pub mod highlight;
pub mod keymap;
pub mod keys;
pub mod model;
pub mod preview;
pub mod search;
pub mod thumbs;
pub mod tree;

pub use keymap::{Action, Keymap};
pub use keys::{Key, KeyCode, Mods};
pub use model::{Fm, View};
pub use preview::{Preview, PreviewRequest};
pub use tree::{NodeId, NodeKind, TreeMode, VfsTree};
