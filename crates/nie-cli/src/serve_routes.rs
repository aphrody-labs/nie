//! The route table of `nie serve`: one list feeds the router and `/openapi.json`, so the
//! document cannot drift from what the server answers.

use serde_json::{Value, json};

/// The three page families of the wiki.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family {
    Characters,
    Skills,
    Teams,
}

/// Every handler of the server; the router matches on it exhaustively.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Route {
    Health,
    Ready,
    OpenApi,
    VfsRemotes,
    FormatsDetect,
    FormatsDecode,
    VfsInfo,
    VfsFind,
    VfsStat,
    VfsRead,
    GameQuery,
    WikiLanguages,
    WikiUnsourced,
    WikiList(Family),
    WikiRecord(Family),
    PageIndex,
    PageList(Family),
    PageRecord(Family),
}

/// A documented parameter.
pub(crate) struct Param {
    name: &'static str,
    description: &'static str,
}

pub(crate) struct RouteDef {
    method: &'static str,
    pattern: &'static str,
    pub(crate) route: Route,
    tag: &'static str,
    summary: &'static str,
    query: &'static [Param],
    /// Media type of the success answer.
    content_type: &'static str,
    /// Whether a request body is expected (raw bytes).
    body: bool,
}

const LANG: Param = Param {
    name: "lang",
    description: "Languages of the labels: folders or BCP 47 tags (de en es fr it ja pt zh-Hans zh-Hant, as shipped), comma separated; every shipped language when absent",
};
const Q: Param = Param {
    name: "q",
    description: "Case-insensitive substring of names (in the selected languages) and codes",
};
const LIMIT: Param = Param {
    name: "limit",
    description: "Window size, 1 to 500 (default 50)",
};
const OFFSET: Param = Param {
    name: "offset",
    description: "Window start (default 0)",
};
const JSON_TYPE: &str = "application/json";
const HTML_TYPE: &str = "text/html";

const fn def(
    method: &'static str,
    pattern: &'static str,
    route: Route,
    tag: &'static str,
    summary: &'static str,
    query: &'static [Param],
    content_type: &'static str,
) -> RouteDef {
    RouteDef {
        method,
        pattern,
        route,
        tag,
        summary,
        query,
        content_type,
        body: false,
    }
}

