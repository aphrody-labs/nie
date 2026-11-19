//! The theme, derived from the colours of the game's own UI.
//!
//! The only source is `data/common/font/font_color.cfg.bin` (`FONT_COLOR`, 70 entries): every
//! role of the theme is a colour of that palette, moved toward black or white only as far as the
//! contrast target requires. Nothing here is a hand-picked brand colour. When the palette has no
//! colour near a wanted hue the hue itself is used as the fallback, at the palette's median
//! saturation and lightness.

use gpui::{Hsla, Rgba};
use nie_data::font_color::{FontColor, parse_font_colors};

/// An sRGB colour, 8 bits per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// WCAG relative luminance, 0.0 (black) to 1.0 (white).
    #[must_use]
    pub fn luminance(self) -> f32 {
        let lin = |c: u8| {
            let c = f32::from(c) / 255.0;
            if c <= 0.039_28 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * lin(self.0) + 0.7152 * lin(self.1) + 0.0722 * lin(self.2)
    }

    /// WCAG contrast ratio between two colours, 1.0 to 21.0.
    #[must_use]
    pub fn contrast(self, other: Self) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// Linear mix in sRGB space: `t` of `other`, the rest of `self`.
    #[must_use]
    pub fn mix(self, other: Self, t: f32) -> Self {
        let m = |a: u8, b: u8| {
            let v = f32::from(a) + (f32::from(b) - f32::from(a)) * t.clamp(0.0, 1.0);
            v.round().clamp(0.0, 255.0) as u8
        };
        Self(m(self.0, other.0), m(self.1, other.1), m(self.2, other.2))
    }

    /// Hue in degrees, saturation and lightness (HSL, both 0.0 to 1.0).
    #[must_use]
    pub fn hsl(self) -> (f32, f32, f32) {
        let (r, g, b) = (f32::from(self.0) / 255.0, f32::from(self.1) / 255.0, f32::from(self.2) / 255.0);
        let (max, min) = (r.max(g).max(b), r.min(g).min(b));
        let l = (max + min) / 2.0;
        let d = max - min;
        if d == 0.0 {
            return (0.0, 0.0, l);
        }
        let s = d / (1.0 - (2.0 * l - 1.0).abs());
        let h = if max == r {
            ((g - b) / d).rem_euclid(6.0)
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        (h * 60.0, s, l)
    }

    /// The colour of the given HSL triple.
    #[must_use]
    pub fn from_hsl(h: f32, s: f32, l: f32) -> Self {
        let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
        let m = l - c / 2.0;
        let (r, g, b) = match (h.rem_euclid(360.0) / 60.0) as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let q = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
        Self(q(r), q(g), q(b))
    }

    /// The colour as a GPUI value.
    #[must_use]
    pub fn hsla(self) -> Hsla {
        Rgba { r: f32::from(self.0) / 255.0, g: f32::from(self.1) / 255.0, b: f32::from(self.2) / 255.0, a: 1.0 }.into()
    }
}

/// How the theme is tuned for the reader.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Preferences {
    /// WCAG AAA contrast (7:1) instead of AA (4.5:1) for every text role.
    pub high_contrast: bool,
    /// Multiplier of every length (1.0 is the design size).
    pub scale: f32,
    /// Skip animations such as the frame surfaces' autoplay.
    pub reduce_motion: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self { high_contrast: false, scale: 1.0, reduce_motion: false }
    }
}

impl Preferences {
    /// Reads `NIE_HIGH_CONTRAST`, `NIE_UI_SCALE` and `NIE_REDUCE_MOTION` from the environment.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            high_contrast: std::env::var_os("NIE_HIGH_CONTRAST").is_some(),
            scale: std::env::var("NIE_UI_SCALE").ok().and_then(|s| s.parse().ok()).unwrap_or(1.0),
            reduce_motion: std::env::var_os("NIE_REDUCE_MOTION").is_some(),
        }
    }

    /// The contrast ratio every text role must reach against the background.
    #[must_use]
    pub fn contrast_target(self) -> f32 {
        if self.high_contrast { 7.0 } else { 4.5 }
    }
}

/// Hues the semantic roles ask the palette for, in degrees.
const HUE_ERROR: f32 = 0.0;
const HUE_WARN: f32 = 45.0;
const HUE_OK: f32 = 130.0;
const HUE_INFO: f32 = 205.0;
const HUE_FUNCTION: f32 = 275.0;
const HUE_TYPE: f32 = 170.0;
/// Entries below this saturation count as greys, not as hues.
const MIN_SATURATION: f32 = 0.30;

