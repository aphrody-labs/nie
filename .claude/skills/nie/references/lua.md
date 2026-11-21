# Lua (`nie-lua`)

The game drives menus, scenes and events with Lua 5.2 bytecode (`.lua.bin`, under `data/common/script/lua/`). To
reproduce it exactly, nie runs those chunks in the same VM: `crates/nie-lua` uses mlua with PUC-Rio Lua 5.2.4 vendored
and loads the bytecode directly. A native AOT (retiring the mlua VM) is in progress: `nie_lua::aot`, `rt` and
`inspection` already exist next to the VM, check their state before relying on either.

## Commands

| command | MCP | use |
|---|---|---|
| `nie lua <src> [--functions] [--calls] [--strings] [--crc32] [--limit N]` | `cli_lua` | static analysis (tree-sitter) of a `.lua` source file or tree, never executed; aimed at decompiled scripts (`data/lua_scripts/decompiled/`): declared functions, call targets by frequency, string literals, strings passed to `CRC32` with their hash, syntax errors the decompiler left. Bytecode is not read here |
| `nie lua-run <script> [--game-dir D] [--instruction-limit N] [--menu-host] [--disassemble]` | `cli_lua_run` | run one raw `.lua.bin` in the VM; `<script>` is a disk path, a logical VFS path (`data/common/script/lua/menu/...`) or a basename resolved through the VFS; `INCLUDE` resolves from the VFS; `--menu-host` installs the engine's menu command stubs; `--disassemble` adds the Lua 5.2 listing to the JSON |
| `nie lua-audit [--game-dir D] [--prefix P] [--instruction-limit N] [--menu-host]` | `cli_lua_audit` | run every raw chunk of the VFS (or a prefix) and measure the holes of the host implementation of engine calls |
| `nie mem lua-field ...` | `cli_mem` | read a Lua table field from the live game (`listRowNum`, `pageNum`); the chunk declares the key, only the engine process holds the value |

Instruction limits default to a bounded value (see `--help`); `0` is unlimited and reserved for terminating scripts.
Output is JSON. Use `lua-audit` before claiming the host covers a script family.

## Library map (`crates/nie-lua/src`)

`lib.rs`: `new_vm`, `load_bytecode`, `validate_bytecode`, `is_lua52_bytecode`, `crc32`, `install_include`,
`index_script_paths`, `resolve_script_path`, `discover_host_calls` (names of the engine calls a chunk makes). Modules:
`bytecode` (decoder, shared with `nie-formats::decode` for `.lua.bin`), `host`, `menu_host` (`MenuState`,
`install_menu_host`, `run_menu`), `menu_runtime`, `menu_scene`, `menu_state`, `runtime` (`ExecOptions`, `RuntimeContext`),
`session` (`MenuEvent`, `MenuCallback`), `static_analysis` (feature `analysis`), `aot`, `rt`, `inspection`. The VM
features are off on wasm32.

`unsafe`: loading arbitrary bytecode needs `Lua::unsafe_new`, a malformed chunk can crash the VM; the crate documents
the invariant and bounds execution with an instruction limit. Never run an untrusted chunk without a limit.

## Where Lua meets the rest

- The menu catalogue and screens: `nie mode`, `nie atlas menu`, `crates/nie-formats` `menu*` modules.
- Driving `nie-ui` GPUI views from the game's real Lua, and Lua mods in a Bevy plugin on stand-in assets, do not exist
  yet; do not describe them as working.
- CRC-32 names in scripts are reversed through `hash_name` (`nie seed-ui`), see [re-index.md](re-index.md).