pub(crate) const ROUTES: &[RouteDef] = &[
    def(
        "GET",
        "/health",
        Route::Health,
        "service",
        "Liveness and mount state",
        &[],
        JSON_TYPE,
    ),
    def(
        "GET",
        "/",
        Route::Health,
        "service",
        "Same answer as /health",
        &[],
        JSON_TYPE,
    ),
    def(
        "GET",
        "/ready",
        Route::Ready,
        "service",
        "Readiness: 200 once the VFS is mounted (or in stateless mode), else 503",
        &[],
        JSON_TYPE,
    ),
    def(
        "GET",
        "/openapi.json",
        Route::OpenApi,
        "service",
        "This document",
        &[],
        JSON_TYPE,
    ),
    def(
        "GET",
        "/v1/vfs/remotes",
        Route::VfsRemotes,
        "vfs",
        "rclone remotes",
        &[],
        JSON_TYPE,
    ),
    RouteDef {
        body: true,
        ..def(
            "POST",
            "/v1/formats/detect",
            Route::FormatsDetect,
            "formats",
            "Detect the format of the posted bytes",
            &[],
            JSON_TYPE,
        )
    },
    RouteDef {
        body: true,
        ..def(
            "POST",
            "/v1/formats/decode",
            Route::FormatsDecode,
            "formats",
            "Decode the posted bytes to JSON, or to PNG with format=png",
            &[
                Param {
                    name: "name",
                    description: "Asset name, used for texture decoding",
                },
                Param {
                    name: "format",
                    description: "`png` returns the best texture of a G4TX",
                },
            ],
            JSON_TYPE,
        )
    },
    def(
        "GET",
        "/v1/vfs/info",
        Route::VfsInfo,
        "vfs",
        "Mounted source, entry and pack counts, transport statistics",
        &[],
        JSON_TYPE,
    ),
    def(
        "GET",
        "/v1/vfs/find",
        Route::VfsFind,
        "vfs",
        "Search VFS paths",
        &[
            Param {
                name: "q",
                description: "Substring of the path",
            },
            Param {
                name: "ext",
                description: "File extension",
            },
            Param {
                name: "limit",
                description: "1 to 500 (default 50)",
            },
        ],
        JSON_TYPE,
    ),
    def(
        "GET",
        "/v1/vfs/stat",
        Route::VfsStat,
        "vfs",
        "Size and pack of one VFS path",
        &[Param {
            name: "path",
            description: "Relative VFS path",
        }],
        JSON_TYPE,
    ),
    def(
        "GET",
        "/v1/vfs/read",
        Route::VfsRead,
        "vfs",
        "Decoded entry (decode=json|png); raw bytes need the bearer token and answer 404 without it",
        &[
            Param {
                name: "path",
                description: "Relative VFS path",
            },
            Param {
                name: "decode",
                description: "`json` or `png`; absent returns raw bytes",
            },
        ],
        "application/octet-stream",
    ),
    def(
        "GET",
        "/v1/game/query",
        Route::GameQuery,
        "game",
        "Game-data rows of one domain",
        &[
            Param {
                name: "domain",
                description: "Data domain (default characters)",
            },
            Param {
                name: "q",
                description: "Filter",
            },
            Param {
                name: "limit",
                description: "1 to 500 (default 50)",
            },
        ],
        JSON_TYPE,
    ),
    def(
        "GET",
        "/v1/wiki/languages",
        Route::WikiLanguages,
        "wiki",
        "The text languages the installation ships",
        &[],
        JSON_TYPE,
    ),
    def(
        "GET",
        "/v1/wiki/unsourced",
        Route::WikiUnsourced,
        "wiki",
        "The page families the VFS gives no source for",
        &[],
        JSON_TYPE,
    ),
    def(
        "GET",
        "/v1/wiki/characters",
        Route::WikiList(Family::Characters),
        "wiki",
        "Characters: chara_param joined to chara_base, paged",
        &[LANG, Q, LIMIT, OFFSET],
        JSON_TYPE,
    ),
    def(
        "GET",
        "/v1/wiki/characters/{key}",
        Route::WikiRecord(Family::Characters),
        "wiki",
        "One character by chara_param id or internal code",
        &[LANG],
        JSON_TYPE,
    ),
    def(
        "GET",
        "/v1/wiki/skills",
        Route::WikiList(Family::Skills),
        "wiki",
        "Skills of skill_config, paged",
        &[LANG, Q, LIMIT, OFFSET],
        JSON_TYPE,
    ),
    def(
        "GET",
        "/v1/wiki/skills/{key}",
        Route::WikiRecord(Family::Skills),
        "wiki",
        "One skill by skillID or code",
        &[LANG],
        JSON_TYPE,
    ),
    def(
        "GET",
        "/v1/wiki/teams",
        Route::WikiList(Family::Teams),
        "wiki",
        "Teams of belong_team_config, paged",
        &[LANG, Q, LIMIT, OFFSET],
        JSON_TYPE,
    ),
    def(
        "GET",
        "/v1/wiki/teams/{key}",
        Route::WikiRecord(Family::Teams),
        "wiki",
        "One team by belongTeamId",
        &[LANG],
        JSON_TYPE,
    ),
    def(
        "GET",
        "/wiki",
        Route::PageIndex,
        "pages",
        "Wiki home: languages, backing tables, families without a source",
        &[],
        HTML_TYPE,
    ),
    def(
        "GET",
        "/wiki/{lang}/characters",
        Route::PageList(Family::Characters),
        "pages",
        "Character list",
        &[Q, LIMIT, OFFSET],
        HTML_TYPE,
    ),
    def(
        "GET",
        "/wiki/{lang}/characters/{key}",
        Route::PageRecord(Family::Characters),
        "pages",
        "Character page",
        &[],
        HTML_TYPE,
    ),
    def(
        "GET",
        "/wiki/{lang}/skills",
        Route::PageList(Family::Skills),
        "pages",
        "Skill list",
        &[Q, LIMIT, OFFSET],
        HTML_TYPE,
    ),
    def(
        "GET",
        "/wiki/{lang}/skills/{key}",
        Route::PageRecord(Family::Skills),
        "pages",
        "Skill page",
        &[],
        HTML_TYPE,
    ),
    def(
        "GET",
        "/wiki/{lang}/teams",
        Route::PageList(Family::Teams),
        "pages",
        "Team list",
        &[Q, LIMIT, OFFSET],
        HTML_TYPE,
    ),
    def(
        "GET",
        "/wiki/{lang}/teams/{key}",
        Route::PageRecord(Family::Teams),
        "pages",
        "Team page with resolved emblems",
        &[],
        HTML_TYPE,
    ),
];

