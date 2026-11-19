# Questions for the Level-5 partner review

Listed, never decided by contributors. Each item names the code or document that raises it. Status of every item: open.

## Anti-cheat and online services (Epic, Level-5)

1. May EasyAntiCheat (client library, service and kernel driver) run on a reimplemented NT/Win32 layer (`vendor/libernt`)? Until answered, online modes stay off there and nothing spoofs, patches, stubs or skips an EAC check. Source: plan section 10, `docs/specs/libernt-closure.json`.
2. May a test harness start `nie.exe` without the EAC runtime (offline, single-player only)? The previous `PROTON_NO_EAC_RUNTIME=1` path in `plugins/exe/nie-trace/src/proton.rs` is deleted with the Proton path and listed here for review. Also deleted, with their tests: the offline EAC patch of `nie.exe` (`nie_trace::patch_eac`, `nie mem patch-eac`, `nie-mem patch-eac`, `plugins/exe/nie-trace/scripts/patch-eac.sh`), the EAC scanner and patcher of the launcher plugin (`nie-launcher` `eac` commands, `nie launcher eac`) and the headless boot scripts that used them. Also deleted: the direct start of `nie.exe` without `EACLauncher.exe` (`plugins/exe/nie-trace/src/lancement.rs`, `nie-mem live`) and the MCP tool `game_launch` (`launch_game` in `crates/nie-cli/src/mcp.rs`). `nie` starts no game process; memory tools attach only to a `nie.exe` that the official launcher already started.
3. Which EOS product, sandbox and deployment ids (`EasyAntiCheat/Settings.json`) may appear in tests or docs? Today only hashes and counts are recorded.
4. 152 EOSSDK imports (Platform, AntiCheatClient, Connect, Auth, Lobby, Sessions, P2P and more) and the Steam flat API are called by `nie.exe`. Neither is reimplemented; is a documented stub of the offline subset acceptable?

## Game binary and recompilation

5. May lifted or recompiled code derived from `nie.exe` be published? Today `src/exe/` stays on the build host and is neither committed nor pushed; only the generator, `PROOF.json` and hashes are committed.
6. May the Lua scripts extracted from the game's VFS be compiled ahead of time (`nie-lua`, no VM) and the output distributed, or only produced on the user's own machine?
7. May a static recompilation of `nie.exe` against a Rust Win32/D3D11 runtime be distributed in binary form, or only as a tool the owner of a licensed install runs?

## Game data

8. The browser app reads the user's own installation through the File System Access API and uploads nothing. Is serving the same data from a self-hosted `nie serve` to its owner acceptable, and under which access controls?
9. Public pages derived from the VFS (wiki, typed tables, localised text in nine languages): which fields may be published, and which stay behind the owner's login?
10. Decoded textures, models, audio and video shown in the terminal explorer, the viewer and the browser: confirm they are only rendered from the user's local source and never cached outside the user's cache directory.

## Distribution

11. Is a Steam-library layout of the game on a server (`/srv/nie/steam`, one install, no copies) within the licence for the owner's own account?
12. May the launcher and mods plugins load community mods next to the original data without redistributing any game file?

## Legal texts that name other Aphrody projects (2026-11-19, isolation purge)

The isolation purge removed every code, build and documentation reference to other Aphrody
projects. These legal texts were left untouched on purpose:

13. Fifteen per-crate `LICENSE` and `LICENSE.upstream` files (crates `nie-cli`, `nie-data`,
    `nie-explore`, `nie-ffi`, `nie-model-serve`, `nie-ocgen`, `nie-render3d`, `nie-sql`, and the
    plugins `nie-camera`, `nie-computer-use`, `nie-forge`, `nie-queue`, `nie-re-core`,
    `nie-launcher`, `nie-viola`, `nie-save`) carry an agreement between Level-5 and Rose Griffon
    that names `https://github.com/aphrody-labs/rosegriffon` as the associated project and Rose
    Griffon as the licensee. Is that text still the right instrument for `nie`, or must it be
    re-issued naming `nie` and its owner?
14. `plugins/exe/nie-re-core/NOTICE` credits `shenron-re` and a former `nie-steam` source as
    Apache-2.0 origins of absorbed code. Keep the attribution as is, or restate it?
15. The root `LICENSE` is MIT with the copyright holder "Aphrody Labs", which is the owner of this
    repository (`aphrody-labs/nie`). Confirm that holder name stays.

Nothing above is decided by this repository. Answers go to the owner, who records them here with date and source.
