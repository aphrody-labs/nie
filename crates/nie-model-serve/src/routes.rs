//! The route table of `nie-model-serve`: one list documents every route and generates
//! `/openapi.json`, and the tests of the binary check the table against the handler in both
//! directions, so the document cannot drift from what the server answers.
//!
//! Every route reads the game VFS (or derives from it) and nothing else. A `{path}` segment is a
//! VFS path relative to `data/`, slashes included.

use serde_json::{Value, json};

/// A documented query parameter.
pub struct Param {
    /// Parameter name.
    pub name: &'static str,
    /// What it selects.
    pub description: &'static str,
}

/// One route.
pub struct RouteDef {
    /// Path pattern. A final `{name}` segment takes the rest of the path, slashes included.
    pub pattern: &'static str,
    /// Another path answered exactly like the pattern.
    pub alias: Option<&'static str>,
    /// OpenAPI tag.
    pub tag: &'static str,
    /// One-line summary.
    pub summary: &'static str,
    /// Query parameters.
    pub query: &'static [Param],
    /// Media type of the success answer.
    pub content_type: &'static str,
    /// Gated by the bearer token (`NIE_RAW_VFS_TOKEN`): the game's untouched bytes.
    pub private: bool,
}

const JSON: &str = "application/json";
const PNG: &str = "image/png";
const GLB: &str = "model/gltf-binary";

const PATH: Param = Param {
    name: "path",
    description: "VFS path",
};
const LIMIT: Param = Param {
    name: "limit",
    description: "Window size",
};
const OFFSET: Param = Param {
    name: "offset",
    description: "Window start",
};

const fn route(
    pattern: &'static str,
    tag: &'static str,
    summary: &'static str,
    query: &'static [Param],
    content_type: &'static str,
) -> RouteDef {
    RouteDef {
        pattern,
        alias: None,
        tag,
        summary,
        query,
        content_type,
        private: false,
    }
}

const fn with_alias(mut def: RouteDef, alias: &'static str) -> RouteDef {
    def.alias = Some(alias);
    def
}

const fn private(mut def: RouteDef) -> RouteDef {
    def.private = true;
    def
}