/// The resolved colours and sizes every component draws with.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    /// The preferences the theme was derived for.
    pub preferences: Preferences,
    /// Window background.
    pub background: Rgb,
    /// Panels and rows.
    pub surface: Rgb,
    /// A hovered row.
    pub surface_hover: Rgb,
    /// The selected row.
    pub selection: Rgb,
    /// Text drawn on `selection`.
    pub selection_text: Rgb,
    /// Lines between panels.
    pub border: Rgb,
    /// The keyboard focus ring.
    pub focus: Rgb,
    /// Primary text.
    pub text: Rgb,
    /// Secondary text.
    pub text_dim: Rgb,
    /// The accent, the most saturated colour of the palette.
    pub accent: Rgb,
    /// Errors.
    pub error: Rgb,
    /// Warnings.
    pub warn: Rgb,
    /// Success.
    pub ok: Rgb,
    /// Information.
    pub info: Rgb,
    /// Functions and opcodes in a code view.
    pub function: Rgb,
    /// Types and constructors in a code view.
    pub type_: Rgb,
    /// UI font family (bundled with GPUI).
    pub ui_font: &'static str,
    /// Monospace font family (bundled with GPUI).
    pub mono_font: &'static str,
    /// Body text size in logical pixels before scaling.
    pub text_size: f32,
}

impl Theme {
    /// Derives the theme from the bytes of `data/common/font/font_color.cfg.bin`.
    ///
    /// # Errors
    /// The file is not a cfg.bin with a `FONT_COLOR` list.
    pub fn from_font_color_bin(bytes: &[u8], preferences: Preferences) -> anyhow::Result<Self> {
        let root = nie_formats::cfgbin::to_tagged_json(bytes)
            .ok_or_else(|| anyhow::anyhow!("font_color.cfg.bin is not a cfg.bin"))?;
        let palette = parse_font_colors(&root);
        anyhow::ensure!(!palette.is_empty(), "font_color.cfg.bin has no FONT_COLOR entries");
        Ok(Self::derive(&palette, preferences))
    }

    /// Derives the theme from a palette.
    ///
    /// # Panics
    /// Never: an empty palette falls back to black and white.
    #[must_use]
    pub fn derive(palette: &[FontColor], preferences: Preferences) -> Self {
        let mut colours: Vec<Rgb> = palette.iter().map(|c| Rgb(c.rgb.0, c.rgb.1, c.rgb.2)).collect();
        colours.sort_by(|a, b| a.luminance().total_cmp(&b.luminance()));
        colours.dedup();
        let target = preferences.contrast_target();
        let black = Rgb(0, 0, 0);
        let white = Rgb(255, 255, 255);
        // The text is the least saturated of the light entries (a near-white, not a bright yellow);
        // the background is the darkest entry.
        let lightest = colours.last().copied().unwrap_or(white);
        let text0 = colours
            .iter()
            .copied()
            .filter(|c| c.luminance() >= 0.7 * lightest.luminance())
            .min_by(|a, b| a.hsl().1.total_cmp(&b.hsl().1))
            .unwrap_or(lightest);
        let (mut background, mut text) = (colours.first().copied().unwrap_or(black), text0);
        // The darkest entry is the background; the lightest is the text. Each moves toward its
        // extreme until the contrast target is met.
        for step in 0..=20 {
            let t = step as f32 / 20.0;
            let (b, f) = (background.mix(black, t), text.mix(white, t));
            if f.contrast(b) >= target * 1.2 {
                background = b;
                text = f;
                break;
            }
            background = background.mix(black, t.min(1.0));
            text = text.mix(white, t.min(1.0));
        }
        let reach = |c: Rgb| ensure_contrast(c, background, target);
        let hue_of = |hue: f32| {
            let pick = colours
                .iter()
                .copied()
                .filter(|c| c.hsl().1 >= MIN_SATURATION)
                .min_by(|a, b| hue_distance(a.hsl().0, hue).total_cmp(&hue_distance(b.hsl().0, hue)))
                .filter(|c| hue_distance(c.hsl().0, hue) <= 40.0)
                .unwrap_or_else(|| Rgb::from_hsl(hue, 0.65, 0.6));
            reach(pick)
        };
        let accent = reach(
            colours
                .iter()
                .copied()
                .max_by(|a, b| {
                    let (a, b) = (a.hsl(), b.hsl());
                    (a.1 * (1.0 - (a.2 - 0.5).abs())).total_cmp(&(b.1 * (1.0 - (b.2 - 0.5).abs())))
                })
                .unwrap_or(text),
        );
        let text_dim = reach(text.mix(background, 0.30));
        let border = background.mix(text, 0.35);
        // The selection is the accent as far as the text on it keeps the contrast target.
        let selection = (0..=9)
            .map(|step| background.mix(accent, 0.45 - 0.05 * step as f32))
            .find(|c| text.contrast(*c) >= target)
            .unwrap_or(background);
        let selection_text = ensure_contrast(text, selection, target);
        Self {
            preferences,
            background,
            surface: background.mix(text, 0.07),
            surface_hover: background.mix(text, 0.14),
            selection,
            selection_text,
            border,
            focus: reach(accent.mix(white, 0.25)),
            text,
            text_dim,
            accent,
            error: hue_of(HUE_ERROR),
            warn: hue_of(HUE_WARN),
            ok: hue_of(HUE_OK),
            info: hue_of(HUE_INFO),
            function: hue_of(HUE_FUNCTION),
            type_: hue_of(HUE_TYPE),
            ui_font: "IBM Plex Sans",
            mono_font: "Lilex",
            text_size: 15.0,
        }
    }

