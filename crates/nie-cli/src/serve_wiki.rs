//! The wiki routes of `nie serve`: JSON under `/v1/wiki`, HTML under `/wiki`. Every answer is
//! computed by `nie-wiki-vfs` from the mounted VFS; the index is built once per process.

use std::collections::HashMap;
use std::sync::Arc;

use nie_wiki_vfs::html::{self, Ctx, ListParams};
use nie_wiki_vfs::{Error, Language, Query, Wiki};
use serde::Serialize;
use serde_json::{Value, json};

use crate::serve_cmd::HttpResponse;
use crate::serve_routes::{Family, Route};

const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT: usize = 500;

fn error_json(status: u16, reason: &'static str, message: impl ToString) -> HttpResponse {
    HttpResponse::json(status, reason, &json!({ "error": message.to_string() }))
}

fn failure(error: &Error) -> HttpResponse {
    match error {
        Error::UnknownLanguage(_) => error_json(400, "Bad Request", error),
        _ => error_json(500, "Internal Server Error", error),
    }
}

fn html_response(status: u16, reason: &'static str, page: String) -> HttpResponse {
    HttpResponse::bytes(
        status,
        reason,
        "text/html; charset=utf-8",
        "public, max-age=300",
        page.into_bytes(),
    )
}

fn limit_of(query: &HashMap<String, String>) -> usize {
    query
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_LIMIT)
        .clamp(1, MAX_LIMIT)
}

