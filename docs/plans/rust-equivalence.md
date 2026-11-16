# Rust equivalence of the Rose Griffon monorepo

Every TypeScript workspace, its role, the Rust crate that already covers it (if any) and the Rust
stack that would replace it. Crate versions read from the crates.io API on 2026-11-16. Inventory
from `docs/proofs/stack/matrix.md`.

AGENTS.md scope: the APIs, auth, server and current site stay TypeScript on Bun; Achillea and
Azalee are rewritten in Rust around Bevy. This table covers everything so the cost of each port is
visible; the "Port" column says which ones the architecture already asks for.

## Common Rust base

| Need | TS today | Rust equivalent (version) |
| --- | --- | --- |
| Async runtime | Bun | tokio 1.54.1 (already in 5 crates) |
| HTTP server, middleware | Elysia, Bun.serve | axum 0.8.9 + tower-http 0.7.1 (no crate uses an HTTP framework today; 6 hand-written `TcpListener` servers) |
| OpenAPI | @elysiajs/swagger | utoipa 6.0.0 + utoipa-axum 0.3.0 |
| Validation | TypeBox (Elysia), zod | garde 0.23.0 or validator 0.21.0 |
| Postgres | Bun.SQL, pg, Kysely, drizzle, Supabase | sqlx 0.9.0 (compile-checked SQL; today tokio-postgres in nie-sql) |
| SQLite | bun:sqlite | rusqlite 0.43.0 (already in 10 crates) |
| Redis | Bun.redis | redis 1.8.2 with its async connection manager (replace r2d2 in nie-cli) |
| Object storage (R2) | Bun.s3, Supabase Storage | object_store 0.14.2 or aws-sdk-s3 1.159.0 |
| HTTP client | fetch | reqwest 0.13.5 (already in 4 crates) |
| Images | sharp, Bun.Image | image 0.25.10 + fast_image_resize 6.1.0 |
| Templates / SSR HTML | Next.js / React | askama 0.16.1 or minijinja 3.1.0 (server HTML), dioxus 0.7.10 or leptos 0.8.23 (full-stack UI) |
| i18n | rg-i18n | fluent 0.17.0 or rust-i18n 4.2.4 |
| Shared types to TS | rg-types, achillea-types | typeshare 1.0.5, specta 1.0.5, schemars 1.2.3 (JSON Schema, achillea-contract already exports Rust types) |
| Cron / jobs | rg-cron | tokio-cron-scheduler 0.15.1, apalis 1.2.0 (queues on Redis/Postgres) |
| CLI | apps/cli | clap 4.6.7 (nie-cli already uses it) |
| MCP server | apps/mcp | rmcp 3.10.0 |
| Auth | Better Auth (rg-auth) | oauth2 5.0.0, openidconnect 4.1.0, webauthn-rs 0.6.2, jsonwebtoken 12.0.0, argon2 0.6.0 (no single equivalent of Better Auth) |
| Discord | discord.js (98 files in rg-bot) | serenity 0.12.5 + poise 0.7.1 (commands), or twilight-gateway 0.18.0 |
| Payments | Stripe SDK | async-stripe 0.41.0 |
| Realtime game rooms | Colyseus (achillea ranked) | bevy_replicon 0.45.5 or lightyear 0.31.0 on bevy 0.20.1 |

## Workspace by workspace

| Workspace (TS files) | Role | Existing Rust counterpart | Rust stack | Port |
| --- | --- | --- | --- | --- |
| apps/rg-website (1201) | Next.js site | none | dioxus or leptos full-stack, axum, sqlx | no (stays TS) |
| apps/rg-bot (381) | Discord bot + Elysia API + Achillea API | none | serenity + poise, axum, sqlx, redis | no (stays TS) |
| apps/rosegriffon (8) | self-hostable server binary | none | axum single binary | no |
| apps/cli (2) | `ie`/`rg` CLI | nie-cli | clap, merge into nie-cli | possible |
| apps/mcp (2) | MCP server | none | rmcp | possible |
| packages/achillea-core (60), achillea-client (33), achillea-types (37), achillea-contract (16) | Achillea rules, client, DTOs | achillea-contract exports Rust types | one `achillea` crate (rules) + typeshare for the TS client | yes (Achillea Rust/Bevy rewrite) |
| packages/azalee (150) | wiki library and CLI | nie-wiki, nie-data, nie-zukan | merge into nie-wiki | yes (Azalee Rust/Bevy rewrite) |
| packages/inagle (313) | game data API, SQLite, drizzle | nie-data, nie-index, nie-sql | sqlx + rusqlite in nie-data / nie-sql | yes (data layer feeds Azalee) |
| packages/bun-engine-formats (21) | engine formats | nie-formats, yolo-re-core | already Rust | yes (drop the TS copy) |
| packages/nie-ui (17) | emulator web UI | nie-ui (Taffy), nie-render3d | dioxus on the shared taffy/wgpu base | yes |
| packages/rg-db (35) | Postgres facade | nie-sql | sqlx | only with the APIs |
| packages/rg-cron (111) | scheduled tasks | none | tokio-cron-scheduler or apalis | only with the APIs |
| packages/rg-auth (18) | Better Auth guards | none | oauth2/openidconnect/webauthn-rs | no (auth stays TS, shared with Shenron) |
| packages/rg-ui (118), rg-app-shell (20) | React UI, app chrome | nie-ui | dioxus components | no (site stays TS) |
| packages/rg-i18n (9) | i18n core | none | fluent | with the Rust apps |
| packages/rg-types (30) | shared types | none | typeshare from Rust structs | with the Rust apps |
| packages/rg-config (13), rg-assets (2) | configs, assets | none | not code to port | no |
| packages/patreon-bun (177) | Patreon API client | none | reqwest + serde client | no |
| packages/filmcraft (6) | video engine wrapper | filmcraft repo | owned by the filmcraft repo | no |

## Order if the port goes ahead

1. Achillea: `achillea` crate from achillea-core + achillea-contract, bevy_replicon for ranked rooms, axum for its HTTP API; typeshare keeps the TS client in sync while the legacy site serves.
2. Azalee: inagle + azalee into nie-data / nie-wiki (sqlx, rusqlite), Bevy/WebGPU front on its own subdomain.
3. Shared base from section 1 set once in `[workspace.dependencies]` (axum, sqlx, redis async, object_store) so both apps use the same versions.

Proof per step: `cargo nextest` on the build role (prd) or CI, the WASM build served on its
subdomain, an agent-browser session inside the Bevy canvas, screenshots in `docs/proofs/`.
