//! Keys, independent of any terminal or windowing library.
//!
//! The notation is yazi's: a printable character stands for itself (`j`, `G`, `/`), a named key
//! or a key with modifiers sits in angle brackets (`<Enter>`, `<C-u>`, `<S-Tab>`, `<A-x>`).

use std::fmt;

/// The key itself, without modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    /// A printable character; shifted letters arrive already upper-cased.
    Char(char),
    /// Return.
    Enter,
    /// Escape.
    Esc,
    /// Backspace.
    Backspace,
    /// Delete.
    Delete,
    /// Tab.
    Tab,
    /// The space bar.
    Space,
    /// Cursor up.
    Up,
    /// Cursor down.
    Down,
    /// Cursor left.
    Left,
    /// Cursor right.
    Right,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// Home.
    Home,
    /// End.
    End,
    /// Function key `F1`..`F12`.
    F(u8),
}

/// Modifier keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Mods {
    /// Control.
    pub ctrl: bool,
    /// Alt.
    pub alt: bool,
    /// Shift, only kept for non-character keys (a shifted letter is a different `Char`).
    pub shift: bool,
}

impl Mods {
    /// No modifier.
    pub const NONE: Self = Self {
        ctrl: false,
        alt: false,
        shift: false,
    };
    /// Control only.
    pub const CTRL: Self = Self {
        ctrl: true,
        alt: false,
        shift: false,
    };
}

/// One key press.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    /// The key.
    pub code: KeyCode,
    /// The modifiers.
    pub mods: Mods,
}

impl Key {
    /// A key without modifiers.
    #[must_use]
    pub const fn plain(code: KeyCode) -> Self {
        Self {
            code,
            mods: Mods::NONE,
        }
    }

    /// A printable character.
    #[must_use]
    pub const fn char(c: char) -> Self {
        Self::plain(KeyCode::Char(c))
    }

    /// The character to insert in a text field, if this key types one.
    #[must_use]
    pub fn typed(&self) -> Option<char> {
        if self.mods.ctrl || self.mods.alt {
            return None;
        }
        match self.code {
            KeyCode::Char(c) => Some(c),
            KeyCode::Space => Some(' '),
            _ => None,
        }
    }

    /// Parses one key of the yazi notation.
    ///
    /// # Errors
    /// Fails on an unknown name or a malformed `<...>` group.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut chars = text.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            return Ok(if c == ' ' {
                Self::plain(KeyCode::Space)
            } else {
                Self::char(c)
            });
        }
        let inner = text
            .strip_prefix('<')
            .and_then(|t| t.strip_suffix('>'))
            .ok_or_else(|| format!("`{text}` is neither one character nor `<...>`"))?;
        let mut mods = Mods::NONE;
        let mut rest = inner;
        loop {
            let lower = rest.to_ascii_lowercase();
            if lower.starts_with("c-") {
                mods.ctrl = true;
            } else if lower.starts_with("a-") {
                mods.alt = true;
            } else if lower.starts_with("s-") {
                mods.shift = true;
            } else {
                break;
            }
            rest = &rest[2..];
        }
        let code = match rest.to_ascii_lowercase().as_str() {
            "enter" | "cr" | "return" => KeyCode::Enter,
            "esc" | "escape" => KeyCode::Esc,
            "backspace" | "bs" => KeyCode::Backspace,
            "delete" | "del" => KeyCode::Delete,
            "tab" => KeyCode::Tab,
            "space" => KeyCode::Space,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            other => {
                if let Some(n) = other.strip_prefix('f').and_then(|n| n.parse::<u8>().ok())
                    && (1..=12).contains(&n)
                {
                    KeyCode::F(n)
                } else {
                    let mut it = rest.chars();
                    match (it.next(), it.next()) {
                        (Some(c), None) => KeyCode::Char(c),
                        _ => return Err(format!("unknown key `<{inner}>`")),
                    }
                }
            }
        };
        // A shifted letter is an upper-case character, as terminals report it.
        if let (KeyCode::Char(c), true) = (code, mods.shift) {
            return Ok(Self {
                code: KeyCode::Char(c.to_ascii_uppercase()),
                mods: Mods {
                    shift: false,
                    ..mods
                },
            });
        }
        Ok(Self { code, mods })
    }

    /// Parses a key sequence: `gg`, `<C-u>`, `g<Space>`.
    ///
    /// # Errors
    /// Fails when a part of the sequence is not a key.
    pub fn parse_sequence(text: &str) -> Result<Vec<Self>, String> {
        let mut keys = Vec::new();
        let mut rest = text;
        while let Some(c) = rest.chars().next() {
            if c == '<' && let Some(end) = rest.find('>') && end > 1 {
                keys.push(Self::parse(&rest[..=end])?);
                rest = &rest[end + 1..];
            } else {
                keys.push(Self::char(c));
                rest = &rest[c.len_utf8()..];
            }
        }
        if keys.is_empty() {
            return Err("empty key sequence".into());
        }
        Ok(keys)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self.code {
            KeyCode::Char(c) if !self.mods.ctrl && !self.mods.alt => return write!(f, "{c}"),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Enter => "Enter".into(),
            KeyCode::Esc => "Esc".into(),
            KeyCode::Backspace => "Backspace".into(),
            KeyCode::Delete => "Delete".into(),
            KeyCode::Tab => "Tab".into(),
            KeyCode::Space => "Space".into(),
            KeyCode::Up => "Up".into(),
            KeyCode::Down => "Down".into(),
            KeyCode::Left => "Left".into(),
            KeyCode::Right => "Right".into(),
            KeyCode::PageUp => "PageUp".into(),
            KeyCode::PageDown => "PageDown".into(),
            KeyCode::Home => "Home".into(),
            KeyCode::End => "End".into(),
            KeyCode::F(n) => format!("F{n}"),
        };
        write!(f, "<")?;
        if self.mods.ctrl {
            write!(f, "C-")?;
        }
        if self.mods.alt {
            write!(f, "A-")?;
        }
        if self.mods.shift {
            write!(f, "S-")?;
        }
        write!(f, "{name}>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_yazi_notation() {
        assert_eq!(Key::parse("j").unwrap(), Key::char('j'));
        assert_eq!(Key::parse("G").unwrap(), Key::char('G'));
        assert_eq!(
            Key::parse("<C-u>").unwrap(),
            Key {
                code: KeyCode::Char('u'),
                mods: Mods::CTRL
            }
        );
        assert_eq!(Key::parse("<Enter>").unwrap(), Key::plain(KeyCode::Enter));
        assert_eq!(Key::parse("<S-g>").unwrap(), Key::char('G'));
        assert_eq!(Key::parse("<F5>").unwrap(), Key::plain(KeyCode::F(5)));
        assert!(Key::parse("<Nope>").is_err());
    }

    #[test]
    fn parses_sequences() {
        let keys = Key::parse_sequence("g<Space>").unwrap();
        assert_eq!(keys, [Key::char('g'), Key::plain(KeyCode::Space)]);
        assert_eq!(Key::parse_sequence("gg").unwrap().len(), 2);
        assert_eq!(Key::parse_sequence("<").unwrap(), [Key::char('<')]);
    }

    #[test]
    fn displays_what_it_parses() {
        for text in ["j", "<C-u>", "<Enter>", "<A-x>", "<F2>"] {
            assert_eq!(Key::parse(text).unwrap().to_string(), text);
        }
    }
}