fn offset_of(query: &HashMap<String, String>) -> usize {
    query
        .get("offset")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

fn languages_of(query: &HashMap<String, String>) -> Vec<String> {
    query
        .get("lang")
        .map(|v| {
            v.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn text_of(query: &HashMap<String, String>) -> Option<String> {
    query.get("q").filter(|q| !q.is_empty()).cloned()
}

/// A page of JSON records.
fn page_json<T: Serialize>(page: &nie_wiki_vfs::Page<T>, limit: usize) -> Value {
    json!({
        "total": page.total,
        "offset": page.offset,
        "limit": limit,
        "items": page.items,
    })
}

fn record_json<T: Serialize>(record: Result<Option<T>, Error>, key: &str) -> HttpResponse {
    match record {
        Ok(Some(value)) => HttpResponse::json(
            200,
            "OK",
            &serde_json::to_value(value).unwrap_or(Value::Null),
        ),
        Ok(None) => error_json(404, "Not Found", format!("no entry matches `{key}`")),
        Err(error) => failure(&error),
    }
}

/// JSON routes (`/v1/wiki/...`).
pub(crate) fn json_route(
    route: Route,
    params: &[String],
    query: &HashMap<String, String>,
    wiki: &Wiki,
) -> HttpResponse {
    let limit = limit_of(query);
    let list_query = Query {
        languages: languages_of(query),
        text: text_of(query),
        offset: offset_of(query),
        limit: Some(limit),
    };
    let langs = languages_of(query);
    let key = params.first().map_or("", String::as_str);
    match route {
        Route::WikiLanguages => HttpResponse::json(
            200,
            "OK",
            &json!({ "count": wiki.languages().len(), "items": wiki.languages() }),
        ),
        Route::WikiUnsourced => HttpResponse::json(
            200,
            "OK",
            &json!({ "count": wiki.unsourced().len(), "items": wiki.unsourced() }),
        ),
        Route::WikiList(Family::Characters) => match wiki.characters(&list_query) {
            Ok(page) => HttpResponse::json(200, "OK", &page_json(&page, limit)),
            Err(error) => failure(&error),
        },
        Route::WikiList(Family::Skills) => match wiki.skills(&list_query) {
            Ok(page) => HttpResponse::json(200, "OK", &page_json(&page, limit)),
            Err(error) => failure(&error),
        },
        Route::WikiList(Family::Teams) => match wiki.teams(&list_query) {
            Ok(page) => HttpResponse::json(200, "OK", &page_json(&page, limit)),
            Err(error) => failure(&error),
        },
        Route::WikiRecord(Family::Characters) => record_json(wiki.character(key, &langs), key),
        Route::WikiRecord(Family::Skills) => record_json(wiki.skill(key, &langs), key),
        Route::WikiRecord(Family::Teams) => record_json(wiki.team(key, &langs), key),
        _ => error_json(404, "Not Found", "route not found"),
    }
}

fn not_found(wiki: &Wiki, what: &str) -> HttpResponse {
    html_response(404, "Not Found", html::not_found(wiki.languages(), what))
}

/// HTML routes (`/wiki/...`).
pub(crate) fn page_route(
    route: Route,
    params: &[String],
    query: &HashMap<String, String>,
    wiki: &Wiki,
) -> HttpResponse {
    if route == Route::PageIndex {
        return match wiki.sources() {
            Ok(sources) => html_response(
                200,
                "OK",
                html::index(wiki.languages(), &sources, wiki.unsourced()),
            ),
            Err(error) => failure(&error),
        };
    }
    let Some(tag) = params.first() else {
        return not_found(wiki, "no language in the path");
    };
    let languages: &[Language] = wiki.languages();
    let Some(lang) = wiki
        .resolve_languages(&[tag])
        .ok()
        .and_then(|folders| languages.iter().find(|l| l.folder == folders[0]))
    else {
        return not_found(
            wiki,
            &format!("language `{tag}` is not shipped by this installation"),
        );
    };
    let limit = limit_of(query);
    let params_of_list = ListParams {
        text: text_of(query),
        offset: offset_of(query),
        limit,
    };
    let list_query = Query {
        languages: vec![lang.folder.clone()],
        text: params_of_list.text.clone(),
        offset: params_of_list.offset,
        limit: Some(limit),
    };
    let key = params.get(1).map_or("", String::as_str);
    let render = |rest: String, page: &dyn Fn(&Ctx<'_>) -> String| {
        let ctx = Ctx {
            languages,
            lang,
            rest: &rest,
        };
        html_response(200, "OK", page(&ctx))
    };
    let missing = || not_found(wiki, &format!("no entry matches `{key}`"));
    let all: [String; 0] = [];
    match route {
        Route::PageList(Family::Characters) => match wiki.characters(&list_query) {
            Ok(page) => render("characters".to_owned(), &|c| {
                html::characters(c, &page, &params_of_list)
            }),
            Err(error) => failure(&error),
        },
        Route::PageList(Family::Skills) => match wiki.skills(&list_query) {
            Ok(page) => render("skills".to_owned(), &|c| {
                html::skills(c, &page, &params_of_list)
            }),
            Err(error) => failure(&error),
        },
        Route::PageList(Family::Teams) => match wiki.teams(&list_query) {
            Ok(page) => render("teams".to_owned(), &|c| {
                html::teams(c, &page, &params_of_list)
            }),
            Err(error) => failure(&error),
        },
        Route::PageRecord(Family::Characters) => match wiki.character(key, &all) {
            Ok(Some(record)) => render(format!("characters/{key}"), &|c| {
                html::character(c, &record)
            }),
            Ok(None) => missing(),
            Err(error) => failure(&error),
        },
        Route::PageRecord(Family::Skills) => match wiki.skill(key, &all) {
            Ok(Some(record)) => render(format!("skills/{key}"), &|c| html::skill(c, &record)),
            Ok(None) => missing(),
            Err(error) => failure(&error),
        },
        Route::PageRecord(Family::Teams) => match wiki.team(key, &all) {
            Ok(Some(record)) => render(format!("teams/{key}"), &|c| html::team(c, &record)),
            Ok(None) => missing(),
            Err(error) => failure(&error),
        },
        _ => error_json(404, "Not Found", "route not found"),
    }
}

/// The shared wiki of the process, built on first use.
pub(crate) fn shared(
    cell: &std::sync::OnceLock<Result<Arc<Wiki>, String>>,
    vfs: &Arc<nie_formats::vfs::Vfs>,
) -> Result<Arc<Wiki>, String> {
    cell.get_or_init(|| {
        Wiki::new(Arc::clone(vfs))
            .map(Arc::new)
            .map_err(|error| error.to_string())
    })
    .clone()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use nie_formats::cfgbin::{
        CfgEntry, RdbnList, RdbnRow, RdbnValue, Value as Cfg, encode_rdbn, encode_t2b,
    };

    use crate::serve_cmd::tests_support::{route, state_with_vfs};

    fn entry(name: &str, variables: Vec<Cfg>, children: Vec<CfgEntry>) -> CfgEntry {
        CfgEntry {
            name: name.to_owned(),
            variables,
            children,
        }
    }

    fn int(value: u32) -> Cfg {
        Cfg::Int(value as i32)
    }

    fn text_table(kind: &str, rows: Vec<CfgEntry>) -> Vec<u8> {
        encode_t2b(&[entry(kind, vec![Cfg::Int(rows.len() as i32)], rows)])
    }

    fn noun(hash: u32, label: &str) -> CfgEntry {
        let mut vars = vec![int(hash), Cfg::Int(0)];
        vars.extend((0..3).map(|_| Cfg::String(String::new())));
        vars.push(Cfg::String(label.to_owned()));
        vars.extend((0..4).map(|_| Cfg::String(String::new())));
        vars.extend((0..5).map(|_| Cfg::Int(0)));
        entry("NOUN_INFO", vars, vec![])
    }

    fn rdbn(list: &str, fields: Vec<(&str, RdbnValue)>) -> Vec<u8> {
        encode_rdbn(&[RdbnList {
            name: list.to_owned(),
            type_name: "INFO".to_owned(),
            rows: vec![RdbnRow {
                fields: fields.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
            }],
        }])
        .unwrap()
    }

    /// One language, one character on one team with one skill, and an emblem file.
    fn installation(dir: &std::path::Path) -> Arc<crate::serve_cmd::ServerState> {
        let mut base: Vec<Cfg> = (0..20).map(|_| Cfg::Int(0)).collect();
        base[0] = int(0xA000_0001);
        base[1] = Cfg::String("c00000001".to_owned());
        base[3] = int(0x1111_0001);
        base[11] = Cfg::Int(2);
        base[16] = int(0xD000_0001);
        let mut param: Vec<Cfg> = (0..12).map(|_| Cfg::Int(0)).collect();
        param[0] = int(0xB000_0001);
        param[1] = int(0xA000_0001);
        param[3] = Cfg::Int(4);
        let team_row = vec![
            ("belongTeamId", RdbnValue::Hash(0xD000_0001)),
            ("teamNameTextId", RdbnValue::Hash(0x4444_0001)),
            (
                "teamEmblemId_IE",
                RdbnValue::Hash(crc32fast::hash(b"em010001")),
            ),
        ];
        let skill_row = vec![
            ("skillID", RdbnValue::Hash(0xC000_0001)),
            ("skillIDStr", RdbnValue::Condition("whs00001".to_owned())),
            ("skillNameId", RdbnValue::Hash(0x3333_0001)),
            ("element", RdbnValue::Int(1)),
        ];
        let files: Vec<(&str, Vec<u8>)> = vec![
            (
                "data/common/gamedata/character/chara_base_1.0.cfg.bin",
                text_table(
                    "CHARA_BASE_INFO_LIST_BEG",
                    vec![entry("CHARA_BASE_INFO", base, vec![])],
                ),
            ),
            (
                "data/common/gamedata/character/chara_param_1.0.cfg.bin",
                text_table(
                    "CHARA_PARAM_INFO_LIST_BEG",
                    vec![entry("CHARA_PARAM_INFO", param, vec![])],
                ),
            ),
            (
                "data/common/gamedata/skill/skill_config_1.0.cfg.bin",
                rdbn("m_skillInfoList", skill_row),
            ),
            (
                "data/common/gamedata/character/belong_team_config_0.0.cfg.bin",
                rdbn("m_belongTeamInfoList", team_row),
            ),
            (
                "data/common/text/fr/chara_text.cfg.bin",
                text_table("NOUN_INFO_BEGIN", vec![noun(0x1111_0001, "Aki & Kino")]),
            ),
            (
                "data/common/text/fr/skill_text.cfg.bin",
                text_table("NOUN_INFO_BEGIN", vec![noun(0x3333_0001, "Tir")]),
            ),
            (
                "data/common/text/fr/team_text.cfg.bin",
                text_table("NOUN_INFO_BEGIN", vec![noun(0x4444_0001, "Raimon")]),
            ),
            (
                "data/dx11/menu/200_icon/01_icon_emblem/em010001.g4tx",
                b"x".to_vec(),
            ),
        ];
        let mut vfs = nie_formats::vfs::Vfs::new();
        for (path, bytes) in files {
            let disk = dir.join(path.replace('/', "__"));
            std::fs::write(&disk, &bytes).unwrap();
            vfs.add_overlay_file(path.to_owned(), disk, bytes.len() as u32);
        }
        state_with_vfs(vfs)
    }

    fn body(response: &crate::serve_cmd::HttpResponse) -> String {
        String::from_utf8(response.body.clone()).unwrap()
    }

    #[tokio::test]
    async fn json_routes_serve_pages_records_and_errors() {
        let dir = tempfile::tempdir().unwrap();
        let state = installation(dir.path());

        let r = route(&state, "/v1/wiki/languages").await;
        assert_eq!(r.status, 200);
        assert!(body(&r).contains("\"tag\":\"fr\""));

        let r = route(&state, "/v1/wiki/characters?lang=fr&q=kino&limit=1").await;
        assert_eq!(r.status, 200);
        let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert_eq!(
            (v["total"].as_u64(), v["limit"].as_u64()),
            (Some(1), Some(1))
        );
        assert_eq!(v["items"][0]["internal_code"], "c00000001");
        assert_eq!(v["items"][0]["gender"]["value"], 2);
        assert_eq!(v["items"][0]["param_source"]["table"], "CHARA_PARAM_INFO");

        assert_eq!(
            route(&state, "/v1/wiki/characters?lang=xx").await.status,
            400
        );
        assert_eq!(
            route(&state, "/v1/wiki/characters/nobody").await.status,
            404
        );
        assert_eq!(
            route(&state, "/v1/wiki/skills/whs00001?lang=fr")
                .await
                .status,
            200
        );

        let r = route(&state, "/v1/wiki/teams/0xD0000001").await;
        let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert_eq!(
            v["emblems"]["teamEmblemId_IE"]["files"][0],
            "data/dx11/menu/200_icon/01_icon_emblem/em010001.g4tx"
        );
        assert_eq!(v["roster"]["family"], "team roster");

        let r = route(&state, "/v1/wiki/unsourced").await;
        assert!(body(&r).contains("capsule lot contents"));
    }

    #[tokio::test]
    async fn html_pages_render_per_language_with_provenance() {
        let dir = tempfile::tempdir().unwrap();
        let state = installation(dir.path());

        let r = route(&state, "/wiki").await;
        assert_eq!(r.status, 200);
        assert!(r.content_type.starts_with("text/html"));
        assert!(body(&r).contains("chara_param_1.0.cfg.bin"));

        let r = route(&state, "/wiki/fr/characters").await;
        let page = body(&r);
        assert!(page.contains("<html lang=\"fr\">"));
        // Text from the game is escaped.
        assert!(page.contains("Aki &amp; Kino"));

        let page = body(&route(&state, "/wiki/FR/teams/0xD0000001").await);
        assert!(page.contains("/tex/dx11/menu/200_icon/01_icon_emblem/em010001.png"));

        assert_eq!(route(&state, "/wiki/xx/characters").await.status, 404);
        assert_eq!(route(&state, "/wiki/fr/skills/missing").await.status, 404);
    }

    #[tokio::test]
    async fn the_document_is_served_and_the_wiki_needs_a_vfs() {
        let r = crate::serve_cmd::tests_support::route_stateless("/openapi.json").await;
        assert_eq!(r.status, 200);
        let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert!(v["paths"]["/v1/wiki/characters/{key}"]["get"].is_object());
        let r = crate::serve_cmd::tests_support::route_stateless("/v1/wiki/languages").await;
        assert_eq!(r.status, 503);
    }
}
