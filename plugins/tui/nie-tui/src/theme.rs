//! Colours of the semantic styles. A terminal theme would replace this table; the model only
//! knows [`nie_explore::fm::preview::Style`].

use nie_explore::fm::preview::Style as Semantic;
use ratatui::style::{Color, Modifier, Style};

/// The ratatui style of a semantic style.
#[must_use]
pub fn style(semantic: Semantic) -> Style {
    let fg = |r, g, b| Style::new().fg(Color::Rgb(r, g, b));
    match semantic {
        Semantic::Plain => Style::new(),
        Semantic::Keyword => fg(0xc5, 0x86, 0xc0).add_modifier(Modifier::BOLD),
        Semantic::String => fg(0xce, 0x91, 0x78),
        Semantic::Number => fg(0xb5, 0xce, 0xa8),
        Semantic::Constant => fg(0x56, 0x9c, 0xd6),
        Semantic::Comment => fg(0x6a, 0x99, 0x55).add_modifier(Modifier::ITALIC),
        Semantic::Function => fg(0xdc, 0xdc, 0xaa),
        Semantic::Type => fg(0x4e, 0xc9, 0xb0),
        Semantic::Property => fg(0x9c, 0xdc, 0xfe),
        Semantic::Operator => fg(0xd4, 0xd4, 0xd4),
        Semantic::Punctuation => fg(0x80, 0x80, 0x80),
        Semantic::Label => fg(0xd7, 0xba, 0x7d),
        Semantic::Dim => Style::new().add_modifier(Modifier::DIM),
        Semantic::Header => Style::new().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        Semantic::Accent => fg(0x4f, 0xc1, 0xff).add_modifier(Modifier::BOLD),
        Semantic::Error => fg(0xf4, 0x47, 0x47),
    }
}

/// The cursor row.
#[must_use]
pub fn cursor() -> Style {
    Style::new().bg(Color::Rgb(0x26, 0x4f, 0x78)).add_modifier(Modifier::BOLD)
}

/// A directory name.
#[must_use]
pub fn directory() -> Style {
    Style::new().fg(Color::Rgb(0x56, 0x9c, 0xd6)).add_modifier(Modifier::BOLD)
}

/// A marked entry.
#[must_use]
pub fn marked() -> Style {
    Style::new().fg(Color::Rgb(0xd7, 0xba, 0x7d))
}

/// A border.
#[must_use]
pub fn border() -> Style {
    Style::new().fg(Color::Rgb(0x58, 0x58, 0x58))
}

/// The status bar.
#[must_use]
pub fn bar() -> Style {
    Style::new().bg(Color::Rgb(0x1e, 0x1e, 0x1e)).fg(Color::Rgb(0xcc, 0xcc, 0xcc))
}
