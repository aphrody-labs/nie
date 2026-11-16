# Port of Achillea, Azalee and nie to Dioxus + Bevy, driven by the game's own data

Date 2026-11-16. Builds on `docs/plans/target-architecture.md` (products, FFI contract, `rgui`,
self-hosting) and `docs/plans/rust-equivalence.md` (crate versions read from crates.io that day).
Branch `rust`. The game install lives only on prd (`/srv/rosegriffon/game/ievr/data`); locally
`NIE_GAME_SOURCE=ssh://prd/srv/rosegriffon/game/ievr/data` (remote VFS, `vfs_remote.rs`).

## Principle

The game is the source of truth. Rust is the interpreter, not the author:

```
game install --(VFS: 255,628 files, 936 CPK)--> nie-formats readers
   cfg.bin (RDBN/T2B) --> nie-cfgsql --> Postgres (prod) / SQLite (local, embedded)
   Lua scripts + layout files --> nie-lua / UI interpreter --> Dioxus (Taffy+wgpu) and Bevy scenes
   textures (g4tx) / audio / models --> asset pipeline --> R2 (cdn.rosegriffon.fr) or AOT embed
   file listing --> nie-sitemap --> sitemap.xml, robots.txt, route tree
```

No screen of Achillea, Azalee or nie is written by hand when the game already describes it.

## Crate layout (one crate per concern, merged where duplicated)

| Concern | Crate | State |
| --- | --- | --- |
| Formats, VFS, manifest | `nie-formats` | exists |
| cfg.bin to SQL (schema, rows, both dialects) | `nie-cfgsql` | added, phase 1 |
| VFS sitemap, route tree, robots | `nie-sitemap` | added, phase 2 |
| Postgres migrations, SQLite registry | `nie-sql` | exists; consumes `nie-cfgsql` output |
| Search/knowledge index | `nie-index` | exists |
| Data services | `nie-data` | exists; merges with `nie-sql` access layer |
| Scripts | `nie-lua` | exists |
| UI interpreter (game layout/Lua to a retained tree) | `nie-ui` (extends) | phase 3 |
| Dioxus renderer (shared taffy/wgpu/winit versions) | `nie-ui-dioxus` | phase 3, new |
| Bevy scenes, WebGPU, WASM | `nie-bevy`, `nie-wasm`, `nie-render3d` | exist |
| Multiplayer protocol and server | `nie-net`, `nie-game` | exist; phase 5 |
| Achillea domain (ranked, clans, tournaments, leagues, Ultimate Team) | `achillea-core`, `achillea-server`, `achillea-app`, `achillea-bot` | new, phase 6 |
| Azalee wiki app (merged with inagle and nie wiki/zukan) | `azalee-app` over `nie-wiki`, `nie-zukan` | phase 4 |
| TS/Rust bridge: C ABI for `bun:ffi` | `rg-ffi` (generates headers, OpenAPI, JSON Schemas, DB schema, tokens) | phase 7 |
| `rgui` (rg-ui to Bevy UI, Taffy, Dioxus, GPUI) | `rgui` tool | phase 7 |

Rule: Dioxus, Blitz, Bevy UI, GPUI share Taffy, wgpu, winit and text crates at one version in
`[workspace.dependencies]`; versions are re-read from crates.io before each phase.

## cfg.bin to SQL: schema strategy (implemented in `nie-cfgsql`)

- One RDBN list is one table, one row is one row, one field is one column. Table and column
  names are the game's own names, snake_cased (`sanitize_ident`), deduplicated with `_2`, `_3`.
- Implicit `row_index INTEGER PRIMARY KEY` keeps the original order (the game addresses rows by
  index).
- Type mapping: Bool BOOLEAN/INTEGER, Byte/Short/ActType SMALLINT, Int/Flag INTEGER, Hash BIGINT
  (u32 stays exact), Float REAL, Rates/Position 4 REAL columns `_0.._3`, ShortTuple 2 SMALLINT
  columns, Condition TEXT, unknown/blob BYTEA or BLOB. SQLite collapses all integers to INTEGER.
- `Invalid` (offset outside the buffer) is `NULL`; nothing is fabricated. NaN/inf floats are NULL.
- T2B (hierarchical) maps to `t2b_entry(parent, ord, name)` and `t2b_var(entry, ord, kind,
  int_value, real_value, text_value)`.
