# The VFS (`nie vfs`)

The VFS is the game's data seen through its own index: `data/cpk_list.cfg.bin` names every CPK, a logical path
(`data/common/...`, `data/dx11/...`) resolves to the CPK that holds it, CRILAYLA is decoded on read and only the
byte ranges asked for are transferred. Code: `crates/nie-formats` (`vfs`, `cpk`, `cpk_list`, `crilayla`,
`byte_source`, `vfs_remote`, `game_source`, `synthetic`), `crates/nie-assets` (the `game://` Bevy source),
`crates/nie-explore` (format-aware summaries). Nothing is extracted to disk except by an explicit `extract`.

## Where the installation is read from

Order for the root (`nie_formats::game_source::resolve`, used by `resolve_game_dir`): the `--game-dir` hint, the variables of `SOURCE_ENV` (`NIE_GAME_SOURCE`, `NIE_GAME_DIR`, `NIE_GAME_PATH`), the `game-source` file of the config dir, then the current
directory or an ancestor that holds `data/cpk_list.cfg.bin`, then the executable's directory and ancestors. A set
but empty variable is ignored. `--game-dir` on a subcommand wins.

| `NIE_GAME_SOURCE` value | transport |
|---|---|
| `/game`, `D:\Games\ievr` | local, CPK ranges read in place |
| `ssh://[user@]host[:port]/path`, `sftp://...` | SFTP over russh, no `ssh` binary |
| `r2://`, `s3://`, `gdrive://`, `rclone://remote/path` | opendal ranged reads from `rclone.conf` remotes |
| `https://host/path` | HTTP range requests |

Credentials never go in the URI. The game host and directory are `deploy/supervise.toml` `[vars]` (`game_dir`) and
`docs/specs/hosts.md`; never write them in code. Example: `NIE_GAME_SOURCE="ssh://<game-host>/<game_dir>" nie vfs ls data/common/menu`.
Full transport and serving details: `docs/serve.md`. The format and VFS contract: `docs/VFS-SPECIFICATION.md`.

## Commands

All take `--game-dir` (a path or a URI). MCP tool for all of them: `cli_vfs` with the tail as `args`.

| command | use |
|---|---|
| `nie vfs locate [--game-dir X \| --scan-root DIR] [--json] [--benchmark N]` | find or register a source without mounting it |
| `nie vfs ls [prefix]` | folders and files directly under a prefix |
| `nie vfs find <query> [--ext e] [-n N] [-j]` | substring search of internal paths (extension without the dot; default limit 100) |
| `nie vfs stat <path>` | size, containing CPK, detected format (magic) |
| `nie vfs cat <path> [--hex] [--len N] [--png-out f.png] [--wav-out f.wav]` | structured preview by detected format, hexdump as fallback; a `.g4tx` to PNG, ADX audio to WAV |
| `nie vfs extract <path-or-prefix> -o DIR [--ext e]` | write one file or a whole prefix to disk; always filter by `--ext` for a prefix, an unfiltered prefix writes every asset below it |
| `nie vfs stats [--top N]` | histogram of extensions over the whole VFS |
| `nie vfs formats [--parse] [--prefix P] [--limit N] [-j]` | format coverage: magic only by default, `--parse` runs the full decode (the only way to tell T2B containers apart) |
| `nie vfs lint [--prefix P] [--limit N] [--examples N] [--json] [--strict]` | every format parser against the VFS read in place; `--strict` fails when a format with a decoder fails on a file |
| `nie vfs chara [query] [--element E] [--position P] [--no-paths] [-n N] [-j]` | characters by localized name, hash or internal code, then their VFS files (models, textures, animations) |
| `nie vfs waza [query] [--category C] [--element E] [--no-paths] [-n N] [-j]` | skills ("waza") by name, hash or code |
| `nie vfs bundle --out F [--screen S] [--locale L] [--profile P] [--manifest-out F] [--split-dir D]` | deterministic initial browser VFS built from the local licensed installation (output is never committed) |
| `nie vfs remotes [-j]` | the rclone remotes configured |
| `nie vfs cloud --dest D [--source S] [--mode M] [--max-packs N] [--dry-run]` | export `data` to R2, S3, Google Drive or an rclone remote |

Native MCP tools with a bounded output, no CLI twin: `vfs_list`, `vfs_search`, `vfs_stat`, `vfs_cat` (UTF-8 or
base64), `asset_get` (raw, JSON, PNG or WAV). Use them for a quick look, `cli_vfs` for everything else.

## Decode and convert (not under `vfs`)

- `nie format <file-or-dir>`: the format of a file or tree, writes nothing.
- `nie decode <src> [-o out] [-q]`: JSON for data, PNG for textures, same dispatch as the FFI
  (`nie_formats::decode`); a directory is decoded in parallel with the tree preserved, unknown files are counted.
- `nie convert <vfs-path-or-file> --to png|webp|gif|jpg|bmp|tga|tiff|qoi|css|svg|json [-o f]`: a VFS path is used when
  the path is not a file on disk; an icon atlas can go to `css`, `svg` or a region manifest.
- `nie refresh-typed-json <dir> [--force] [-q]`: writes `*.cfg.bin.json` in the **tagged** form
  (`{entries}` / `{lists}`) next to each `cfg.bin`; the typed parsers of `nie-data` read that form. `nie decode`
  writes the raw generic structure, a different shape.
- `nie locales [--json] [--samples N] [--export F --langs a,b --categories c --locales-out D --seed-locales D]`: language tags actually present under `common/text/<locale>` and their
  localized resources.

## Formats in `nie-formats`

Containers and crypto: CPK (`@UTF` tables), CRILAYLA, the cpk_list encryption. Tables: RDBN and T2B (`cfg.bin`,
`objbin`, `mevbin` share a header, `vfs formats --parse` separates them). Models and animation: G4MD, G4MG, G4SK,
G4MT, G4MA, G4CM, G4PK, G4RA, G4LA, G4VS. Textures: G4TX, NXTCH. Others: NAVM (`.g4nv`), fonts, menu layouts,
USM/Sofdec2 video, Criware audio (ADX, AWB), Lua bytecode. The module list is
`crates/nie-formats/src/lib.rs`; a format counts as read only when `nie vfs lint` parses it.

## Gotchas

- Nine text languages ship (`de en es fr it ja pt zh_hans zh_hant`). `nie vfs cat` on a text file prints NOUN_INFO
  names in Japanese for most of them: compare raw bytes before concluding a language is missing.
- The extension lies (`.g4nv` carries the `NAVM` magic): trust `nie vfs formats`, not the suffix.
- Tests and CI use `nie_formats::synthetic` fixtures, never the real game; real-game tests skip when the source is absent.
- `crates/nie-formats` is `no_std` + alloc and builds for wasm32: do not add `std`-only or host-only dependencies to it.
