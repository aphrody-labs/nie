# Reverse engineering and the index (`plugins/exe/*`)

The plugin `exe` (package `nie-re-core`, `plugins/exe/nie-re-core`) owns these commands, all in the `host` build:
`index`, `rtti`, `disasm`, `pdata`, `rebuild`, `recover`, `propagate`, `strings`, `seed`, `seed-ui`, `queue`,
`coverage`, `atlas`, `mem`, `computer-use`. MCP tool of each: `cli_<command>` (`cli_seed_ui`, `cli_computer_use`).
`nie exe lint` (MCP: none) belongs to `nie-cli`, not to the plugin. Authoritative list: [commands.md](commands.md).

The knowledge database is SQLite, default `var/nie.sqlite` (`--db`; `NIE_SQLITE` is read by the MCP server only), schema
`plugins/exe/nie-index/src/schema.sql`: `binary`, `section`, `function`, `xref`, `str`, `func_str_ref`, `func_const`,
`rtti_class`, `rtti_base`, `pdata_func`, `symbol`, `format`, `hash_name`, `anchor`, `hypothesis`, `coverage`,
`forge_unit`, plus the camera tables `cam_*`. Redis is optional (`NIE_REDIS`, default `redis://127.0.0.1/`).
Read it with the MCP tools `re_query` (read-only SQL), `re_function` (by name fragment or virtual address) and
`re_coverage`.

## The loop, in order

Every step takes `--exe <nie.exe>` (a path; `NIE_EXE` is the variable of `nie-recomp`) and `--db`. Run them on the
build host, on the binary of an installation you own; the binary is read in place.

| step | command | what it writes | gate |
|---|---|---|---|
| 1 triage | `nie index --exe E --db D` | PE/ELF sections, imports, exports | `nie coverage --db D` runs, `binary` has one row |
| 2 types | `nie rtti --exe E --db D` | MSVC RTTI classes and bases | `rtti_class` is non-empty |
| 3 bounds | `nie pdata --exe E --db D` | authoritative function starts from `.pdata`, measures Ghidra misalignment | `pdata_func` is non-empty |
| 4 edges | `nie disasm --exe E --db D` | missing call edges from iced-x86 over `.text` | `xref` grows |
| 5 strings | `nie strings --exe E --db D [--min-len N] [--section S] [--no-xrefs] [--dry-run] [--sample N]` | ASCII and UTF-16LE strings and the functions that reference them | no row when `--dry-run` |
| 6 rebuild | `nie rebuild --exe E --db D [--rounds N]` | rebases the map on `.pdata`, re-anchors Ghidra, disasm and propagate; honest coverage | `nie coverage` |
| 7 leaves | `nie recover --exe E --db D [--dry-run] [--ghidra-csv F]` | leaf functions invisible to `.pdata` | `--dry-run` first |
| 8 spread | `nie propagate --db D [--rounds N]` | labels on the call graph, string anchoring and label spreading | `nie coverage` |
| 9 seed | `nie seed --db D --json research/nie-index.json [--exe E]`, `nie seed-ui --db D [--textures]` | merged Ghidra knowledge, UI names into `hash_name` (reverses the CRC-32 in `cfg.bin`, `objbin` and the binary) | counts printed |
| 10 measure | `nie coverage --db D`, `nie exe lint --db D [--json]` | classified-function coverage; which classes and functions have a typed Rust equivalent | `nie exe lint` |

`nie queue push|pop|len|reset` is the Redis BFS frontier shared by parallel workers (`--redis`, `--tag`). `nie atlas`
is the single index over every RE surface, see [knowledge-docs.md](knowledge-docs.md).
`nie computer-use` is a bounded probe of the native `nie.exe` or Ghidra surface; it never runs a shell.

Ghidra export script and CSV are inputs, not part of the repository. `NIE_RE_GHIDRA_ARG_0`, `GHIDRA_INSTALL_DIR`
and `docs/specs/pe.md` are the external references; `docs/NIE-EXE-REVERSE-ENGINEERING.md` is a prose map of the binary
(addresses, constants) - treat every address in it as a lead and re-read it from `nie index`/`pdata` output.

## Live memory (`nie mem`, `nie live`)

`nie mem` attaches to a running `nie.exe`, never launches or stops it. Backend by platform: `OpenProcess` and
`ReadProcessMemory` on Windows, `process_vm_readv` on Linux. Sub-operations: `maps` (ranges), `base` (module base),
`read` (hex dump or `-o` raw), `write` (explicit, read and write mode), `dump` (readable ranges to a directory),
`scan` (hex `48 8B 0D`, `str:Closing`, `wstr:Title`), `lua-field` (a Lua table field the engine sets at runtime),
`palettes` (avatar editor colour presets), `catalog` (AOB signatures and derived offsets, `nie-trace::catalog`),
`recette` (apply a live-modding recipe `.txt`, dry run unless `--force`). `nie live attach [-j]` describes the running
process (pid, module base, ptrace scope) and works on Linux only. MCP: `cli_mem`, `cli_live`.

## Crates behind the plugin (libraries and standalone binaries)

| crate | role |
|---|---|
| `nie-re-core` | facade and the `ExePlugin`; the `nie-re-core` binary |
| `nie-re` | the core: PE/ELF/Mach-O triage (goblin), x86-64 disassembly (iced-x86), `.pdata` and minidump readers, byte-exact PE64 forge (`pe_forge`), Ghidra P-Code lifting, the staged pipeline (`pipeline/`) |
| `nie-pe` | thin facade over `nie_re::pe_forge` (PE64 model, rebuild, COFF) |
| `nie-index` | `Db`, schema, atlas, FTS (tantivy), bounded memory index that builds for wasm |
| `nie-seed` | ingestion of the merged knowledge (Ghidra index JSON, RTTI classes, format catalog, UI names) |
| `nie-queue` | memory frontier (wasm-safe) and the Redis frontier |
| `nie-trace` | live memory read/write, AOB catalog, recipes; binaries `nie-mem`, `nie-edit` |
| `nie-camera` | camera model, G4CM codec, configs, live camera; binary `nie-cam` |
| `nie-forge` | production chain of `nie.exe`: split into units, build, verify byte identity, report the share produced by Rust; binary `nie-forge split\|build\|verify\|report\|match\|unit` |
| `nie-computer-use` | bounded offline image and Ghidra probes |
| `nie-recomp`, `nie-win32` | static recompilation and its runtime, see [exe-to-wasm.md](exe-to-wasm.md) |

Rules: no hardcoded path or host in non-test code, no game byte committed, the decompiled and lifted output stays on
the build host (AGENTS.md copyright boundary, `docs/research/level5-questions.md` questions 5 and 7).
