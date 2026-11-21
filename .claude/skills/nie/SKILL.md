---
name: nie
description: Operate the nie framework and its first app (Inazuma Eleven Victory Road) end to end - the game VFS (`nie vfs`), the nie.exe reverse-engineering tools (`nie index|rtti|disasm|pdata|mem`, plugins/exe/*), knowledge and docs (`nie atlas`, `nie wiki`), Lua 5.2 (`nie lua`, nie-lua), cfg.bin tables (nie-data), plus the CLI to MCP tool to owning plugin map and the nie.exe to wasm workflow. Use for any task that reads the game installation, decodes a format, queries the RE database, runs a game script, or touches a nie command, MCP tool or plugin.
---

# nie

nie is a reproducible full-stack framework; IEVR is its first app. Everything the IEVR app knows comes from two
artifacts of an installation the user owns: the binary `nie.exe` and the data VFS (`data/cpk_list.cfg.bin`, the CPK
files, `data/common/*`, `data/dx11/*`). `AGENTS.md` is the contract, `STYLE.md` the Rust rules, `MAP.tsv` the
structure (`just map`).

## Ground rules

- Derive, never recall: `nie --help`, `nie <cmd> --help`, `nie vfs stats`, `nie vfs formats`, `nie plugins --json`
  and the decoded tables are the source of truth. A count, size or version written in a doc is a lead to re-check.
- Never ship, copy or commit a game asset. Read in place (`game://`, local path, SSH, rclone remote). Tests build
  synthetic fixtures in a temp directory. Decompiled or lifted output of `nie.exe` stays on `fr1`, never committed.
- Heavy work (builds, lifts, RE passes) runs on the build host of `deploy/build.toml` in tmux, in your own scratch
  directory, deleted at the end. Never on the PC. Commit by path on `main`, no AI attribution.
- Licensing questions are listed in `docs/research/level5-questions.md`, nothing is decided in code.
- EAC: nothing spoofs Windows to EAC or patches, stubs or skips an EAC check (AGENTS.md, `fw-nt/third_party/LiberNT`).

## Pick the entry point

| I need | Go to |
|---|---|
| a file, folder, character or move of the game | `nie vfs ...`, [references/vfs.md](references/vfs.md) |
| a decoded table (`*.cfg.bin`) as typed Rust or JSON | [references/cfgbin-data.md](references/cfgbin-data.md) |
| a Lua script run, audited or analysed | [references/lua.md](references/lua.md) |
| functions, classes, strings, live memory of `nie.exe` | [references/re-index.md](references/re-index.md) |
| the launch chain, Steam, saves, mods, the running game | [references/nie-exe.md](references/nie-exe.md) |
| knowledge base, atlas, wiki, docs, specs | [references/knowledge-docs.md](references/knowledge-docs.md) |
| any other command (img, render, avatar, serve, bindgen, deps ...) | [references/other-commands.md](references/other-commands.md) |
| which MCP tool or plugin owns a command | [references/commands.md](references/commands.md) (generated) |
| recompile `nie.exe` toward wasm | [references/exe-to-wasm.md](references/exe-to-wasm.md) |

## CLI, MCP, plugin in one paragraph

`nie` is `crates/nie-cli` (`src/main.rs`, clap `enum Cmd`). Each top-level command has an MCP tool `cli_<command>`
(`src/mcp.rs`, `define_cli_tools!`) that runs the same code in process and takes the exact CLI tail as
`{"args": [...]}`; for example `cli_vfs` with `["find", "--ext", "g4tx", "."]`. A few native tools have no CLI twin
(`vfs_list`, `vfs_cat`, `re_query`, `asset_get`, `explorer_*`). A feature outside the nie.exe + VFS core
is a plugin (`plugins/<group>/<crate>`, `impl nie_plugin::Plugin`, `Plugin::info` lists the commands it owns);
`crates/nie-cli/src/plugins.rs` registers it and `nie plugins [--json]` prints it. Plugins of this build: `exe`
(nie-re-core), `launcher`, `save`, `steam`, `tui`, `shell-desktop`. The full table is generated, see below.

The MCP server is `.mcp.json` -> `tools/nie-mcp-launch` -> `nie-mcp` (`NIE_MCP_BIN`, else the newest local build, else
the build host over SSH). When the `nie` MCP server is down, use the CLI on the build host; the tools are the same.

## Keep the table honest

`.claude/skills/nie/references/commands.md` is generated from `main.rs`, `mcp.rs`, `plugins.rs` and every
`plugins/*/*/src/lib.rs` by `xtask/src/skill.rs`:

```sh
just skill            # rewrite the table
just skill --check    # fail when it is stale (also a unit test of nie-xtask, run by `just ci-workspace`)
```

The task also fails when an MCP tool or a plugin names a command clap does not have, when two plugins own one
command, or when a declared plugin is not registered. Change a command, a tool or a plugin: run `just skill` and
commit the table with the change. Everything else in this skill is hand-written prose; re-verify it against the
source when a command changes.

## Building and testing (where each gate runs)

- `just` lists the recipes. Local checks on the PC are light only: `just map --check`, `just skill --check`,
  `cargo test --locked -p nie-plugin --test layering`.
- `just check|lint|fmt|test|compile|repro` dispatch to the build host (`--local` / `--remote` force it);
  `just fr1 <recipe>` runs any recipe there in a detached tmux session.
- `lint|fmt|test|compile|check|repro` are `just` recipes over `cargo xtask`, not `nie` subcommands. The
  typed-coverage checks are `nie vfs lint` and `nie exe lint`.
- A UI is done only with `test-tui`, agent-desktop and agent-browser proofs and the 120 fps gate (AGENTS.md).
