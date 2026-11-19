# cfg.bin tables (`nie-formats`, `nie-data`, `nie-cfgsql`)

`*.cfg.bin` is the Level-5 table container. Two wire formats share the extension: RDBN (flat typed lists, magic
`RDBN`) and T2B (hierarchical nodes; `objbin` and `mevbin` use the same header). Which one a file is, only a full decode
settles: `nie vfs formats --parse`. Game tables live in `data/common/gamedata/**.cfg.bin`, texts in
`data/common/text/<locale>/*.cfg.bin`. Names inside are CRC-32 hashes (`0xXXXXXXXX`); `hash_name` in the RE database
reverses them (`nie seed-ui`), and a file name hash is the CRC-32 of the file name (an emblem id, a model id).

## Layers

| layer | crate | what |
|---|---|---|
| bytes to values | `nie-formats::cfgbin` | RDBN reader and typed value decoding; writers in `rdbn_patch`, `t2b_patch`, `cpk_encode` |
| values to named structs | `nie-data` | `no_std` + alloc, wasm32-compatible, one module per table family (`skill`, `chara_param`, `item`, `growth`, `formation`, `quest`, `shop`, `soccer_*`, `text`, ...); the list is `crates/nie-data/src/lib.rs` |
| dispatch | `nie_data::typed` | `family_key(path)` (file name without `.json`, `.cfg.bin` and the trailing `_<version>`: `formation_config_0.02.16.cfg.bin` gives `formation_config`) and `decode_by_key(key, tagged_json)` to the named struct as JSON; shared by `nie-model-serve` (`/typed`) and `nie-wasm` |
| SQL | `nie-cfgsql` | exact mapping: one RDBN list is a table, a row is a row, a field is a column, an invalid value is `NULL`; one schema renders to Postgres and SQLite |
| read-only SQL | `nie-sql` | SQLite mirror and PostgreSQL service contract, never a product schema |
| wiki | `nie-wiki-vfs` | every record joined and labeled from these tables |

Rule stated by `nie-data`: every struct, offset and golden value is anchored to a real table read from the VFS and nothing is
invented. Repository tests never carry game bytes: they build synthetic fixtures in a temp directory (AGENTS.md).

## Work flow

1. Find the file: `nie vfs find <name> --ext cfg.bin`, then `nie vfs cat <path>` for the structured preview.
2. Dump tagged JSON: `nie refresh-typed-json <dir>` writes `*.cfg.bin.json` next to each file in the form `nie-data`
   reads (`{entries}` or `{lists}`). `nie decode <file>` gives the raw generic structure instead, a different shape.
   `nie vfs extract <prefix> --ext cfg.bin -o <dir>` gets the files first; always filter by extension.
3. Typed read: `nie wiki tables`, `nie wiki table <table> <class>` (entries of one class as stored), or the typed
   route `/typed` of `nie-model-serve`.
4. Edit: `nie mod set|get|texture|recolor` (the `cfgbin`/`g4tx` encoders of `nie-formats`), then `nie mod validate`
   and `install`. A writer needs a round-trip test; task #39 tracks writers for every parsed format.
5. A new family: add `crates/nie-data/src/<family>.rs`, register it in `typed::decode_by_key` and in `lib.rs`, write the
   golden test, run `cargo test -p nie-data` and `just ci-nostd` (the crate must stay `no_std`) and
   `just ci-wasm-core` (wasm32).

## Languages

Nine text languages ship (`de en es fr it ja pt zh_hans zh_hant`). `nie wiki languages` lists what the installation
ships; `nie locales` inventories tags and localized resources. A text lookup that prints Japanese for another language
is a preview artifact of `nie vfs cat`: compare raw bytes first.
