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
        class: query.get("class").filter(|c| !c.is_empty()).cloned(),
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
        Route::WikiCatalog => match wiki.catalog() {
            Ok(items) => {
                HttpResponse::json(200, "OK", &json!({ "count": items.len(), "items": items }))
            }
            Err(error) => failure(&error),
        },
        Route::WikiTables => match wiki.tables() {
            Ok(items) => {
                HttpResponse::json(200, "OK", &json!({ "count": items.len(), "items": items }))
            }
            Err(error) => failure(&error),
        },
        Route::WikiTableList => {
            let class = params.get(1).map_or("", String::as_str);
            match wiki.table_records(key, class, &list_query) {
                Ok(Some(page)) => HttpResponse::json(200, "OK", &page_json(&page, limit)),
                Ok(None) => error_json(404, "Not Found", format!("no table class `{key}/{class}`")),
                Err(error) => failure(&error),
            }
        }
        Route::WikiTableRecord => {
            let class = params.get(1).map_or("", String::as_str);
            let index = params.get(2).and_then(|i| i.parse::<usize>().ok());
            let id = format!("{key}/{class}/{}", params.get(2).map_or("", String::as_str));
            match index {
                Some(index) => record_json(wiki.table_record(key, class, index, &langs), &id),
                None => error_json(400, "Bad Request", "the entry index is not a number"),
            }
        }
        Route::WikiList(Family::Characters) => match wiki.characters(&list_query) {
            Ok(page) => {
                let mut value = page_json(&page, limit);
                match wiki.character_classes() {
                    Ok(classes) => value["classes"] = json!(classes),
                    Err(error) => return failure(&error),
                }
                HttpResponse::json(200, "OK", &value)
            }
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
        let built = (|| {
            Ok::<_, Error>(html::index(
                wiki.languages(),
                &wiki.sources()?,
                &wiki.character_classes()?,
                wiki.unsourced(),
                &wiki.catalog()?,
            ))
        })();
        return match built {
            Ok(page) => html_response(200, "OK", page),
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
        class: query.get("class").filter(|c| !c.is_empty()).cloned(),
        offset: params_of_list.offset,
        limit: Some(limit),
    };
    let key = params.get(1).map_or("", String::as_str);
    let class = params.get(2).map_or("", String::as_str);
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
        Route::PageTables => match wiki.tables() {
            Ok(tables) => render("tables".to_owned(), &|c| html::tables(c, &tables)),
            Err(error) => failure(&error),
        },
        Route::PageTableList => match wiki.table_records(key, class, &list_query) {
            Ok(Some(page)) => render(format!("tables/{key}/{class}"), &|c| {
                html::table_list(c, key, class, &page, &params_of_list)
            }),
            Ok(None) => not_found(wiki, &format!("no table class `{key}/{class}`")),
            Err(error) => failure(&error),
        },
        Route::PageTableRecord => {
            let index = params.get(3).and_then(|i| i.parse::<usize>().ok());
            match index.map(|i| wiki.table_record(key, class, i, &all)) {
                Some(Ok(Some(record))) => {
                    render(format!("tables/{key}/{class}/{}", record.index), &|c| {
                        html::table_record(c, &record)
                    })
                }
                Some(Ok(None)) | None => missing(),
                Some(Err(error)) => failure(&error),
            }
        }
        Route::PageList(Family::Characters) => {
            let built = wiki
                .characters(&list_query)
                .and_then(|page| Ok((page, wiki.character_classes()?)));
            match built {
                Ok((page, classes)) => render("characters".to_owned(), &|c| {
                    html::characters(c, &page, &params_of_list, &classes)
                }),
                Err(error) => failure(&error),
            }
        }
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

    use nie_wiki_vfs::testing::installation;

    use crate::serve_cmd::tests_support::{route, route_stateless, state_with_vfs};
    use crate::serve_cmd::{HttpResponse, ServerState};

    fn state(dir: &std::path::Path) -> Arc<ServerState> {
        state_with_vfs(installation(dir))
    }

    fn body(response: &HttpResponse) -> String {
        String::from_utf8(response.body.clone()).unwrap()
    }

    #[tokio::test]
    async fn json_routes_serve_pages_records_and_errors() {
        let dir = tempfile::tempdir().unwrap();
        let state = state(dir.path());

        let r = route(&state, "/v1/wiki/languages").await;
        assert_eq!(r.status, 200);
        assert!(body(&r).contains("\"tag\":\"fr\""));

        let r = route(&state, "/v1/wiki/characters?lang=fr&q=kino&limit=1").await;
        assert_eq!(r.status, 200);
        let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert_eq!(
            (v["total"].as_u64(), v["limit"].as_u64()),
            (Some(4), Some(1))
        );
        assert_eq!(v["items"][0]["internal_code"], "c00000001");
        assert_eq!(v["items"][0]["gender"]["value"], 2);
        assert_eq!(v["items"][0]["param_source"]["table"], "CHARA_PARAM_INFO");
        // The class counts travel with the list.
        let counts: Vec<u64> = v["classes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["count"].as_u64().unwrap())
            .collect();
        assert_eq!(counts, [2, 1, 2]);

        let r = route(&state, "/v1/wiki/characters?class=base_without_param").await;
        let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert_eq!(v["total"], 2);
        assert_eq!(
            route(&state, "/v1/wiki/characters/base-2").await.status,
            200
        );

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
    async fn table_and_catalog_routes_serve_every_class_as_stored() {
        let dir = tempfile::tempdir().unwrap();
        let state = state(dir.path());

        let r = route(&state, "/v1/wiki/tables").await;
        let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert_eq!(v["count"], 12);

        let r = route(
            &state,
            "/v1/wiki/tables/chara_param/STORY_FIX_SKILL_INFO?lang=fr",
        )
        .await;
        assert_eq!(r.status, 200);
        let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert_eq!(v["total"], 1);
        assert_eq!(v["items"][0]["variables"], serde_json::json!([1, 2]));

        let r = route(&state, "/v1/wiki/tables/aura_skill_config/AURA_CMD_INFO/0").await;
        let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert_eq!(
            v["labels"]["name"]["texts"]["fr"]["forms"][0]["text"],
            "Esprit"
        );
        assert_eq!(v["refs"]["AURA_CMD_EFFECT"]["items"][0]["variables"][0], 20);

        assert_eq!(
            route(&state, "/v1/wiki/tables/chara_param/NOPE")
                .await
                .status,
            404
        );
        assert_eq!(
            route(&state, "/v1/wiki/tables/chara_param/CHARA_PARAM_INFO/x")
                .await
                .status,
            400
        );
        assert_eq!(
            route(&state, "/v1/wiki/tables/chara_param/CHARA_PARAM_INFO/99")
                .await
                .status,
            404
        );

        let r = route(&state, "/v1/wiki/catalog").await;
        assert!(body(&r).contains("\"status\":\"decoded\""));
        assert!(body(&r).contains("unrelated_config_1.00.cfg.bin"));
    }

    #[tokio::test]
    async fn html_pages_render_per_language_with_provenance() {
        let dir = tempfile::tempdir().unwrap();
        let state = state(dir.path());

        let r = route(&state, "/wiki").await;
        assert_eq!(r.status, 200);
        assert!(r.content_type.starts_with("text/html"));
        assert!(body(&r).contains("chara_param_1.00.00.00.cfg.bin"));
        assert!(body(&r).contains("unrelated_config_1.00.cfg.bin"));

        let page = body(&route(&state, "/wiki/fr/characters").await);
        assert!(page.contains("<html lang=\"fr\">"));
        assert!(page.contains("Aki Kino"));

        let page = body(&route(&state, "/wiki/FR/teams/0xD0000001").await);
        assert!(page.contains("/tex/dx11/menu/200_icon/01_icon_emblem/em010001.png"));
        let page = body(&route(&state, "/wiki/fr/tables").await);
        assert!(page.contains("STORY_FIX_SKILL_INFO"));
        let page = body(&route(&state, "/wiki/de/tables/aura_skill_config/AURA_CMD_INFO/0").await);
        assert!(page.contains("AURA_CMD_INFO #0"));

        assert_eq!(route(&state, "/wiki/xx/characters").await.status, 404);
        assert_eq!(route(&state, "/wiki/fr/skills/missing").await.status, 404);
        assert_eq!(
            route(&state, "/wiki/fr/tables/chara_param/NOPE")
                .await
                .status,
            404
        );
    }

    #[tokio::test]
    async fn the_document_is_served_and_the_wiki_needs_a_vfs() {
        let r = route_stateless("/openapi.json").await;
        assert_eq!(r.status, 200);
        let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert!(v["paths"]["/v1/wiki/characters/{key}"]["get"].is_object());
        assert!(v["paths"]["/v1/wiki/tables/{table}/{class}/{index}"]["get"].is_object());
        assert_eq!(route_stateless("/v1/wiki/languages").await.status, 503);
    }

    /// The conformance check passes on a synthetic installation, in process and over the
    /// routes, pages included: it compares every served record to the raw cfg.bin entries.
    #[tokio::test]
    async fn verify_finds_no_mismatch_on_a_consistent_wiki() {
        let dir = tempfile::tempdir().unwrap();
        let vfs = Arc::new(installation(dir.path()));
        let args = crate::wiki_verify::VerifyArgs {
            server: None,
            concurrency: 4,
            no_pages: false,
            show: 20,
        };
        let report = crate::wiki_verify::verify(&args, vfs).await.unwrap();
        report.print(20);
        assert_eq!(report.total_mismatches(), 0);
        assert!(report.checked("characters.records") >= 5);
        // Every character, skill and team has a page, and the table records outside those families too.
        let rich = report.checked("characters.records")
            + report.checked("skills.records")
            + report.checked("teams.records");
        assert!(report.checked("pages.records") > rich);
    }
}
