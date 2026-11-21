# nie.exe, the installation and the running game

`nie.exe` is the game binary of Inazuma Eleven: Victory Road (PE32+, x86-64, MSVC). It is one of the two source
artifacts of nie; the other is the VFS. nie never ships it and never copies it: tools take it as `--exe <path>`
(`NIE_EXE`), a local path or `ssh://host/abs/path` for `nie-recomp`. Every fact about it comes from a command, not from
a doc: `nie info`, `nie index`, `nie rtti`, `nie pdata`, `nie exe lint`.

## What each tool says about it

| question | command | MCP tool |
|---|---|---|
| which installation, which binary (sha256), VFS mount, launch chain | `nie info [--game-dir D] [--json]` | `cli_info` |
| sections, imports, exports | `nie index` | `cli_index` |
| classes and vtables | `nie rtti` | `cli_rtti` |
| function boundaries | `nie pdata`, `nie rebuild`, `nie recover` | `cli_pdata`, `cli_rebuild`, `cli_recover` |
| strings and who references them | `nie strings` | `cli_strings` |
| which functions have a typed Rust twin | `nie exe lint` | none |
| the running process | `nie live attach`, `nie mem ...` | `cli_live`, `cli_mem` |
| produce the binary from Rust units, share generated | `nie-forge split\|build\|verify\|report` | none (own binary) |
| lift to Cranelift, emit an object | `nie-recomp inventory\|lift` | none (own binary) |

The Rust code that replaces `nie.exe` behaviour is proven by a differential test against recorded outputs
(`STYLE.md`); the classification of what is typed is `nie exe lint`.

## Getting and launching the game

`nie` does not launch the game: Steam starts it, `nie live attach` describes it, `nie mem` reads it (`nie live` is
Linux-only, on Windows use `nie mem`).

- `nie steam list|download|sync|index` (plugin `steam`, MCP `cli_steam`): depot client in Rust over steamroom,
  app id default is IEVR, credentials from `NIE_STEAM_*` variables, state under the tooling directory, never inside the
  game directory. `nie steam index` records a Steam install into SQLite read-only (credential files by path, size and
  blake3 only). `download` and `sync` write to the target.
- The launch chain (EAC, EOS, Steamworks, signed third-party components) is supplied by Steam, which is why
  `nie steam` is the only route to a startable installation. `nie info` prints the chain.
- EAC/EOS rule (AGENTS.md): LiberNT (`fw-nt/third_party/LiberNT`) implements only documented NT/Win32 behaviour; nothing spoofs
  Windows to EAC or patches, stubs or skips an EAC check; online modes stay off until Epic and Level-5 answer.

## Saves, launcher, mods

| command | plugin | MCP | notes |
|---|---|---|---|
| `nie save read\|decrypt\|encrypt\|edit` | `save` | `cli_save` | Lives save container: decrypt to plaintext, edit one byte of a blob at an offset, re-encrypt; slot name (for example `002AB8F4-SYSTEMLIVE`) derives the key |
| `nie launcher team\|package\|save\|spirit\|ut ...` | `launcher` | `cli_launcher` | Ultimate Team launcher: team encrypt/decrypt, `.utmod` packages, save parking and team injection, UT packs, players, formations |
| `nie mod init\|add\|get\|set\|texture\|recolor\|status\|validate\|install\|uninstall` | `launcher` | `cli_mod` | the whole modding cycle, including editing (`cfgbin`/`g4tx` encoders) |
| `nie viola dump\|verify\|pack\|merge\|crypto` | `launcher` | `cli_viola` | native CPK dump, verify, pack, merge and crypto, no external toolkit |

Back up before any write to a save or an installation; `park`/`restore` exist for that.

## Reproducibility

Builds use the pinned toolchain, `--locked`, `SOURCE_DATE_EPOCH` from the commit time and `--remap-path-prefix`; never
bake a path or a timestamp into a binary. `just repro` builds HEAD twice in two directories and compares BLAKE3 of every
artifact (`docs/specs/repro-manifest.json`); CI job `repro`.
