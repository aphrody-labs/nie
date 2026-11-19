//! Maps rio-window keyboard events to the window-system-independent events of the terminal model.

use nie_explore::term::{KeyCode, KeyEvent, KeyState, Mods};
use rio_window::event::{ElementState, KeyEvent as WinKey};
use rio_window::keyboard::{Key, ModifiersState, NamedKey};
use rio_window::platform::modifier_supplement::KeyEventExtModifierSupplement;

/// The modifiers as the model knows them.
pub fn mods(state: ModifiersState) -> Mods {
    Mods {
        shift: state.shift_key(),
        alt: state.alt_key(),
        ctrl: state.control_key(),
        logo: state.super_key(),
    }
}

fn named(key: NamedKey) -> Option<KeyCode> {
    Some(match key {
        NamedKey::Enter => KeyCode::Enter,
        NamedKey::Tab => KeyCode::Tab,
        NamedKey::Backspace => KeyCode::Backspace,
        NamedKey::Escape => KeyCode::Escape,
        NamedKey::ArrowUp => KeyCode::Up,
        NamedKey::ArrowDown => KeyCode::Down,
        NamedKey::ArrowLeft => KeyCode::Left,
        NamedKey::ArrowRight => KeyCode::Right,
        NamedKey::Home => KeyCode::Home,
        NamedKey::End => KeyCode::End,
        NamedKey::PageUp => KeyCode::PageUp,
        NamedKey::PageDown => KeyCode::PageDown,
        NamedKey::Insert => KeyCode::Insert,
        NamedKey::Delete => KeyCode::Delete,
        NamedKey::Space => KeyCode::Char(' '),
        NamedKey::F1 => KeyCode::F(1),
        NamedKey::F2 => KeyCode::F(2),
        NamedKey::F3 => KeyCode::F(3),
        NamedKey::F4 => KeyCode::F(4),
        NamedKey::F5 => KeyCode::F(5),
        NamedKey::F6 => KeyCode::F(6),
        NamedKey::F7 => KeyCode::F(7),
        NamedKey::F8 => KeyCode::F(8),
        NamedKey::F9 => KeyCode::F(9),
        NamedKey::F10 => KeyCode::F(10),
        NamedKey::F11 => KeyCode::F(11),
        NamedKey::F12 => KeyCode::F(12),
        _ => return None,
    })
}

/// The model event of a window key event, `None` for keys a terminal never sees (modifiers alone,
/// media keys, dead keys).
pub fn key_event(ev: &WinKey, mods: Mods) -> Option<KeyEvent> {
    let state = match (ev.state, ev.repeat) {
        (ElementState::Released, _) => KeyState::Release,
        (ElementState::Pressed, true) => KeyState::Repeat,
        (ElementState::Pressed, false) => KeyState::Press,
    };
    let code = match &ev.logical_key {
        Key::Named(key) => named(*key)?,
        Key::Character(text) => {
            // The base key: what the key types without modifiers.
            let base = match ev.key_without_modifiers() {
                Key::Character(c) => c.chars().next(),
                _ => text.chars().next(),
            };
            KeyCode::Char(base?)
        }
        _ => return None,
    };
    let text = ev.text.as_ref().map(|t| t.to_string()).filter(|t| !t.is_empty());
    Some(KeyEvent {
        code,
        mods,
        state,
        text,
    })
}
