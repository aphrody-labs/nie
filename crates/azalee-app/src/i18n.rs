//! UI strings and the series catalogue, both read from data files compiled in.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::Value;

/// Locale files, keyed by language tag. English is the source language.
const LOCALES: &[(&str, &str)] = &[
    ("en", include_str!("../locales/en.json")),
    ("fr", include_str!("../locales/fr.json")),
    ("ja", include_str!("../locales/ja.json")),
];

/// Series catalogue (slugs and official paths on the source site).
const SERIES: &str = include_str!("../data/series.json");

type Catalog = BTreeMap<String, BTreeMap<String, String>>;

fn catalog() -> &'static Catalog {
    static C: OnceLock<Catalog> = OnceLock::new();
    C.get_or_init(|| {
        LOCALES
            .iter()
            .map(|(tag, src)| {
                let map: BTreeMap<String, String> = serde_json::from_str(src).unwrap_or_default();
                ((*tag).to_owned(), map)
            })
            .collect()
    })
}

/// Language tags with a catalogue.
#[must_use]
pub fn languages() -> Vec<&'static str> {
    LOCALES.iter().map(|(t, _)| *t).collect()
}

/// Picks the best language from a `lang` query value, then `Accept-Language`, then English.
#[must_use]
pub fn negotiate(query: Option<&str>, accept: Option<&str>) -> &'static str {
    let known = languages();
    let pick = |tag: &str| {
        let base = tag
            .split(['-', '_', ';'])
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        known.iter().copied().find(|k| *k == base)
    };
    if let Some(l) = query.and_then(pick) {
        return l;
    }
    if let Some(a) = accept {
        for part in a.split(',') {
            if let Some(l) = pick(part) {
                return l;
            }
        }
    }
    "en"
}

/// String for `key` in `lang`, falling back to English, then to the key itself.
#[must_use]
pub fn t(lang: &str, key: &str) -> String {
    let c = catalog();
    c.get(lang)
        .and_then(|m| m.get(key))
        .or_else(|| c.get("en").and_then(|m| m.get(key)))
        .cloned()
        .unwrap_or_else(|| key.to_owned())
}

/// Same as [`t`] with `{name}` placeholders replaced.
#[must_use]
pub fn tf(lang: &str, key: &str, args: &[(&str, String)]) -> String {
    args.iter()
        .fold(t(lang, key), |s, (k, v)| s.replace(&format!("{{{k}}}"), v))
}

/// One game of the series.
#[derive(Debug, Clone)]
pub struct Game {
    /// Slug, also the locale key suffix `game.<slug>`.
    pub slug: String,
    /// Absolute URL of the official page.
    pub official: String,
    /// Whether nie reads this game.
    pub nie: bool,
    /// Platform codes, locale keys `platform.<code>`.
    pub platforms: Vec<String>,
}

/// Games of the series, in catalogue order.
#[must_use]
pub fn series() -> Vec<Game> {
    let v: Value = serde_json::from_str(SERIES).unwrap_or(Value::Null);
    let base = v
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim_end_matches('/');
    v.get("games")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|g| {
                    Some(Game {
                        slug: g.get("slug")?.as_str()?.to_owned(),
                        official: format!("{base}{}", g.get("official")?.as_str()?),
                        nie: g.get("nie").and_then(Value::as_bool).unwrap_or(false),
                        platforms: g
                            .get("platforms")
                            .and_then(Value::as_array)
                            .map(|a| {
                                a.iter()
                                    .filter_map(|p| p.as_str().map(str::to_owned))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Game text marks furigana as `[base/reading]`; keeps the base text.
#[must_use]
pub fn strip_ruby(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('[') {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 1..];
        match (tail.find('/'), tail.find(']')) {
            (Some(sl), Some(end)) if sl < end => {
                out.push_str(&tail[..sl]);
                rest = &tail[end + 1..];
            }
            _ => {
                out.push('[');
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn ruby() {
        assert_eq!(super::strip_ruby("[円堂/えんどう] [守/まもる]"), "円堂 守");
        assert_eq!(super::strip_ruby("Mark [x"), "Mark [x");
    }

    use super::*;

    #[test]
    fn every_locale_has_every_key() {
        let c = catalog();
        let en = &c["en"];
        for (tag, m) in c {
            for k in en.keys() {
                assert!(m.contains_key(k), "{tag} lacks {k}");
            }
        }
    }

    #[test]
    fn every_game_has_a_title_in_every_locale() {
        let games = series();
        assert!(games.len() >= 10);
        for g in &games {
            for l in languages() {
                assert_ne!(
                    t(l, &format!("game.{}", g.slug)),
                    format!("game.{}", g.slug),
                    "{l} {}",
                    g.slug
                );
                assert!(!g.platforms.is_empty(), "{}", g.slug);
                for p in &g.platforms {
                    let k = format!("platform.{p}");
                    assert_ne!(t(l, &k), k, "{l} {k}");
                }
            }
        }
    }

    #[test]
    fn negotiation() {
        assert_eq!(negotiate(Some("fr"), None), "fr");
        assert_eq!(negotiate(None, Some("ja-JP,ja;q=0.9")), "ja");
        assert_eq!(negotiate(Some("xx"), Some("de")), "en");
    }
}

/// Game by slug.
#[must_use]
pub fn game(slug: &str) -> Option<Game> {
    series().into_iter().find(|g| g.slug == slug)
}

/// Paths and entry names of the game text tables (`data/zukan.json`).
#[derive(Debug, Clone)]
pub struct Zukan {
    /// Text folder with a `{lang}` placeholder.
    pub text_dir: String,
    /// Character names file.
    pub chara_names: String,
    /// T2B entry name of a noun row.
    pub noun_entry: String,
    /// Kind of the full-name row.
    pub full_name_kind: i32,
    /// Rows per page.
    pub page_size: i64,
}

/// Zukan configuration.
#[must_use]
pub fn zukan() -> Zukan {
    let v: Value = serde_json::from_str(include_str!("../data/zukan.json")).unwrap_or(Value::Null);
    let s = |k: &str| {
        v.get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    Zukan {
        text_dir: s("text_dir"),
        chara_names: s("chara_names"),
        noun_entry: s("noun_entry"),
        full_name_kind: v
            .get("full_name_kind")
            .and_then(Value::as_i64)
            .and_then(|n| i32::try_from(n).ok())
            .unwrap_or(0),
        page_size: v.get("page_size").and_then(Value::as_i64).unwrap_or(100),
    }
}

impl Zukan {
    /// Path of the character names file for a game language.
    #[must_use]
    pub fn names_path(&self, lang: &str) -> String {
        format!(
            "{}/{}",
            self.text_dir.replace("{lang}", lang),
            self.chara_names
        )
    }
}