- One `Dataset` renders to both dialects, so prod and embedded stay identical.
- Next: per-file provenance table (`vfs_path`, pack, digest from `vfs_manifest`), batch COPY for
  Postgres, foreign keys discovered from Hash columns whose values match another list's key.

## VFS-driven UI pipeline

1. `Vfs::read` the layout/Lua of a screen; `nie-lua` runs it in a sandbox with a host API that
   records widget calls instead of drawing.
2. The recorded tree is a retained node tree (box, text, image, input) with Taffy styles taken
   from the game's numbers.
3. Dioxus renders it on the web/desktop (rsx generated at runtime from the tree via a generic
   `Node` component); Bevy renders it in-canvas (Bevy UI nodes + textures from g4tx).
4. Theme tokens come from the same data plus the design-system tokens exported by `rg-ffi`.

## Sitemap (implemented in `nie-sitemap`)

Pages: one per file and per ancestor directory plus the root, sorted and deduplicated.
sitemaps.org limit of 50,000 URLs per file, `sitemap.xml` becomes an index above that. Source is
the live `Vfs` (`iter_all`) or an NDJSON inventory. Also emits `robots.txt` and `tree.json` for
the router. Regenerated at startup and when `Manifest::digest` changes.

## Asset pipeline

g4tx/ico/svg/png to webp/png/svg via `nie-formats` (`textures`, `images` features); content-hashed
names; uploaded to R2 bucket `rosegriffon-cdn` (served at `cdn.rosegriffon.fr`, see
`infra/cloudflare/edge`) or embedded AOT (`include_bytes!`, LTO) for the single-binary builds.
Icons for each product come from the VFS, not from hand-drawn files.

## Multiplayer and Achillea

`nie-net` carries the real nie.exe protocol (lockstep/rollback session, lobby); `nie-game` hosts
the authoritative match. Achillea adds ranked (Elo/Glicko over match results), clans, tournaments,
leagues and Ultimate Team (card pool and drop tables read from the game's cfg.bin via
`nie-cfgsql`). Its own Discord bot is a Rust crate (`achillea-bot`) on its own subdomain; the
Rose Griffon bot stays TypeScript.

## Self-hosting

One binary per product, run by systemd, Pingora edge with in-code ACME, no Docker/Coolify
(see `target-architecture.md`). Not deployed by this work.

## Phases and proof

| # | Phase | Proof |
| --- | --- | --- |
| 1 | `nie-cfgsql` generator | `cargo nextest` in the prd container: unit tests, SQLite execution of the generated script, then a real cfg.bin from the VFS loaded into SQLite and counted |
| 2 | `nie-sitemap` generator | unit tests, then the real 255,628-entry inventory: 6 sitemap files, XML well-formed |
| 3 | UI interpreter + Dioxus/Bevy renderers | screenshot of one game screen rendered from VFS data (agent-browser) |
| 4 | Azalee app (wiki over SQL + sitemap) | screenshots of a wiki page and a directory page from the new subdomain |
| 5 | nie multiplayer | two headless clients complete a match; recorded session |
| 6 | Achillea (ranked, clans, tournaments, leagues, Ultimate Team, bot) | API responses checked by content, UI screenshots |
| 7 | `rg-ffi` contract and `rgui` | generated headers/OpenAPI/JSON Schema diffed against the TS side; one rg-ui component migrated and screenshotted |

## Status 2026-11-16

- Phase 1 done: `nie-cfgsql`; Postgres 18 and SQLite row counts identical on 4 real cfg.bin files
  (T2B with an empty string table, as in `chara_param_*.cfg.bin`, is now decoded).
- Phase 2 done: `nie-sitemap`, 255,628 files, 6 sitemap files.
- Phase 3 first slice done: `nie-ui-dioxus` (UI tree built from a cfg.bin dataset, Dioxus SSR
  renderer, taffy layout of the same tree). Proof: `docs/proofs/ui/chara_table.png`, a page rendered
  from `chara_param_table_config_0.00.00.00.cfg.bin` read through the VFS. Not done in phase 3:
  the Bevy renderer of the same tree, and the interpretation of the game's compiled Lua menus
  (`nie-lua` menu runtime still marks the native structures as unresolved).

## Status 2026-11-16 (phase 1 hardening and phase 4)

Full VFS import, measured in a throwaway postgres:18 container on prd (script written by
`nie-cfgsql --example import_vfs`, 139.5 s for the 71,103 `*.cfg.bin` plus the 255,628-entry listing,
1.03 GB of SQL, loaded with `psql` in 97 s into a 2,545 MB database):

