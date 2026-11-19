# 2026-11-19: isolation from other Aphrody projects, and the crlib scheme

## Question

Can `nie` depend on no other Aphrody project (Rose Griffon and its apps, the Claude profile
engine, the aphrody native shell, the iecode engines, Shenron, the Bun engine), and can every
library crate of the workspace also build as a C-ABI library with generated headers that a test
keeps in sync?

## Environment

- Base commit `65db95c74` on `main` (`aphrody-labs/nie`). Toolchain `rustc 1.100.0 (0ef1bc386
  2026-11-09)`, `cargo 1.100.0 (3acc79c0c 2026-11-05)`.
- Light checks on the PC (Windows 11, `x86_64-pc-windows-msvc`): the `nie-abi` tests, in a clean
  export of the staged tree (`git archive $(git write-tree)`), with `RUSTFLAGS=` and no
  `sccache`.
- Heavy checks on the build host `fr1` (Ubuntu 26.10, kernel 7.3.0-10-generic, 12 cores, shared
  with other workers; load average 50 to 90 during the run), in `/srv/xbuild/nie/tree-idx`,
  exported from the same staged tree.

## Method

1. Isolation. Greps over tracked files for `rosegriffon`, `rose griffon`, `azalee`, `iecode`,
   `shenron`, `NSH`, `aphrody.app`, `aphrody-store`, `profile.ts`, `bun-os`, `rg_`, `RG_` and
   `aphrody`, excluding the legal texts. Each hit was removed, renamed or reworded:
   - removed: the two `iecode/` submodules and their rule, `naming.toml`, `contract/aphrody-store.toml`
     and the `store` command of `nie-shell`, `docs/ffi-unification.md`,
     `docs/CROSS-LANGUAGE-ARCHITECTURE.md`, the `rg_api_health` MCP tool, the `bun scripts/cargo.ts`
     wrapper branch of the recompile stage, the `SHENRON_*`, `YOLO_RE_ROOT` and `IECODE_GAME_PATH`
     environment aliases, the `rg-` prefix test of the atlas;
   - renamed: the JSON form called "iecode" is the "tagged" form (`to_tagged_json`,
     `encode_tagged_t2b`, `t2b_to_tagged_json`, and the others), the wiki player reference kind
     `azalee` is `wiki`, `~/.rosegriffon/cache` is `~/.nie/cache`, JSON schema ids are
     `urn:nie:schema:re:<id>`, `RG_ENGINE_ROOT` is `NIE_ENGINE_ROOT`, the bun:ffi binding is
     `openNieFfi`;
   - derived from configuration: the texture URI base is `NIE_CDN_BASE`, default `/tex` (the
     `nie-model-serve` route), the wiki host accepted by the avatar importer is `NIE_WIKI_BASE`
     (none when unset), the public hostname is named on the tunnel.
2. crlib. `crates/nie-abi/src/crlib.rs` reads `crates/nie-abi/crlib.json` and each crate's
   `abi.json`, and writes `include/nie_<id>.h` and `<crate>/src/abi_generated.rs`. The existing
   `nie-ffi` generator output stayed byte-identical (the refactor shares `rust_fn`).
   `crates/nie-abi/tests/crlib.rs` checks the manifest against `cargo metadata --no-deps`, the
   crate types, the `crlib` feature, the header presence, symbol uniqueness and drift.
3. Verification (see the results below).

## Decisions

- `nie-data` is `#![no_std]`: a `cdylib` of a `no_std` crate does not link, so it is excluded and
  its C access goes through `nie-formats`, `nie-cfgsql` and the `nie` library.
- The `crlib` cargo feature is off by default, so the C symbols never enter Rust consumers; the
  crate types are always `rlib`, `staticlib`, `cdylib`.
- The Claude profile (`tools/claude/profile.toml`, `.claude/settings.json`) was re-added to `main`
  by commits `861de2d7d` and `e7c3982aa` while this work ran; it still names the profile engine of
  another Aphrody project. Left as committed: its owner decides.
- `plugins/exe/nie-re` still links an external console emulator engine (`bun-engine`, stages
  `emit-console-rust` and the interpreter runner for NES, GBA and PSX images). Removing it means
  retiring those console recompile stages; not done here.

## Rollback

`git revert` of the two commits of this change. The removed code is in history
(`95f742707` holds `tools/azalee`, `tools/zukan`, `tools/azalee-bot`, `docs/rosegriffon-legacy`).
The two `iecode/` submodules were not initialised outside `/c/src/nie`; their gitdirs under
`.git/modules/iecode` were deleted with them, the remote repositories are untouched.

## Results

Filled in with the final commands and their output in the commit message and the lead report.
