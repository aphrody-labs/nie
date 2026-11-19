//! Key, mouse, paste and focus encoding: the bytes a child expects on its standard input.
//!
//! The encoders are pure functions of an event and of the [`InputModes`] the terminal state
//! machine currently has, so a window system maps its native events to [`KeyEvent`] and
//! [`MouseEvent`] and writes the returned bytes to the pty. Legacy xterm sequences are produced by
//! default; once the child enables the kitty keyboard protocol (`CSI > flags u`) the `CSI u` forms
//! of that protocol are produced instead.

/// The input-relevant modes of the terminal, read from the VT state machine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputModes {
    /// DECCKM: arrows, Home and End send `SS3` instead of `CSI`.
    pub app_cursor: bool,
    /// Kitty keyboard protocol flags (bit 0 disambiguate, 1 event types, 2 alternate keys, 3 all
    /// keys as escape codes, 4 associated text).
    pub kitty_flags: u8,
    /// Mouse reporting level.
    pub mouse: MouseMode,
    /// SGR (1006) mouse encoding.
    pub sgr_mouse: bool,
    /// UTF-8 (1005) mouse encoding.
    pub utf8_mouse: bool,
    /// Bracketed paste (2004).
    pub bracketed_paste: bool,
    /// Focus in and out reporting (1004).
    pub focus_events: bool,
    /// The alternate screen is active.
    pub alt_screen: bool,
    /// The wheel sends arrow keys on the alternate screen (1007).
    pub alternate_scroll: bool,
    /// LNM: Enter sends CR LF.
    pub newline_mode: bool,
}

/// How much of the mouse the child wants to see.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MouseMode {
    /// The terminal owns the mouse.
    #[default]
    Off,
    /// X10 (9): presses only.
    X10,
    /// Normal tracking (1000): presses, releases and the wheel.
    Click,
    /// Button-event tracking (1002): adds motion while a button is held.
    Drag,
    /// Any-event tracking (1003): adds all motion.
    Motion,
}

/// A key independent of any window system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    /// A character key, as the unshifted base character of the key.
    Char(char),
    Enter,
    Tab,
    Backspace,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    /// Function key `F1` to `F35`.
    F(u8),
}

/// Modifier keys.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Mods {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
    pub logo: bool,
}

impl Mods {
    /// No modifier.
    pub const NONE: Mods = Mods {
        shift: false,
        alt: false,
        ctrl: false,
        logo: false,
    };

    /// The xterm modifier parameter minus one (shift 1, alt 2, ctrl 4, super 8).
    pub fn bits(self) -> u8 {
        u8::from(self.shift) | u8::from(self.alt) << 1 | u8::from(self.ctrl) << 2 | u8::from(self.logo) << 3
    }

    /// Whether no modifier is down.
    pub fn is_empty(self) -> bool {
        self.bits() == 0
    }
}

/// Press, auto-repeat or release.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeyState {
    #[default]
    Press,
    Repeat,
    Release,
}

/// One key event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub mods: Mods,
    pub state: KeyState,
    /// The text the key produces with the active layout and modifiers, when it produces any.
    pub text: Option<String>,
}

impl KeyEvent {
    /// A key press without text.
    pub fn press(code: KeyCode, mods: Mods) -> Self {
        Self {
            code,
            mods,
            state: KeyState::Press,
            text: None,
        }
    }

    /// A character key press whose text is the character itself.
    pub fn text(c: char, mods: Mods) -> Self {
        Self {
            code: KeyCode::Char(c),
            mods,
            state: KeyState::Press,
            text: Some(c.to_string()),
        }
    }
}

/// A mouse button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

/// What the mouse did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseKind {
    Press(MouseButton),
    Release(MouseButton),
    /// Motion, with the button held when there is one.
    Move(Option<MouseButton>),
    /// Wheel: positive `lines` scrolls up (away from the user), negative down.
    Wheel { lines: i32 },
    /// Horizontal wheel: positive scrolls left.
    WheelH { lines: i32 },
}

