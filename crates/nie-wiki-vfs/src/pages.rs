//! The wiki's HTML pages as a function of a URL path, for hosts that have no HTTP server: the
//! browser app answers `/wiki/...` links with it, over the bytes it fetched from the VFS.
//!
//! The routes are the ones `nie serve` serves under `/wiki` (see `docs/serve.md`):
//! `/wiki`, `/wiki/<lang>/<family>[/<key>]`, `/wiki/<lang>/tables[/<table>/<class>[/<index>]]`
//! with `family` one of `characters`, `skills`, `teams`.

use std::collections::HashMap;

use crate::html::{self, Ctx, ListParams};
use crate::{Error, Language, Query, Wiki};

const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT: usize = 500;

/// An answered page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// HTTP-like status: 200, 400, 404 or 500.
    pub status: u16,
    /// A complete HTML document.
    pub html: String,
}

fn ok(html: String) -> Rendered {
    Rendered { status: 200, html }
}

fn missing(wiki: &Wiki, what: &str) -> Rendered {
    Rendered {
        status: 404,
        html: html::not_found(wiki.languages(), what),
    }
}

fn failed(wiki: &Wiki, error: &Error) -> Rendered {
    let status = if matches!(error, Error::UnknownLanguage(_)) {
        400
    } else {
        500
    };
    Rendered {
        status,
        html: html::not_found(wiki.languages(), &error.to_string()),
    }
}

/// Splits `path?query` into decoded path segments and a decoded query map.
#[must_use]
pub fn split_url(target: &str) -> (Vec<String>, HashMap<String, String>) {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let segments = path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(percent_decode)
        .collect();
    let query = query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            (percent_decode(k), percent_decode(v))
        })
        .collect();
    (segments, query)
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if bytes.get(i + 1..i + 3).is_some() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    out.push(byte);
                    i += 2;
                } else {
                    out.push(b'%');
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Renders the page at `target` (`/wiki/en/characters?q=x&limit=20`).
#[must_use]
pub fn render(wiki: &Wiki, target: &str) -> Rendered {
    let (segments, query) = split_url(target);
    let mut parts = segments.iter().map(String::as_str);
    if parts.next() != Some("wiki") {
        return missing(wiki, "not a wiki path");
    }
    let rest: Vec<&str> = parts.collect();
    let Some(&tag) = rest.first() else {
        let built = (|| {
            Ok::<_, Error>(html::index(
                wiki.languages(),
                &wiki.sources()?,
                &wiki.character_classes()?,
                wiki.unsourced(),
                &wiki.catalog()?,
            ))
        })();
        return built.map_or_else(|e| failed(wiki, &e), ok);
    };
    let languages: &[Language] = wiki.languages();
    let Some(lang) = wiki
        .resolve_languages(&[tag])
        .ok()
        .and_then(|folders| languages.iter().find(|l| l.folder == folders[0]))
    else {
        return missing(
            wiki,
            &format!("language `{tag}` is not shipped by this installation"),
        );
    };
    let limit = query
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_LIMIT)
        .clamp(1, MAX_LIMIT);
    let text = query.get("q").filter(|q| !q.is_empty()).cloned();
    let offset = query
        .get("offset")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let params = ListParams {
        text: text.clone(),
        offset,
        limit,
    };
    let list_query = Query {
        languages: vec![lang.folder.clone()],
        text,
        class: query.get("class").filter(|c| !c.is_empty()).cloned(),
        offset,
        limit: Some(limit),
    };
    let render = |rest: String, page: &dyn Fn(&Ctx<'_>) -> String| {
        let ctx = Ctx {
            languages,
            lang,
            rest: &rest,
        };
        ok(page(&ctx))
    };
    let all: [String; 0] = [];
    let key = rest.get(2).copied().unwrap_or("");
    let none = |what: &str| missing(wiki, &format!("no entry matches `{what}`"));
    match (rest.get(1).copied(), rest.len()) {
        (Some("tables"), 2) => match wiki.tables() {
            Ok(tables) => render("tables".to_owned(), &|c| html::tables(c, &tables)),
            Err(error) => failed(wiki, &error),
        },
        (Some("tables"), 4) => {
            let (table, class) = (rest[2], rest[3]);
            match wiki.table_records(table, class, &list_query) {
                Ok(Some(page)) => render(format!("tables/{table}/{class}"), &|c| {
                    html::table_list(c, table, class, &page, &params)
                }),
                Ok(None) => missing(wiki, &format!("no table class `{table}/{class}`")),
                Err(error) => failed(wiki, &error),
            }
        }
        (Some("tables"), 5) => {
            let (table, class) = (rest[2], rest[3]);
            match rest[4].parse::<usize>() {
                Ok(index) => match wiki.table_record(table, class, index, &all) {
                    Ok(Some(record)) => {
                        render(format!("tables/{table}/{class}/{}", record.index), &|c| {
                            html::table_record(c, &record)
                        })
                    }
                    Ok(None) => none(rest[4]),
                    Err(error) => failed(wiki, &error),
                },
                Err(_) => none(rest[4]),
            }
        }
        (Some("characters"), 2) => {
            let built = wiki
                .characters(&list_query)
                .and_then(|page| Ok((page, wiki.character_classes()?)));
            match built {
                Ok((page, classes)) => render("characters".to_owned(), &|c| {
                    html::characters(c, &page, &params, &classes)
                }),
                Err(error) => failed(wiki, &error),
            }
        }
        (Some("skills"), 2) => match wiki.skills(&list_query) {
            Ok(page) => render("skills".to_owned(), &|c| html::skills(c, &page, &params)),
            Err(error) => failed(wiki, &error),
        },
        (Some("teams"), 2) => match wiki.teams(&list_query) {
            Ok(page) => render("teams".to_owned(), &|c| html::teams(c, &page, &params)),
            Err(error) => failed(wiki, &error),
        },
        (Some("characters"), 3) => match wiki.character(key, &all) {
            Ok(Some(record)) => render(format!("characters/{key}"), &|c| {
                html::character(c, &record)
            }),
            Ok(None) => none(key),
            Err(error) => failed(wiki, &error),
        },
        (Some("skills"), 3) => match wiki.skill(key, &all) {
            Ok(Some(record)) => render(format!("skills/{key}"), &|c| html::skill(c, &record)),
            Ok(None) => none(key),
            Err(error) => failed(wiki, &error),
        },
        (Some("teams"), 3) => match wiki.team(key, &all) {
            Ok(Some(record)) => render(format!("teams/{key}"), &|c| html::team(c, &record)),
            Ok(None) => none(key),
            Err(error) => failed(wiki, &error),
        },
        _ => missing(wiki, "route not found"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_split_into_decoded_segments_and_query() {
        let (segments, query) = split_url("/wiki/zh-Hans/characters/0xA0%2F01?q=a+b%21&limit=5");
        assert_eq!(segments, ["wiki", "zh-Hans", "characters", "0xA0/01"]);
        assert_eq!(query["q"], "a b!");
        assert_eq!(query["limit"], "5");
    }
}
