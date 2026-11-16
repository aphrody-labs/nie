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