| Table | Rows |
| --- | --- |
| `vfs_entry` (whole listing) | 255,628 |
| `vfs_file` (provenance: path, pack, size, SHA-256, format, error) | 71,103 (302 RDBN, 70,763 T2B, 38 "not a cfg.bin") |
| `vfs_table` | 142,460 |
| `t2b_entry` / `t2b_var` | 3,733,978 / 23,495,179 |
| RDBN tables in 302 schemas `rdbn_<id>` | 934 tables, 139,936 rows |

Live `count(*)` equals `vfs_table.row_count` for all 934 RDBN tables and for both T2B tables.
81 foreign keys: 4 fixed (T2B) and 77 inferred from the data (same column name, referenced column
unique with at least 4 values, every referencing value present). Columns whose value kind changes
between rows are stored as TEXT instead of failing the load.

Phase 4: `crates/azalee-app` (axum + the shared Dioxus tree) serves `/`, `/wiki/files/...` (directory
and file pages, T2B and RDBN previews, provenance), `/sitemap*.xml`, `/robots.txt` and the CDN path
`/cdn/static/<vfs path>` (g4tx decoded to PNG; set `AZALEE_CDN_BASE` to the CDN origin in
production). Screenshots taken by Chromium inside a prd container against the running server:
`docs/proofs/azalee/{home,cfg,t2b,dir,texture}.png`.
Not done: Dioxus fullstack hydration (server functions, WASM client); pages are server-rendered
Dioxus. It needs the vendored Dioxus and a wasm build.

## Merge with bun

- `crates/nie-cli/src/serve_cmd.rs:15` includes `packages/rg-config/rg-m3.json`; on `bun` that package
  is `packages/config`. The token JSON must come from the shared contract output (`rg-ffi`) instead of
  a package path; until then resolve both names at merge time.
- Cargo.lock for the new crates could not be regenerated against the full workspace: the vendored
  `third_party/bevy/Cargo.toml` fails to parse (`duplicate key` for `[profile.wasm-release]`), so the
  prd build used a reduced workspace (nie-formats, nie-lua, nie-cfgsql, nie-sitemap, nie-ui-dioxus,
  azalee-app). Regenerate the lock once the fork parses.

## Status 2026-11-16 (phases 6 and 7, Bevy)

- `achillea-core`: elo, progressive AP by tier, ranked queue and score resolution, brackets, league
  table, clan powers, tournament prizes. `tests/parity.json` is generated by bun from
  `packages/achillea-core` (4,000 cases: JavaScript `Math.round` semantics included) and replayed in
  Rust; all pass. Known difference: team-id tie-break uses byte order, the TypeScript uses
  `localeCompare`.
- `achillea-store`: schema `achillea` (players, seasons, rank tiers as data, ratings, ranked matches,
  clans with one captain and one clan per player, single-elimination tournaments with byes and
  placement prizes, leagues, Ultimate Team with rarity weights and cards imported from the nie-cfgsql
  tables). `tests/scenario.rs` runs the whole flow on postgres:18.
- `achillea-bot`: serenity 0.12.5 + poise 0.7.1; `/report` (two-sided score confirmation, unit-tested
  in `lib.rs`), `/ladder`, `/clan_create`, `/pack`. Compiled and unit-tested; it has not been run
  against Discord (no token in this environment).
- `rg-ffi-contract` + `rg-ffi`: `contract.json` generates the C header, the Rust `extern "C"` layer,
  the bun:ffi binding and the OpenAPI document; `tests/in_sync.rs` fails when a committed file is
  stale. `bun test` loads the cdylib and compares it with the TypeScript rules.
- The 38 unreadable cfg.bin were valid empty T2B files (0 entries); 0 of 71,103 are rejected now.
- Azalee serves the same page as JSON at `/api/ui/<path>`. Dioxus fullstack (server functions and
  WASM hydration) is not implemented: it needs the dioxus-fullstack stack and a wasm build of the
  client.
- `nie-bevy` follows the vendored `bevy_asset` (`cargo check -p nie-bevy --features bevy` passes).
- Bevy renderer: `crates/nie-ui-bevy` (UiNode to Bevy UI entities) is written and kept out of the
  workspace members because `bevy_ui` of the vendored fork does not compile against the vendored
  taffy (`E0053 cache_get` mutability, `E0063 missing fields clear, flex_line_count, float in
  taffy::Style`, `E0615 ScrollableOverflow::right` is a method). Fix belongs to `third_party/`.