    /// The bootstrap theme shown before the game's palette is read: the contrast floor itself
    /// (white on black, one grey), not a brand.
    #[must_use]
    pub fn bootstrap(preferences: Preferences) -> Self {
        Self::derive(&[], preferences)
    }

    /// A length scaled by the reader's preference.
    #[must_use]
    pub fn px(&self, logical: f32) -> gpui::Pixels {
        gpui::px(logical * self.preferences.scale)
    }

    /// The colour of a code-view style class.
    #[must_use]
    pub fn syntax(&self, style: Syntax) -> Rgb {
        match style {
            Syntax::Plain => self.text,
            Syntax::Keyword => self.accent,
            Syntax::String => self.ok,
            Syntax::Number | Syntax::Constant => self.warn,
            Syntax::Comment | Syntax::Dim | Syntax::Punctuation => self.text_dim,
            Syntax::Function => self.function,
            Syntax::Type => self.type_,
            Syntax::Property | Syntax::Label => self.info,
            Syntax::Operator => self.text,
            Syntax::Header | Syntax::Accent => self.accent,
            Syntax::Error => self.error,
        }
    }
}

/// The semantic class of a piece of code or log text (the vocabulary of `nie fm`'s previews).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Syntax {
    /// Unstyled.
    Plain,
    /// A language keyword.
    Keyword,
    /// A string literal.
    String,
    /// A number literal.
    Number,
    /// A constant.
    Constant,
    /// A comment.
    Comment,
    /// A function or an opcode.
    Function,
    /// A type or a constructor.
    Type,
    /// A field name.
    Property,
    /// An operator.
    Operator,
    /// Brackets and separators.
    Punctuation,
    /// A label.
    Label,
    /// Secondary information.
    Dim,
    /// A heading.
    Header,
    /// A value worth looking at.
    Accent,
    /// An error.
    Error,
}

fn hue_distance(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

/// Moves `fg` toward white or black (whichever reaches further against `bg`) until the contrast
/// target holds.
fn ensure_contrast(fg: Rgb, bg: Rgb, target: f32) -> Rgb {
    if fg.contrast(bg) >= target {
        return fg;
    }
    let toward = if bg.luminance() < 0.5 { Rgb(255, 255, 255) } else { Rgb(0, 0, 0) };
    for step in 1..=20 {
        let c = fg.mix(toward, step as f32 / 20.0);
        if c.contrast(bg) >= target {
            return c;
        }
    }
    toward
}

#[cfg(test)]
mod tests {
    use super::*;
    use nie_data::hash::HashId;

    fn palette(rgbs: &[(u8, u8, u8)]) -> Vec<FontColor> {
        rgbs.iter()
            .enumerate()
            .map(|(i, rgb)| FontColor { id: HashId(i as u32), rgb: *rgb, rubi_rgb: *rgb })
            .collect()
    }

    #[test]
    fn every_text_role_reaches_the_contrast_target() {
        let p = palette(&[
            (20, 22, 40), (250, 250, 240), (230, 40, 50), (40, 200, 90), (240, 190, 40),
            (60, 140, 240), (170, 80, 220), (30, 190, 160), (128, 128, 128),
        ]);
        for high in [false, true] {
            let prefs = Preferences { high_contrast: high, ..Preferences::default() };
            let t = Theme::derive(&p, prefs);
            let target = prefs.contrast_target();
            for (name, c) in [
                ("text", t.text), ("text_dim", t.text_dim), ("accent", t.accent), ("error", t.error),
                ("warn", t.warn), ("ok", t.ok), ("info", t.info), ("function", t.function), ("type", t.type_),
            ] {
                assert!(c.contrast(t.background) >= target, "{name} {c:?} on {:?} high={high}", t.background);
            }
            assert!(t.selection_text.contrast(t.selection) >= target);
        }
    }

    #[test]
    fn roles_come_from_the_palette_when_it_has_the_hue() {
        let p = palette(&[(0, 0, 0), (255, 255, 255), (255, 0, 0), (0, 255, 0), (255, 200, 0), (0, 120, 255)]);
        let t = Theme::derive(&p, Preferences::default());
        assert_eq!(t.error.hsl().0.round(), 0.0);
        assert!((t.ok.hsl().0 - 120.0).abs() < 2.0);
    }

    #[test]
    fn an_empty_palette_falls_back_to_the_contrast_floor() {
        let t = Theme::bootstrap(Preferences::default());
        assert!(t.text.contrast(t.background) >= 7.0);
    }

    #[test]
    fn scale_multiplies_lengths() {
        let t = Theme::bootstrap(Preferences { scale: 1.5, ..Preferences::default() });
        assert_eq!(t.px(10.0), gpui::px(15.0));
    }
}