/// The route of `method` and `path` (HEAD answers like GET) and its path parameters in order.
pub(crate) fn match_route(method: &str, path: &str) -> Option<(Route, Vec<String>)> {
    let method = if method.eq_ignore_ascii_case("HEAD") {
        "GET"
    } else {
        method
    };
    let segments: Vec<&str> = path.trim_end_matches('/').split('/').collect();
    ROUTES
        .iter()
        .filter(|def| def.method.eq_ignore_ascii_case(method))
        .find_map(|def| {
            let pattern: Vec<&str> = def.pattern.trim_end_matches('/').split('/').collect();
            if pattern.len() != segments.len() {
                return None;
            }
            let mut params = Vec::new();
            for (want, got) in pattern.iter().zip(&segments) {
                if want.starts_with('{') {
                    if got.is_empty() {
                        return None;
                    }
                    params.push((*got).to_owned());
                } else if want != got {
                    return None;
                }
            }
            Some((def.route, params))
        })
}

fn path_params(pattern: &str) -> Vec<&str> {
    pattern
        .split('/')
        .filter_map(|seg| seg.strip_prefix('{')?.strip_suffix('}'))
        .collect()
}

fn description_of(name: &str) -> &'static str {
    match name {
        "lang" => "Language folder or BCP 47 tag of the shipped text languages",
        "key" => {
            "chara_param id (0xXXXXXXXX) or internal code for characters, skillID or code for skills, belongTeamId for teams"
        }
        _ => "",
    }
}

