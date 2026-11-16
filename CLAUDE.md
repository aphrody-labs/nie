# CLAUDE.md

@AGENTS.md

- Skills: project-specific ones live in `.agents/skills` (single source); the `rosegriffon` plugin (`plugins/rosegriffon`) adds one skill per Bevy, Dioxus, Blitz, Taffy, manganis and subsecond crate generated from `third_party/` (`bun plugins/rosegriffon/scripts/gen-skills.ts`), an index pointing at `.agents/skills`, and the Steam MCP server and agent. Generic ones (bun, fleet, rust-cargo, cloudflare-platform, epic-games, gaming-stack, steam-*, reverse-engineer-anything, r2, cli-*) come from the aphrody-skills plugin.
- Vendored forks: `third_party/{bevy,dioxus,blitz,taffy}` (git submodules of aphrody-labs, `git submodule update --init --depth 1`); the whole workspace uses them through `[patch.crates-io]`; Rust builds and checks run only in the capped prd container.
- Shell `rg` may be shadowed on Windows: use the Grep tool.
- Context to start from: `docs/ARCHITECTURE.md`, `infra/services.toml`, `docs/vps/README.md`.
