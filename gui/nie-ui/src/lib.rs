//! The GPUI toolkit of nie: the role GTK has in GNOME and Qt in KDE.
//!
//! - [`theme`]: colours derived from the game's own UI (`FONT_COLOR` of the VFS).
//! - [`a11y`]: the reader's preferences (high contrast, scale, reduced motion) and the audit of the
//!   AccessKit tree, so that every node a screen reader announces has a name.
//! - Components: [`tree`], [`table`], [`tabs`], [`picker`] (nucleo), [`surface`] (image and frame
//!   sources such as video and the 3D viewer).
//!
//! GPUI is the path dependency on `vendor/zed` (see `gui/Cargo.toml`).

pub mod a11y;
pub mod explorer;
pub mod picker;
pub mod surface;
pub mod table;
pub mod tabs;
pub mod terminal;
pub mod theme;
pub mod tree;

pub use a11y::Settings;
pub use theme::{Preferences, Rgb, Syntax, Theme};

/// Everything an app needs to draw with the toolkit, in one import.
pub mod prelude {
    pub use crate::a11y::{Report, Settings, audit};
    pub use crate::explorer::{Backend, Explorer, Frame};
    pub use crate::picker::{Picker, PickerEvent};
    pub use crate::surface::{FrameSource, FrameSurface, ImageSurface, Rgba8, render_image};
    pub use crate::table::{Table, TableEvent};
    pub use crate::tabs::Tabs;
    pub use crate::terminal::{TermCell, TermGrid, TerminalView};
    pub use crate::theme::{Preferences, Rgb, Syntax, Theme};
    pub use crate::tree::{Tree, TreeEvent, TreeRow};
}