/// One mouse event at a cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseEvent {
    pub kind: MouseKind,
    /// Zero-based column.
    pub col: usize,
    /// Zero-based row of the viewport.
    pub row: usize,
    pub mods: Mods,
}

const ESC: u8 = 0x1b;

fn mod_param(mods: Mods) -> u8 {
    1 + mods.bits()
}

fn csi(out: &mut Vec<u8>, body: &str) {
    out.extend_from_slice(&[ESC, b'[']);
    out.extend_from_slice(body.as_bytes());
}

/// The final byte of the cursor-like keys and the number of the tilde keys.
enum Functional {
    /// `CSI 1 ; mods <final>`, or `SS3 <final>` without modifiers in application mode.
    Letter(u8, bool),
    /// `CSI <number> ; mods ~`.
    Tilde(u16),
}

fn functional(code: KeyCode) -> Option<Functional> {
    Some(match code {
        KeyCode::Up => Functional::Letter(b'A', true),
        KeyCode::Down => Functional::Letter(b'B', true),
        KeyCode::Right => Functional::Letter(b'C', true),
        KeyCode::Left => Functional::Letter(b'D', true),
        KeyCode::Home => Functional::Letter(b'H', true),
        KeyCode::End => Functional::Letter(b'F', true),
        KeyCode::Insert => Functional::Tilde(2),
        KeyCode::Delete => Functional::Tilde(3),
        KeyCode::PageUp => Functional::Tilde(5),
        KeyCode::PageDown => Functional::Tilde(6),
        KeyCode::F(n) => match n {
            1 => Functional::Letter(b'P', false),
            2 => Functional::Letter(b'Q', false),
            3 => Functional::Letter(b'R', false),
            4 => Functional::Letter(b'S', false),
            5 => Functional::Tilde(15),
            6 => Functional::Tilde(17),
            7 => Functional::Tilde(18),
            8 => Functional::Tilde(19),
            9 => Functional::Tilde(20),
            10 => Functional::Tilde(21),
            11 => Functional::Tilde(23),
            12 => Functional::Tilde(24),
            13 => Functional::Tilde(25),
            14 => Functional::Tilde(26),
            15 => Functional::Tilde(28),
            16 => Functional::Tilde(29),
            17 => Functional::Tilde(31),
            18 => Functional::Tilde(32),
            19 => Functional::Tilde(33),
            20 => Functional::Tilde(34),
            _ => return None,
        },
        _ => return None,
    })
}

/// The control byte of `ctrl+<c>`, when the combination has one.
fn control_byte(c: char) -> Option<u8> {
    let b = u8::try_from(u32::from(c)).ok()?;
    match b {
        b'@' | b' ' | b'2' => Some(0),
        b'a'..=b'z' => Some(b - b'a' + 1),
        b'A'..=b'Z' => Some(b - b'A' + 1),
        b'[' | b'3' => Some(27),
        b'\\' | b'4' => Some(28),
        b']' | b'5' => Some(29),
        b'^' | b'6' => Some(30),
        b'_' | b'7' | b'/' => Some(31),
        b'8' | b'?' => Some(127),
        _ => None,
    }
}

/// The kitty key number of a key without a functional escape form.
fn kitty_number(code: KeyCode) -> Option<u32> {
    Some(match code {
        KeyCode::Char(c) => u32::from(c.to_lowercase().next().unwrap_or(c)),
        KeyCode::Escape => 27,
        KeyCode::Enter => 13,
        KeyCode::Tab => 9,
        KeyCode::Backspace => 127,
        KeyCode::F(n) if n >= 13 => 57363 + u32::from(n - 13),
        _ => return None,
    })
}

/// Encodes a key event to the bytes the child expects, empty when the event sends nothing.
pub fn encode_key(modes: &InputModes, ev: &KeyEvent) -> Vec<u8> {
    if modes.kitty_flags != 0 {
        return encode_key_kitty(modes, ev);
    }
    if ev.state == KeyState::Release {
        return Vec::new();
    }
    encode_key_legacy(modes, ev)
}