/// Every route of the server.
pub const ROUTES: &[RouteDef] = &[
    route("/health", "service", "Liveness and mount state", &[], JSON),
    route("/openapi.json", "service", "This document", &[], JSON),
    route(
        "/vfs/ls",
        "vfs",
        "One folder of the VFS, paged, with the role of each entry",
        &[PATH, LIMIT, OFFSET],
        JSON,
    ),
    route(
        "/vfs/find",
        "vfs",
        "Substring search over VFS paths",
        &[
            Param {
                name: "q",
                description: "Substring",
            },
            Param {
                name: "ext",
                description: "Extension filter",
            },
            LIMIT,
        ],
        JSON,
    ),
    route(
        "/vfs/stat",
        "vfs",
        "Metadata, export formats and description of one file",
        &[PATH],
        JSON,
    ),
    route(
        "/vfs/index",
        "vfs",
        "Canonical manifest of the VFS, NDJSON",
        &[],
        "application/x-ndjson",
    ),
    route(
        "/vfs/digest",
        "vfs",
        "Header of the canonical manifest",
        &[],
        JSON,
    ),
    private(route(
        "/vfs/hash",
        "vfs",
        "Digest of the game's own bytes",
        &[PATH],
        JSON,
    )),
    route(
        "/vfs/stats",
        "vfs",
        "Extension histogram of the VFS",
        &[],
        JSON,
    ),
    private(route(
        "/raw/{path}",
        "bytes",
        "The untouched bytes of a VFS file",
        &[],
        "application/octet-stream",
    )),
    private(route(
        "/export/{path}",
        "bytes",
        "A file converted to the requested format; `raw` (the default) is gated",
        &[
            Param {
                name: "format",
                description: "Target format id, as `/vfs/stat` lists them for this file",
            },
            Param {
                name: "id",
                description: "AWB waveform id (wav)",
            },
            Param {
                name: "cue",
                description: "ACB cue index (wav)",
            },
        ],
        "application/octet-stream",
    )),
    route(
        "/tex/{path}",
        "textures",
        "A G4TX texture as PNG; `<file>.g4tx/<name>.png` addresses one named texture",
        &[],
        PNG,
    ),
    route(
        "/tex-info/{path}",
        "textures",
        "Catalogue of the textures and regions of a G4TX container",
        &[Param {
            name: "css",
            description: "Return the sprite stylesheet instead of JSON",
        }],
        JSON,
    ),
    with_alias(
        route(
            "/ui/font-metrics.json",
            "ui",
            "Metrics of the game font, from font.cfg.bin",
            &[],
            JSON,
        ),
        "/ui/font-metrics",
    ),
    with_alias(
        route(
            "/ui/theme.json",
            "ui",
            "FONT_COLOR palette and font families of the game",
            &[],
            JSON,
        ),
        "/ui/theme",
    ),
    route(
        "/ui/text.png",
        "ui",
        "A text rendered with the game's bitmap font",
        &[
            Param {
                name: "t",
                description: "Text, at most 120 characters",
            },
            Param {
                name: "fg",
                description: "Colour, RRGGBB",
            },
        ],
        PNG,
    ),
    with_alias(
        route(
            "/icons/index.json",
            "ui",
            "Index of the named icons of the menu atlases, computed from the VFS",
            &[],
            JSON,
        ),
        "/icons/index",
    ),
    route(
        "/avatar/icon/{name}",
        "ui",
        "One avatar editor thumbnail, decoded from its atlas",
        &[],
        PNG,
    ),
    with_alias(
        route(
            "/menu-tree.json",
            "ui",
            "Every menu screen: layers, resources and commands from the `*_setting.cfg.bin` files",
            &[],
            JSON,
        ),
        "/menu-tree",
    ),
    route("/menu-tree/{screen}", "ui", "One menu screen", &[], JSON),
    route(
        "/cfg/{path}",
        "data",
        "A cfg.bin, objbin, fxbin or mevbin as JSON",
        &[],
        JSON,
    ),
    route(
        "/typed/{path}",
        "data",
        "A cfg.bin as its typed `nie-data` game structure",
        &[],
        JSON,
    ),
    route(
        "/lip/{path}",
        "data",
        "A p3lip lip-sync track as timed visemes",
        &[],
        JSON,
    ),
    route(
        "/model-full/{code}",
        "models",
        "An assembled character, keshin or armour as GLB",
        &[],
        GLB,
    ),
    route(
        "/model-report/{code}",
        "models",
        "Assembly report of a model: pieces, sources, textures, skinning",
        &[],
        JSON,
    ),
    route(
        "/model-chr/{path}",
        "models",
        "A generic model of `common/chr/_<sub>/` (techniques, items, animals)",
        &[],
        GLB,
    ),
    route(
        "/model-avatar/{path}",
        "models",
        "An avatar editor part or assembled avatar as GLB",
        &[],
        GLB,
    ),
    route(
        "/model-edit/{path}",
        "models",
        "A model with a replaced texture, as GLB",
        &[],
        GLB,
    ),
    route(
        "/model-map/{path}",
        "models",
        "A stage or map model as GLB",
        &[],
        GLB,
    ),
    route(
        "/model-tree/{root}/{path}",
        "models",
        "A standalone model of one of the `data/common/` model trees as GLB",
        &[],
        GLB,
    ),
    route(
        "/audio-info/{path}",
        "audio",
        "Cues of an ACB bank",
        &[],
        JSON,
    ),
    route(
        "/audio/{path}",
        "audio",
        "A cue of an ACB or AWB as WAV",
        &[
            Param {
                name: "id",
                description: "AWB waveform id",
            },
            Param {
                name: "cue",
                description: "ACB cue index",
            },
        ],
        "audio/wav",
    ),
    with_alias(
        route(
            "/video/catalog.json",
            "video",
            "Every cutscene of `data/common/movie`, computed from the VFS in the background (503 until ready)",
            &[],
            JSON,
        ),
        "/video/catalog",
    ),
    route(
        "/video/{path}",
        "video",
        "A USM cutscene as MP4 (`?track=audio`: the soundtrack as WAV, `?info=1`: its record)",
        &[
            Param {
                name: "track",
                description: "`audio` for the WAV soundtrack",
            },
            Param {
                name: "info",
                description: "Return the film record as JSON",
            },
        ],
        "video/mp4",
    ),
];

/// The static prefix of a pattern: everything before its `{…}` segment.
#[must_use]
pub fn prefix_of(pattern: &str) -> &str {
    pattern.split('{').next().unwrap_or(pattern)
}