/// The OpenAPI 3.1 description of every route of the table.
pub(crate) fn openapi() -> Value {
    let mut paths = serde_json::Map::new();
    for def in ROUTES {
        let mut parameters: Vec<Value> = path_params(def.pattern)
            .into_iter()
            .map(|name| {
                json!({
                    "name": name, "in": "path", "required": true,
                    "description": description_of(name),
                    "schema": { "type": "string" },
                })
            })
            .collect();
        parameters.extend(def.query.iter().map(|p| {
            json!({
                "name": p.name, "in": "query", "required": false,
                "description": p.description, "schema": { "type": "string" },
            })
        }));
        let mut operation = json!({
            "summary": def.summary,
            "tags": [def.tag],
            "operationId": format!(
                "{}{}",
                def.method.to_ascii_lowercase(),
                def.pattern.split(['/', '{', '}']).filter(|s| !s.is_empty())
                    .map(|s| s[..1].to_ascii_uppercase() + &s[1..])
                    .collect::<String>()
            ),
            "parameters": parameters,
            "responses": {
                "200": { "description": "Success", "content": { def.content_type: {} } },
                "400": { "description": "Invalid parameter", "content": { JSON_TYPE: { "schema": { "$ref": "#/components/schemas/Error" } } } },
                "404": { "description": "Unknown route or entry" },
                "503": { "description": "No VFS mounted", "content": { JSON_TYPE: { "schema": { "$ref": "#/components/schemas/Error" } } } },
            },
        });
        if def.body {
            operation["requestBody"] = json!({
                "required": true,
                "content": { "application/octet-stream": { "schema": { "type": "string", "format": "binary" } } },
            });
        }
        paths
            .entry(def.pattern.to_owned())
            .or_insert_with(|| json!({}))[def.method.to_ascii_lowercase()] = operation;
    }
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "nie serve",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "VFS, formats, game data and the wiki of the installation the operator owns. HEAD answers like GET on every GET route.",
            "license": { "name": "MIT OR Apache-2.0", "identifier": "MIT OR Apache-2.0" },
        },
        "tags": [
            { "name": "service" }, { "name": "vfs" }, { "name": "formats" }, { "name": "game" },
            { "name": "wiki", "description": "JSON pages computed from the VFS; see docs/wiki.md" },
            { "name": "pages", "description": "Server-rendered HTML wiki pages" },
        ],
        "paths": paths,
        "components": { "schemas": { "Error": {
            "type": "object", "required": ["error"],
            "properties": { "error": { "type": "string" } },
        } } },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(pattern: &str) -> String {
        pattern
            .split('/')
            .map(|seg| if seg.starts_with('{') { "x" } else { seg })
            .collect::<Vec<_>>()
            .join("/")
    }

    #[test]
    fn every_listed_route_is_dispatched_to_its_own_handler() {
        for def in ROUTES {
            let (route, params) = match_route(def.method, &sample(def.pattern))
                .unwrap_or_else(|| panic!("{} {} is not matched", def.method, def.pattern));
            assert_eq!(route, def.route, "{}", def.pattern);
            assert_eq!(params.len(), path_params(def.pattern).len());
        }
    }

    #[test]
    fn unlisted_routes_and_methods_are_not_matched() {
        assert!(match_route("GET", "/v1/wiki/characters/a/b").is_none());
        assert!(match_route("GET", "/v1/wiki").is_none());
        assert!(match_route("POST", "/health").is_none());
        assert!(match_route("GET", "/wiki//characters").is_none());
        assert_eq!(
            match_route("HEAD", "/health").map(|m| m.0),
            Some(Route::Health)
        );
        assert_eq!(match_route("GET", "/").map(|m| m.0), Some(Route::Health));
        assert_eq!(
            match_route("GET", "/wiki/zh-Hans/teams/0x1").map(|m| m.1),
            Some(vec!["zh-Hans".to_owned(), "0x1".to_owned()])
        );
    }

    #[test]
    fn the_document_lists_exactly_the_routes_of_the_table() {
        let doc = openapi();
        assert_eq!(doc["openapi"], "3.1.0");
        let mut documented: Vec<(String, String)> = doc["paths"]
            .as_object()
            .unwrap()
            .iter()
            .flat_map(|(path, item)| {
                item.as_object()
                    .unwrap()
                    .keys()
                    .map(move |method| (method.to_uppercase(), path.clone()))
            })
            .collect();
        let mut routed: Vec<(String, String)> = ROUTES
            .iter()
            .map(|d| (d.method.to_owned(), d.pattern.to_owned()))
            .collect();
        documented.sort();
        routed.sort();
        assert_eq!(documented, routed);
        for (_, item) in doc["paths"].as_object().unwrap() {
            for (_, op) in item.as_object().unwrap() {
                assert!(op["responses"]["200"].is_object());
                assert!(op["operationId"].as_str().is_some_and(|id| !id.is_empty()));
            }
        }
    }

    #[test]
    fn operation_ids_are_unique() {
        let doc = openapi();
        let mut ids: Vec<&str> = doc["paths"]
            .as_object()
            .unwrap()
            .values()
            .flat_map(|item| item.as_object().unwrap().values())
            .map(|op| op["operationId"].as_str().unwrap())
            .collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);
    }
}
