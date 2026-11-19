//! Syntax highlighting of the read-only text views, with `tree-sitter-highlight`.
//!
//! The game ships Lua only as 5.2 bytecode and event scripts only as T2B tables, so the text views
//! render both as Lua call syntax (see `preview::text`) and this module colours that text with the
//! tree-sitter Lua grammar. One tree-sitter runtime serves the whole binary: `nie-lua`'s analysis
//! uses the same `tree-sitter` crate, so no second copy of its C symbols is linked.

use std::sync::OnceLock;

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

use super::preview::{Line, Span, Style};

/// Capture names the grammar's highlight query may produce, each with the style it renders as.
/// A longer capture (`keyword.return`) matches the longest name that prefixes it.
const CAPTURES: [(&str, Style); 25] = [
    ("keyword", Style::Keyword),
    ("repeat", Style::Keyword),
    ("conditional", Style::Keyword),
    ("include", Style::Keyword),
    ("exception", Style::Keyword),
    ("string", Style::String),
    ("character", Style::String),
    ("number", Style::Number),
    ("float", Style::Number),
    ("boolean", Style::Constant),
    ("constant", Style::Constant),
    ("comment", Style::Comment),
    ("function", Style::Function),
    ("method", Style::Function),
    ("type", Style::Type),
    ("constructor", Style::Type),
    ("attribute", Style::Type),
    ("namespace", Style::Type),
    ("property", Style::Property),
    ("field", Style::Property),
    ("variable.member", Style::Property),
    ("operator", Style::Operator),
    ("punctuation", Style::Punctuation),
    ("delimiter", Style::Punctuation),
    ("label", Style::Label),
];

fn config() -> Option<&'static HighlightConfiguration> {
    static CONFIG: OnceLock<Option<HighlightConfiguration>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let mut config = HighlightConfiguration::new(
                tree_sitter_lua::LANGUAGE.into(),
                "lua",
                tree_sitter_lua::HIGHLIGHTS_QUERY,
                "",
                "",
            )
            .ok()?;
            let names: Vec<&str> = CAPTURES.iter().map(|(name, _)| *name).collect();
            config.configure(&names);
            Some(config)
        })
        .as_ref()
}

/// Whether the Lua grammar loaded in this build.
#[must_use]
pub fn lua_available() -> bool {
    config().is_some()
}

/// Highlights Lua text into styled lines. Text the grammar cannot parse comes back unstyled, one
/// span per line, so the view is never empty because the highlighter failed.
#[must_use]
pub fn lua(source: &str) -> Vec<Line> {
    let styled = (|| {
        let config = config()?;
        let mut highlighter = Highlighter::new();
        let events = highlighter
            .highlight(config, source.as_bytes(), None, None, |_| None)
            .ok()?;
        let mut runs: Vec<(usize, Style)> = Vec::new();
        let mut stack: Vec<Style> = Vec::new();
        for event in events {
            match event.ok()? {
                HighlightEvent::HighlightStart(h) => {
                    stack.push(CAPTURES.get(h.0).map_or(Style::Plain, |(_, style)| *style));
                }
                HighlightEvent::HighlightEnd => {
                    stack.pop();
                }
                HighlightEvent::Source { end, .. } => {
                    runs.push((end, stack.last().copied().unwrap_or(Style::Plain)));
                }
            }
        }
        Some(runs)
    })();
    match styled {
        Some(runs) => lines_from_runs(source, &runs),
        None => source
            .lines()
            .map(|l| Line::new(l.to_owned(), Style::Plain))
            .collect(),
    }
}

/// Builds lines from `(end offset, style of the text before it)` boundaries.
fn lines_from_runs(source: &str, boundaries: &[(usize, Style)]) -> Vec<Line> {
    let mut lines = vec![Line::default()];
    let mut cursor = 0usize;
    let push = |lines: &mut Vec<Line>, text: &str, style: Style| {
        let mut parts = text.split('\n');
        if let Some(first) = parts.next()
            && !first.is_empty()
        {
            let first = first.strip_suffix('\r').unwrap_or(first);
            append(lines.last_mut(), first, style);
        }
        for part in parts {
            lines.push(Line::default());
            let part = part.strip_suffix('\r').unwrap_or(part);
            if !part.is_empty() {
                append(lines.last_mut(), part, style);
            }
        }
    };
    for (end, style) in boundaries {
        let end = (*end).min(source.len());
        if end > cursor && source.is_char_boundary(cursor) && source.is_char_boundary(end) {
            push(&mut lines, &source[cursor..end], *style);
            cursor = end;
        }
    }
    if cursor < source.len() && source.is_char_boundary(cursor) {
        push(&mut lines, &source[cursor..], Style::Plain);
    }
    if lines.last().is_some_and(|l| l.0.is_empty()) && source.ends_with('\n') {
        lines.pop();
    }
    lines
}

fn append(line: Option<&mut Line>, text: &str, style: Style) {
    let Some(line) = line else { return };
    match line.0.last_mut() {
        Some(last) if last.style == style => last.text.push_str(text),
        _ => line.0.push(Span {
            text: text.to_owned(),
            style,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn styles_of(lines: &[Line], needle: &str) -> Vec<Style> {
        lines
            .iter()
            .flat_map(|l| l.0.iter())
            .filter(|s| s.text.contains(needle))
            .map(|s| s.style)
            .collect()
    }

    #[test]
    fn highlights_lua_source() {
        assert!(lua_available());
        let lines = lua("local x = 42 -- note\nreturn \"hi\"\n");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].plain(), "local x = 42 -- note");
        assert!(styles_of(&lines, "local").contains(&Style::Keyword));
        assert!(styles_of(&lines, "42").contains(&Style::Number));
        assert!(styles_of(&lines, "-- note").contains(&Style::Comment));
        assert!(styles_of(&lines, "hi").contains(&Style::String));
    }

    #[test]
    fn highlighting_keeps_every_character() {
        let source = "function f(a, b)\n  return a + b -- sum\nend\n";
        let lines = lua(source);
        let rebuilt: Vec<String> = lines.iter().map(Line::plain).collect();
        assert_eq!(rebuilt.join("\n") + "\n", source);
    }

    #[test]
    fn broken_input_still_renders() {
        let lines = lua("local = = (\n");
        assert_eq!(lines[0].plain(), "local = = (");
    }
}
