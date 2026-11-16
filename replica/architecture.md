# Architecture: Inazuma Eleven portal and encyclopaedia (Rust, branch `rust`)

Built from `recon.md` and `features.csv`. We use the existing stack, not the replica-architect defaults.

| Layer | Choice | Why |
| --- | --- | --- |
| Web server | axum 0.8 in `crates/azalee-app` | already serves the wiki and the sitemap |
| UI | Dioxus SSR through `nie-ui-dioxus` (UiNode tree); the same tree goes to Bevy UI (`nie-ui-bevy`) for native | one UI tree for web and native, native first |
| Theme | `packages/rg-ui/src/tokens.json` (from `styles.css`) | no hardcoded colour |
| Text | `crates/azalee-app/locales/{en,fr,ja}.json`, negotiated per request | no hardcoded text, ja like the source |
| Game data | Postgres import from `nie-cfgsql` (vfs_*, t2b_*, rdbn_*) | real data from the player's install |
| Editorial data | `schema.sql` (news, patch notes, i18n rows) | content that has no game source |
| Assets | VFS textures decoded to PNG at `/cdn/static/*`, R2 in production | nothing redistributed from the official sites |
| Auth (writes) | rg-auth sessions checked in axum | one auth for both worlds |

## Routes

| Method and path | What | Who | Feature |
| --- | --- | --- | --- |
| GET /series | series hub | public | F01 (done) |
| GET /series/{slug} | game page | public | F02 |
| GET /news, /news/{id} | news | public | F03 |
| GET /series/{slug}/patchnotes/{platform} | patch notes | public | F05 |
| GET /zukan/chara, /zukan/chara/{id} | characters | public | F09, F10 |
| GET /zukan/skill, /zukan/item, /zukan/team, /zukan/formation | encyclopaedia | public | F11-F14 |
| POST /admin/news, /admin/patchnotes | edit content | admin | F03, F05 |
| GET /api/ui/{path} | any page as a UiNode JSON tree | public | native and WASM clients |

## Parts that bite
- **Name mapping**: cfg.bin columns are hashed or unnamed; human names come from the text tables of each language in the VFS.
- **Multilingual game text**: one row per language, read from the VFS locale folders, never machine-translated.
- **Several games**: only Victory Road is read by nie today; every other game needs its own format support in nie-formats.

## Build order
1. Vertical slice: /series (done), then /series/victory-road and /zukan/chara from the real import.
2. Must-haves: F02, F03, F09-F13.
3. Should-haves: F05, F07, F08, F14, F15.
4. Native: the same UiNode pages in the Bevy shell once vendored bevy_ui compiles.
