//! A terminal surface: a grid of styled cells drawn in a monospace font, with the `Terminal` role
//! whose value is the visible text, and keyboard input turned into the bytes a PTY expects.
//!
//! The grid is plain data ([`TermGrid`]). With the `rio` feature [`rio::grid_of`] fills it from a
//! `rio-vt` `Crosswords`, the VT state machine and grid of Rio, so `nie term` keeps rio-vt as
//! its parser while GPUI draws.

use std::rc::Rc;

use gpui::{
    Context, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, Role, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::*, transparent_black,
};

use crate::a11y::Settings;
use crate::theme::Rgb;

/// One cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TermCell {
    /// The character; a space for an empty cell.
    pub ch: char,
    /// Foreground, or the theme text colour.
    pub fg: Option<Rgb>,
    /// Background, or none.
    pub bg: Option<Rgb>,
    /// Bold.
    pub bold: bool,
    /// Underlined.
    pub underline: bool,
}

impl Default for TermCell {
    fn default() -> Self {
        Self { ch: ' ', fg: None, bg: None, bold: false, underline: false }
    }
}

/// The visible screen of a terminal.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TermGrid {
    /// Rows of cells, all of the same width.
    pub rows: Vec<Vec<TermCell>>,
    /// Cursor as (row, column), when visible.
    pub cursor: Option<(usize, usize)>,
    /// The window title the program set.
    pub title: String,
}

impl TermGrid {
    /// A blank grid.
    #[must_use]
    pub fn blank(cols: usize, rows: usize) -> Self {
        Self { rows: vec![vec![TermCell::default(); cols]; rows], cursor: None, title: String::new() }
    }

    /// The visible text, one line per row, trailing spaces trimmed.
    #[must_use]
    pub fn text(&self) -> String {
        self.rows
            .iter()
            .map(|r| r.iter().map(|c| c.ch).collect::<String>().trim_end().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// The bytes a PTY expects for a key press, or `None` when the key types nothing.
#[must_use]
pub fn key_bytes(key: &str, key_char: Option<&str>, ctrl: bool, alt: bool) -> Option<Vec<u8>> {
    let named: Option<&[u8]> = match key {
        "enter" => Some(b"\r"),
        "backspace" => Some(b"\x7f"),
        "tab" => Some(b"\t"),
        "escape" => Some(b"\x1b"),
        "up" => Some(b"\x1b[A"),
        "down" => Some(b"\x1b[B"),
        "right" => Some(b"\x1b[C"),
        "left" => Some(b"\x1b[D"),
        "home" => Some(b"\x1b[H"),
        "end" => Some(b"\x1b[F"),
        "pageup" => Some(b"\x1b[5~"),
        "pagedown" => Some(b"\x1b[6~"),
        "delete" => Some(b"\x1b[3~"),
        _ => None,
    };
    if let Some(bytes) = named {
        return Some(bytes.to_vec());
    }
    if ctrl {
        let c = key.chars().next().filter(|_| key.chars().count() == 1)?;
        return c.is_ascii_alphabetic().then(|| vec![(c.to_ascii_lowercase() as u8) - b'a' + 1]);
    }
    let text = key_char?;
    let mut bytes = Vec::new();
    if alt {
        bytes.push(0x1b);
    }
    bytes.extend_from_slice(text.as_bytes());
    Some(bytes)
}

type OnInput = Rc<dyn Fn(&[u8])>;

/// The terminal view.
pub struct TerminalView {
    grid: TermGrid,
    label: SharedString,
    on_input: Option<OnInput>,
    focus: FocusHandle,
}

impl TerminalView {
    /// A terminal named `label` for assistive technology.
    pub fn new(label: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self { grid: TermGrid::default(), label: label.into(), on_input: None, focus: cx.focus_handle() }
    }

    /// Called with the bytes of every key press.
    pub fn on_input(&mut self, f: impl Fn(&[u8]) + 'static) {
        self.on_input = Some(Rc::new(f));
    }

    /// Shows a new screen.
    pub fn set_grid(&mut self, grid: TermGrid, cx: &mut Context<Self>) {
        self.grid = grid;
        cx.notify();
    }

    /// The focus handle.
    #[must_use]
    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }

    /// The grid being shown.
    #[must_use]
    pub fn grid(&self) -> &TermGrid {
        &self.grid
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Settings::theme(cx).clone();
        let cursor = self.grid.cursor;
        let name = if self.grid.title.is_empty() {
            self.label.clone()
        } else {
            SharedString::from(format!("{}, {}", self.label, self.grid.title))
        };
        div()
            .id("nie-terminal")
            .role(Role::Terminal)
            .aria_label(name)
            .aria_value(SharedString::from(self.grid.text()))
            .track_focus(&self.focus)
            .tab_stop(true)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, _cx| {
                let ks = &event.keystroke;
                if let (Some(f), Some(bytes)) = (
                    this.on_input.as_ref(),
                    key_bytes(&ks.key, ks.key_char.as_deref(), ks.modifiers.control, ks.modifiers.alt),
                ) {
                    f(&bytes);
                }
            }))
            .size_full()
            .bg(theme.background.hsla())
            .text_color(theme.text.hsla())
            .font_family(theme.mono_font)
            .text_size(theme.px(theme.text_size))
            .border_2()
            .border_color(transparent_black())
            .focus_visible(|s| s.border_color(theme.focus.hsla()))
            .overflow_hidden()
            .children(self.grid.rows.iter().enumerate().map(|(r, row)| {
                let mut spans: Vec<(TermCell, String)> = Vec::new();
                for (c, cell) in row.iter().enumerate() {
                    let at_cursor = cursor == Some((r, c));
                    let mut cell = *cell;
                    if at_cursor {
                        cell.bg = Some(theme.text);
                        cell.fg = Some(theme.background);
                    }
                    let ch = if cell.ch == ' ' { '\u{a0}' } else { cell.ch };
                    match spans.last_mut() {
                        Some((s, text))
                            if s.fg == cell.fg && s.bg == cell.bg && s.bold == cell.bold && s.underline == cell.underline =>
                        {
                            text.push(ch);
                        }
                        _ => spans.push((cell, ch.to_string())),
                    }
                }
                div().flex().children(spans.into_iter().map(|(cell, text)| {
                    div()
                        .text_color(cell.fg.unwrap_or(theme.text).hsla())
                        .when_some(cell.bg, |d, bg| d.bg(bg.hsla()))
                        .when(cell.bold, |d| d.font_weight(gpui::FontWeight::BOLD))
                        .when(cell.underline, |d| d.underline())
                        .child(SharedString::from(text))
                }))
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_become_pty_bytes() {
        assert_eq!(key_bytes("enter", None, false, false), Some(b"\r".to_vec()));
        assert_eq!(key_bytes("up", None, false, false), Some(b"\x1b[A".to_vec()));
        assert_eq!(key_bytes("c", Some("c"), true, false), Some(vec![3]));
        assert_eq!(key_bytes("a", Some("a"), false, true), Some(b"\x1ba".to_vec()));
        assert_eq!(key_bytes("shift", None, false, false), None);
    }

    #[test]
    fn the_grid_text_trims_trailing_spaces() {
        let mut g = TermGrid::blank(5, 2);
        g.rows[0][0].ch = 'h';
        g.rows[0][1].ch = 'i';
        assert_eq!(g.text(), "hi\n");
    }
}