/// The routes whose pattern or alias matches `path`, most specific first.
#[must_use]
pub fn match_path(path: &str) -> Option<&'static RouteDef> {
    let wildcard = |def: &&RouteDef| {
        def.pattern.contains('{') && {
            let prefix = prefix_of(def.pattern);
            path.strip_prefix(prefix)
                .is_some_and(|rest| !rest.is_empty())
        }
    };
    ROUTES
        .iter()
        .find(|def| def.pattern == path || def.alias == Some(path))
        .or_else(|| ROUTES.iter().find(wildcard))
}

/// A sample path that `match_path` resolves to `def` and its handler accepts as well-formed.
#[must_use]
pub fn sample(def: &RouteDef) -> String {
    def.pattern
        .replace("{root}", "map")
        .replace("{path}", "x")
        .replace("{code}", "x")
        .replace("{name}", "x")
        .replace("{screen}", "x")
}

fn path_params(pattern: &str) -> Vec<&str> {
    pattern
        .split('/')
        .filter_map(|seg| seg.strip_prefix('{')?.strip_suffix('}'))
        .collect()
}

/// The `paths` object of the OpenAPI description of this server. A `{path}` parameter carries a
/// VFS path: slashes are not escaped.
#[must_use]
pub fn openapi_paths() -> serde_json::Map<String, Value> {
    let mut paths = serde_json::Map::new();
    for def in ROUTES {
        let mut parameters: Vec<Value> = path_params(def.pattern)
            .into_iter()
            .map(|name| {
                json!({
                    "name": name, "in": "path", "required": true,
                    "description": "Path or code, as the route's summary says; slashes allowed for a path",
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
        let operation_id: String = def
            .pattern
            .split(['/', '{', '}', '.', '-'])
            .filter(|s| !s.is_empty())
            .map(|s| s[..1].to_ascii_uppercase() + &s[1..])
            .collect();
        let mut operation = json!({
            "summary": def.summary,
            "tags": [def.tag],
            "operationId": format!("get{operation_id}"),
            "parameters": parameters,
            "responses": {
                "200": { "description": "Success", "content": { def.content_type: {} } },
                "400": { "description": "Invalid path or parameter" },
                "404": { "description": "The VFS has no such entry" },
            },
        });
        if def.private {
            operation["security"] = json!([{ "bearer": [] }]);
        }
        paths.insert(def.pattern.to_owned(), json!({ "get": operation }));
    }
    paths
}

/// The OpenAPI 3.1 description of every route of the table.
#[must_use]
pub fn openapi() -> Value {
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "nie-model-serve",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Assets of the installation the operator owns, decoded from the VFS on request: textures, audio, video, cfg tables and models. HEAD answers like GET.",
            "license": { "name": "MIT OR Apache-2.0", "identifier": "MIT OR Apache-2.0" },
        },
        "tags": [
            { "name": "service" }, { "name": "vfs" }, { "name": "bytes" }, { "name": "textures" },
            { "name": "ui" }, { "name": "data" }, { "name": "models" }, { "name": "audio" },
            { "name": "video" },
        ],
        "paths": openapi_paths(),
        "components": { "securitySchemes": { "bearer": { "type": "http", "scheme": "bearer" } } },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_resolves_to_itself() {
        for def in ROUTES {
            let found = match_path(&sample(def))
                .unwrap_or_else(|| panic!("{} is not matched", def.pattern));
            assert_eq!(found.pattern, def.pattern);
            if let Some(alias) = def.alias {
                assert_eq!(match_path(alias).map(|d| d.pattern), Some(def.pattern));
            }
        }
        assert!(match_path("/nope").is_none());
        assert!(match_path("/tex/").is_none());
    }

    #[test]
    fn the_document_lists_exactly_the_routes_of_the_table() {
        let doc = openapi();
        assert_eq!(doc["openapi"], "3.1.0");
        let mut documented: Vec<&str> = doc["paths"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let mut routed: Vec<&str> = ROUTES.iter().map(|d| d.pattern).collect();
        documented.sort_unstable();
        routed.sort_unstable();
        assert_eq!(documented, routed);
    }

    #[test]
    fn operation_ids_are_unique() {
        let doc = openapi();
        let mut ids: Vec<&str> = doc["paths"]
            .as_object()
            .unwrap()
            .values()
            .map(|item| item["get"]["operationId"].as_str().unwrap())
            .collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);
    }
}
