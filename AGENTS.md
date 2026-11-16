# Rose Griffon: agent contract

Read by every agent CLI (Claude Code, Codex, Gemini, OpenCode, Zed). `CLAUDE.md` and `GEMINI.md` import this file.

## What this repo is

Standalone product of the Rose Griffon community (rosegriffon.fr): Next.js site, Discord gateways (`rg`, `achillea`, `azalee`), unified Elysia API, Achillea ranked (Colyseus), Azalee wiki and CLI, and `nie`, the Inazuma Eleven: Victory Road engine and studio (Rust, Bevy, Taffy, WebGPU). It depends on no other monorepo: everything it needs lives here or comes from public npm, crates.io, PyPI and GitHub upstreams.

## Layout (check with `fd -d 1`)

- `apps/`: `rg-website`, `rg-bot`, `rosegriffon` (server binary), `cli` (`ie`/`rg`), `mcp`, `iecode-dotnet`.
- `packages/`: `achillea-*`, `azalee`, `inagle`, `nie-ui`, `rg-*` shared platform.
- `crates/`: Rust 2024 workspace (`nie-*`, `yolo-re-core`), UI with Dioxus/Blitz/Taffy where Rust UI fits.
- `infra/`, `compose.yaml`, `justfile`: deploy; hosts and services come from `infra/hosts.toml` and `infra/services.toml`, nothing hardcoded.
- `.agents/skills/`: single source of skills; `plugins/steam`: Steam plugin and MCP; `.mcp.json`: MCP servers.

## Rules

- Stack: Bun 1.4.5 upstream release (never a fork), Rust 2024, uv for Python, Go for native tooling. No Node, npm, yarn or pnpm commands; `bun`, `bunx`, `cargo`, `uv`.
- Dependencies come from public registries at their newest version, read live (`bun pm view`, crates.io API, PyPI JSON). No `aphrody*` package, crate, env var, path or URL. Config names are `RG_*`; state lives under `/srv/rosegriffon`.
- Never run bare `bun test` at root: `bun test <package path>`. Heavy Rust builds run on the build host (fr1) or CI, not the workstation.
- Hosts: prd runs production (shenron and rosegriffon only), fr1 and fr2 build, preview and CI. Anything deleted on a host is listed with its size first and checked for live dependents.
- Secrets: never print, log or commit `.env` values (only `.env.example`); run gitleaks before every push.
- Proof is a screenshot of the rendered page or a real API response checked by content, never an HTTP status alone.
- No emoji and no AI attribution in code, docs or commits. UI text goes through `rg-i18n`; source language is English, `fr` is a catalogue.
- Work happens directly on `main` in logical commits; no branches, no worktrees. Say plainly what is done and what is not.
- Game assets of Inazuma Eleven (Level-5) are not redistributed: `nie` reads the user's own install.

## Architecture

- Stays in TypeScript on Bun, untouched: APIs, auth, server, the current site and the legacy achillea and azalee.
- Achillea and Azalee are rewritten fully in Rust around Bevy (WASM and WebGPU), each on its own new subdomain; the legacy keeps serving its current hostnames until the Rust versions are proven.
- Scope is every Inazuma Eleven game and platform (inazuma.jp, the whole series), not only Victory Road; code stays game-agnostic.
- UI in Rust: Dioxus on the same base as GPUI (taffy, wgpu, winit, text and layout crates deduplicated, same versions).

## Shared with the sibling product

Shenron (`C:\shenron`, dragonballfr.com) and Rose Griffon (`C:\rosegriffon`, rosegriffon.fr) share prd and part of their needs. Shared code exists once: a published crate or package, or a git dependency on the product that owns it; never a copy. The owner column is proposed; either agent may change it by editing this table in both files in the same step, and the other agent's work is never overwritten.

| Need | Owner | Consumer |
| --- | --- | --- |
| Koharu: OCR, translation, typesetting (service `koharu.`) | shenron | rosegriffon (wiki, zukan, databooks) calls the service or depends on the `koharu-*` crates |
| Video, anime, film (`tv.`): FFmpeg, libvlc, yt-dlp, HLS | shenron | rosegriffon |
| `filmcraft` (video engine): one source `github.com/aphrody-labs/filmcraft`, the vendored copies in both repos are removed | filmcraft repo | both |
| Admin dashboard (Rust, desktop first, web second) | shenron | rosegriffon plugs its own panels in |
| Auth and SSO (Better Auth: oauth-provider, passkey, api-key, agent-auth) | rosegriffon (`@rosegriffon/auth`, superset) | shenron-auth shrinks to Shenron configuration over it |
| Wiki and databook formats, CPK and game archive readers (`shenron-re` game module, `yolo-re-core`) | rosegriffon for Inazuma Eleven formats, shenron for Dragon Ball formats; one shared crate for the generic archive code | both |
| CDN (R2 origin, cdn-sync) | rosegriffon (`infra/docker/cdn-sync`) | shenron |
| Deploy on prd: compose per product, `/srv/deploy.lock` (flock) before any prd change, nginx and tunnel routes in each product's own files | each product for its own units | never touch the other product's units |

Coordination goes through files (this table, `docs/SHARED.md`, `docs/` notes, commits on `main`), not through live edits of the other repository.
