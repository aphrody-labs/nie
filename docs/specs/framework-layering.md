# Framework layering

nie is a framework with applications on top. IEVR (Inazuma Eleven: Victory Road) is the first
application. `docs/specs/framework-layering.json` is the source of truth: it classifies every
crate under `crates/`, `plugins/*/` and `apps/` as `framework` (application-neutral) or `app`.

The gate is `crates/nie-plugin/tests/layering.rs` (`cargo test -p nie-plugin --test layering`):

- every crate on disk is classified exactly once; a new crate must be added to the JSON;
- a framework crate names none of the `identifiers` in a `.rs`, `.toml`, `.md` or `.wgsl` file;
- a framework crate depends on no app crate.

## Result of the audit

Measured with `cargo metadata --no-deps` plus a case-insensitive count of the identifiers over the
`.rs` files of each crate.

| layer     | crates |
|-----------|--------|
| framework | `nie-plugin`, `nie-abi`, `nie-gpu`, `nie-sql`, `nie-health`, `nie-specs` (0 identifier hits each after this change; `nie-plugin` had 2 in its crate documentation, now rewritten) |
| app       | every other crate and every crate under `plugins/` (see the JSON for the list and a note per crate) |

The largest app crates by identifier hits are `nie-data` (1613), `nie-formats` (1477),
`nie-cli` (490), `nie-re` (386), `nie-viola` (332), `nie-explore` (304).

## Mixed crates and the plan

Only mechanical moves were made. The crates below hold generic code inside an app crate; each has a
`plans` entry in the JSON.

| crate | action | plan |
|-------|--------|------|
| `nie-formats` | split | new framework crate `nie-vfs`: `ByteSource`, local/SSH/rclone/HTTP sources, manifest, preload. The name resolution through `cpk_list.cfg.bin` is app code, so the split is not mechanical. |
| `nie-assets` | split | Bevy `AssetSource` over a `ByteSource` becomes framework; the `game://` scheme stays app. Needs `nie-vfs`. |
| `nie-cli` | split | host (clap, registry, MCP bridge, serve skeleton) becomes framework; IEVR commands move behind plugins. |
| `nie-ffi` | split | the contract (`crates/nie-abi/contract.json`) moves under `apps/ievr/`; `nie-ffi` becomes pure generator output. |
| `nie-edge` | promote | replace the `nie-model-serve` dependency by a route table from `nie.toml`. |
| `nie-deploy` | split | `launcher.rs` (Steam app id, Proton) moves to `plugins/launcher`. |
| `nie-link` | split | `steam.rs` and the installation scan are app code; the paged transport is generic. |
| `nie-render3d` | promote | drop the `nie-formats` dev-dependency and the game wording. |
| `nie-shell` | promote | read the contract source from the app `nie.toml`. |
| `nie-lua` | keep | the game's Lua 5.2 bytecode and the `nie.exe` host ABI. |