fn encode_key_legacy(modes: &InputModes, ev: &KeyEvent) -> Vec<u8> {
    let mut out = Vec::new();
    let m = ev.mods;
    if let Some(f) = functional(ev.code) {
        match f {
            Functional::Letter(fin, cursor) => {
                if m.is_empty() {
                    let intro = if !cursor || modes.app_cursor { b'O' } else { b'[' };
                    out.extend_from_slice(&[ESC, intro, fin]);
                } else {
                    csi(&mut out, &format!("1;{}{}", mod_param(m), fin as char));
                }
            }
            Functional::Tilde(n) => {
                if m.is_empty() {
                    csi(&mut out, &format!("{n}~"));
                } else {
                    csi(&mut out, &format!("{n};{}~", mod_param(m)));
                }
            }
        }
        return out;
    }
    match ev.code {
        KeyCode::Enter => {
            if m.alt {
                out.push(ESC);
            }
            out.push(b'\r');
            if modes.newline_mode {
                out.push(b'\n');
            }
        }
        KeyCode::Tab => {
            if m.shift {
                csi(&mut out, "Z");
            } else {
                if m.alt {
                    out.push(ESC);
                }
                out.push(b'\t');
            }
        }
        KeyCode::Backspace => {
            if m.alt {
                out.push(ESC);
            }
            out.push(if m.ctrl { 0x08 } else { 0x7f });
        }
        KeyCode::Escape => {
            if m.alt {
                out.push(ESC);
            }
            out.push(ESC);
        }
        KeyCode::Char(c) => {
            if m.alt {
                out.push(ESC);
            }
            if m.ctrl && let Some(b) = control_byte(c) {
                out.push(b);
            } else if let Some(text) = &ev.text {
                out.extend_from_slice(text.as_bytes());
            } else {
                let mut buf = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
        }
        _ => {}
    }
    out
}

fn encode_key_kitty(modes: &InputModes, ev: &KeyEvent) -> Vec<u8> {
    let flags = modes.kitty_flags;
    let report_events = flags & 0b0_0010 != 0;
    let report_alternates = flags & 0b0_0100 != 0;
    let report_all = flags & 0b0_1000 != 0;
    let report_text = flags & 0b1_0000 != 0;
    let m = ev.mods;
    let event_code = match ev.state {
        KeyState::Press => 1,
        KeyState::Repeat => 2,
        KeyState::Release => 3,
    };
    // Releases are reported only when asked for, and Enter, Tab and Backspace only in the
    // all-keys mode.
    if ev.state == KeyState::Release {
        let text_like = matches!(ev.code, KeyCode::Enter | KeyCode::Tab | KeyCode::Backspace);
        if !report_events || (text_like && !report_all) {
            return Vec::new();
        }
    }
    let event_suffix = if report_events && event_code != 1 {
        format!(":{event_code}")
    } else {
        String::new()
    };
    let mut out = Vec::new();
    if let Some(f) = functional(ev.code) {
        let plain = m.is_empty() && event_suffix.is_empty();
        match f {
            Functional::Letter(fin, cursor) => {
                if plain {
                    let intro = if !cursor || modes.app_cursor { b'O' } else { b'[' };
                    out.extend_from_slice(&[ESC, intro, fin]);
                } else {
                    csi(&mut out, &format!("1;{}{event_suffix}{}", mod_param(m), fin as char));
                }
            }
            Functional::Tilde(n) => {
                if plain {
                    csi(&mut out, &format!("{n}~"));
                } else {
                    csi(&mut out, &format!("{n};{}{event_suffix}~", mod_param(m)));
                }
            }
        }
        return out;
    }
    let Some(number) = kitty_number(ev.code) else {
        return out;
    };
    let text_key = matches!(ev.code, KeyCode::Char(_));
    let enter_like = matches!(ev.code, KeyCode::Enter | KeyCode::Tab | KeyCode::Backspace);
    let only_shift = !m.alt && !m.ctrl && !m.logo;
    // Without the all-keys mode a text key without a real modifier is plain text, and Enter, Tab
    // and Backspace keep their legacy bytes when no modifier but shift... is down.
    if !report_all && ev.state != KeyState::Release {
        if text_key && only_shift {
            if let Some(text) = &ev.text {
                return text.as_bytes().to_vec();
            }
        }
        if enter_like && m.is_empty() {
            return encode_key_legacy(&InputModes { kitty_flags: 0, ..*modes }, ev);
        }
    }
    let mut code = number.to_string();
    if report_alternates && m.shift && let KeyCode::Char(c) = ev.code {
        let shifted = c.to_uppercase().next().unwrap_or(c);
        if shifted != c {
            code.push(':');
            code.push_str(&u32::from(shifted).to_string());
        }
    }
    let text_param = if report_text && report_all && ev.state != KeyState::Release {
        ev.text.as_ref().filter(|t| t.chars().all(|c| !c.is_control())).map(|t| {
            t.chars()
                .map(|c| u32::from(c).to_string())
                .collect::<Vec<_>>()
                .join(":")
        })
    } else {
        None
    };
    let mods = mod_param(m);
    let body = match (&text_param, mods, event_suffix.is_empty()) {
        (Some(text), _, _) => format!("{code};{mods}{event_suffix};{text}u"),
        (None, 1, true) => format!("{code}u"),
        (None, _, _) => format!("{code};{mods}{event_suffix}u"),
    };
    csi(&mut out, &body);
    out
}

/// Encodes a mouse event, or `None` when the child does not track the mouse this way.
pub fn encode_mouse(modes: &InputModes, ev: &MouseEvent) -> Option<Vec<u8>> {
    let held = |b: MouseButton| match b {
        MouseButton::Left => 0u32,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    };
    let (mut code, release) = match ev.kind {
        MouseKind::Press(b) => (held(b), false),
        MouseKind::Release(b) => (if modes.sgr_mouse { held(b) } else { 3 }, true),
        MouseKind::Move(b) => {
            let tracked = match modes.mouse {
                MouseMode::Motion => true,
                MouseMode::Drag => b.is_some(),
                _ => false,
            };
            if !tracked {
                return None;
            }
            (b.map_or(3, held) + 32, false)
        }
        MouseKind::Wheel { lines } => (if lines > 0 { 64 } else { 65 }, false),
        MouseKind::WheelH { lines } => (if lines > 0 { 66 } else { 67 }, false),
    };
    match modes.mouse {
        MouseMode::Off => return None,
        MouseMode::X10 => {
            if release || !matches!(ev.kind, MouseKind::Press(_)) {
                return None;
            }
        }
        _ => {}
    }
    if modes.mouse != MouseMode::X10 {
        code += u32::from(ev.mods.shift) * 4 + u32::from(ev.mods.alt) * 8 + u32::from(ev.mods.ctrl) * 16;
    }
    let (x, y) = (ev.col + 1, ev.row + 1);
    let mut out = Vec::new();
    if modes.sgr_mouse {
        let fin = if release { 'm' } else { 'M' };
        csi(&mut out, &format!("<{code};{x};{y}{fin}"));
    } else if modes.utf8_mouse {
        out.extend_from_slice(&[ESC, b'[', b'M']);
        for v in [code + 32, x as u32 + 32, y as u32 + 32] {
            let c = char::from_u32(v).unwrap_or('\u{7ff}');
            let mut buf = [0u8; 4];
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
        }
    } else {
        if x > 223 || y > 223 {
            return None;
        }
        out.extend_from_slice(&[ESC, b'[', b'M', (code + 32) as u8, (x + 32) as u8, (y + 32) as u8]);
    }
    Some(out)
}

/// Arrow-key bytes that stand for `lines` of wheel on the alternate screen (xterm's alternate
/// scroll), or `None` when the mode does not apply.
pub fn encode_alt_scroll(modes: &InputModes, lines: i32) -> Option<Vec<u8>> {
    if !modes.alt_screen || !modes.alternate_scroll || modes.mouse != MouseMode::Off || lines == 0 {
        return None;
    }
    let fin = if lines > 0 { b'A' } else { b'B' };
    let intro = if modes.app_cursor { b'O' } else { b'[' };
    let mut out = Vec::new();
    for _ in 0..lines.unsigned_abs() {
        out.extend_from_slice(&[ESC, intro, fin]);
    }
    Some(out)
}

/// Wraps pasted text for the child: bracketed when the child asked for it, with the paste end
/// marker removed from the text so the paste cannot close itself early.
pub fn encode_paste(modes: &InputModes, text: &str) -> Vec<u8> {
    if modes.bracketed_paste {
        let clean = text.replace("\x1b[201~", "").replace("\x1b[200~", "");
        let mut out = b"\x1b[200~".to_vec();
        out.extend_from_slice(clean.as_bytes());
        out.extend_from_slice(b"\x1b[201~");
        out
    } else {
        text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
    }
}

/// The focus report, when the child asked for it.
pub fn encode_focus(modes: &InputModes, focused: bool) -> Option<Vec<u8>> {
    modes
        .focus_events
        .then(|| if focused { b"\x1b[I".to_vec() } else { b"\x1b[O".to_vec() })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL: Mods = Mods {
        shift: false,
        alt: false,
        ctrl: true,
        logo: false,
    };
    const ALT: Mods = Mods {
        shift: false,
        alt: true,
        ctrl: false,
        logo: false,
    };

    fn legacy() -> InputModes {
        InputModes::default()
    }

    #[test]
    fn text_and_control_keys() {
        let m = legacy();
        assert_eq!(encode_key(&m, &KeyEvent::text('a', Mods::NONE)), b"a");
        assert_eq!(encode_key(&m, &KeyEvent::text('c', CTRL)), [3]);
        assert_eq!(encode_key(&m, &KeyEvent::text('x', ALT)), b"\x1bx");
        assert_eq!(encode_key(&m, &KeyEvent::press(KeyCode::Enter, Mods::NONE)), b"\r");
        assert_eq!(encode_key(&m, &KeyEvent::press(KeyCode::Backspace, Mods::NONE)), [0x7f]);
        assert_eq!(encode_key(&m, &KeyEvent::press(KeyCode::Escape, Mods::NONE)), [0x1b]);
        let shift = Mods {
            shift: true,
            ..Mods::NONE
        };
        assert_eq!(encode_key(&m, &KeyEvent::press(KeyCode::Tab, shift)), b"\x1b[Z");
    }

    #[test]
    fn cursor_keys_follow_deckcm_and_modifiers() {
        let mut m = legacy();
        assert_eq!(encode_key(&m, &KeyEvent::press(KeyCode::Up, Mods::NONE)), b"\x1b[A");
        m.app_cursor = true;
        assert_eq!(encode_key(&m, &KeyEvent::press(KeyCode::Up, Mods::NONE)), b"\x1bOA");
        assert_eq!(encode_key(&m, &KeyEvent::press(KeyCode::Up, CTRL)), b"\x1b[1;5A");
        assert_eq!(encode_key(&m, &KeyEvent::press(KeyCode::PageDown, Mods::NONE)), b"\x1b[6~");
        assert_eq!(encode_key(&m, &KeyEvent::press(KeyCode::Delete, ALT)), b"\x1b[3;3~");
        assert_eq!(encode_key(&m, &KeyEvent::press(KeyCode::F(1), Mods::NONE)), b"\x1bOP");
        assert_eq!(encode_key(&m, &KeyEvent::press(KeyCode::F(5), Mods::NONE)), b"\x1b[15~");
        assert_eq!(encode_key(&m, &KeyEvent::press(KeyCode::F(2), CTRL)), b"\x1b[1;5Q");
    }

    #[test]
    fn kitty_disambiguate_and_all_keys() {
        let mut m = legacy();
        m.kitty_flags = 0b1;
        // Plain text stays text, Escape and modified keys become CSI u.
        assert_eq!(encode_key(&m, &KeyEvent::text('a', Mods::NONE)), b"a");
        assert_eq!(encode_key(&m, &KeyEvent::press(KeyCode::Escape, Mods::NONE)), b"\x1b[27u");
        assert_eq!(encode_key(&m, &KeyEvent::text('c', CTRL)), b"\x1b[99;5u");
        assert_eq!(encode_key(&m, &KeyEvent::press(KeyCode::Enter, Mods::NONE)), b"\r");
        let shift = Mods {
            shift: true,
            ..Mods::NONE
        };
        assert_eq!(encode_key(&m, &KeyEvent::press(KeyCode::Enter, shift)), b"\x1b[13;2u");
        // Releases are not reported without the event types flag.
        let mut release = KeyEvent::text('a', Mods::NONE);
        release.state = KeyState::Release;
        assert!(encode_key(&m, &release).is_empty());
        m.kitty_flags = 0b1011;
        assert_eq!(encode_key(&m, &release), b"\x1b[97;1:3u");
        let mut text = KeyEvent::text('a', Mods::NONE);
        text.state = KeyState::Press;
        m.kitty_flags = 0b1_1001;
        assert_eq!(encode_key(&m, &text), b"\x1b[97;1;97u");
        assert_eq!(encode_key(&m, &KeyEvent::press(KeyCode::Up, Mods::NONE)), b"\x1b[A");
    }

    #[test]
    fn mouse_encodings() {
        let mut m = legacy();
        let press = MouseEvent {
            kind: MouseKind::Press(MouseButton::Left),
            col: 4,
            row: 9,
            mods: Mods::NONE,
        };
        assert!(encode_mouse(&m, &press).is_none());
        m.mouse = MouseMode::Click;
        assert_eq!(encode_mouse(&m, &press).unwrap(), [0x1b, b'[', b'M', 32, 37, 42]);
        m.sgr_mouse = true;
        assert_eq!(encode_mouse(&m, &press).unwrap(), b"\x1b[<0;5;10M");
        let release = MouseEvent {
            kind: MouseKind::Release(MouseButton::Left),
            ..press
        };
        assert_eq!(encode_mouse(&m, &release).unwrap(), b"\x1b[<0;5;10m");
        let wheel = MouseEvent {
            kind: MouseKind::Wheel { lines: 1 },
            ..press
        };
        assert_eq!(encode_mouse(&m, &wheel).unwrap(), b"\x1b[<64;5;10M");
        let motion = MouseEvent {
            kind: MouseKind::Move(Some(MouseButton::Left)),
            ..press
        };
        assert!(encode_mouse(&m, &motion).is_none());
        m.mouse = MouseMode::Drag;
        assert_eq!(encode_mouse(&m, &motion).unwrap(), b"\x1b[<32;5;10M");
    }

    #[test]
    fn paste_focus_and_alternate_scroll() {
        let mut m = legacy();
        assert_eq!(encode_paste(&m, "a\nb"), b"a\rb");
        m.bracketed_paste = true;
        assert_eq!(encode_paste(&m, "hi\x1b[201~"), b"\x1b[200~hi\x1b[201~");
        assert!(encode_focus(&m, true).is_none());
        m.focus_events = true;
        assert_eq!(encode_focus(&m, false).unwrap(), b"\x1b[O");
        assert!(encode_alt_scroll(&m, 3).is_none());
        m.alt_screen = true;
        m.alternate_scroll = true;
        assert_eq!(encode_alt_scroll(&m, -2).unwrap(), b"\x1b[B\x1b[B");
    }
}
