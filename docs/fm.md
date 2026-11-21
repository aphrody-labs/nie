# nie fm: the terminal explorer of the game VFS

`nie fm` browses the VFS of the installed game in a terminal: a yazi-style three-column manager
(parent, current, preview) over `nie-assets`, so `NIE_GAME_SOURCE`, `ssh://`, `rclone://` and local
paths work unchanged. It never writes into the installed game files; it has no editing mode yet (edits go to an overlay or an output path, `AGENTS.md`).

```sh
NIE_GAME_SOURCE="ssh://<game-host>/<game_dir>" nie fm
nie fm --cd data/common/menu                   # open a directory first
nie fm --select data/dx11/menu/x.g4tx         # put the cursor on a file
nie fm --snapshot 120x40 --select <path>      # one frame as text, no terminal (CI, logs)
nie fm --protocol sixel                       # kitty | sixel | iterm2 | halfblocks
nie fm --metrics timings.json                 # per-preview latency, for the research log
```

## Layout

The code is split so that the view model is shared with other front ends (the GPUI twin, the GPU
terminal of `nie term`):

| Layer | Crate | Contents |
|---|---|---|
| View model | `nie-explore` (`fm` feature, module `fm`) | `VfsTree`, `Keymap`, `Fm` state machine, nucleo search, globset filter, previewers, BLAKE3 thumbnail cache |
| Renderer | `plugins/tui/nie-tui` | ratatui drawing, ratatui-image pictures, tokio preview scheduler, event loop |
| Command | `nie-cli` (`fm_cmd.rs`) | arguments only |

`nie_explore::fm::preview::build` is synchronous and polls a cancel callback between its stages; the
tokio scheduler in `nie-tui` (`sched.rs`) debounces requests (40 ms), keeps one job in flight and
cancels the stale one.

## Keys

The default keymap is `crates/nie-explore/src/fm/keymap.toml`, in yazi's `keymap.toml` notation
(`<C-u>`, `<Enter>`, sequences such as `gg` and `,s`). A user file (`--keymap`, `NIE_FM_KEYMAP`, or
`$XDG_CONFIG_HOME/nie/fm-keymap.toml`) is merged on top: a binding with the same keys replaces the
default. Press `?` for the list.

| Keys | Action |
|---|---|
| `j` `k` `h` `l`, arrows | move, leave, enter |
| `gg` `G` `<C-u>` `<C-d>` `<C-f>` `<C-b>` | top, bottom, half page, page |
| `/` | fuzzy search over every path (nucleo); `<Enter>` reveals the hit |
| `f` | glob filter of the directory (`*.g4tx, !*_l*`) |
| `<Tab>` | group by path or by CPK archive |
| `,n` `,s` `,e` `,r` | sort by name, size, extension; reverse |
| `<Space>` `<C-a>` | mark |
| `]` `[` | next or previous frame, sprite, turntable angle |
| `}` `{` | next or previous texture, record or language |
| `p` | invert autoplay |
| `x` | hex view |
| `J` `K` | scroll the preview |

## Previewers

A file is classified by its magic bytes first (`nie_formats::detect`), by its name second.

| Kind | Preview |
|---|---|
| G4TX texture | the decoded picture; an atlas shows its sprites one by one; planche measurements in the header |
| DDS, PNG | the picture |
| cfg.bin (RDBN, T2B) | records as columns; `}` steps through records; localized text (`data/common/text/<locale>/...`) steps through the languages the VFS has |
| Event script (`event_cfg`) | commands as Lua call syntax, highlighted |
| Lua bytecode (5.2) | disassembly as Lua call syntax, highlighted |
| Text | UTF-8 text |
| USM film | metadata card; H.264 films also show decoded frames (OpenH264) |
| G4MD model | turntable rendered by the CPU rasteriser of `nie-render3d` |
| anything else | hex |

Highlighting uses `tree-sitter-highlight` with the tree-sitter Lua grammar (one tree-sitter runtime for the whole binary). The game ships
no Lua source and no event script text, so both are rendered as Lua call syntax first.

Pictures are drawn with `ratatui-image`: kitty, sixel and iTerm2 graphics when the terminal answers
the capability query, half blocks otherwise. Films and turntables animate; `[` and `]` step by hand.

## Thumbnail cache

Decoded pictures are cached under the BLAKE3 hash of the logical path, the archive, the size, the
picture index and the decoder version (`DECODER_VERSION` in `fm/thumbs.rs`), in
`$NIE_FM_CACHE_DIR` or the user cache directory (`nie/fm`). A location inside the game directory is
refused. The budget is `NIE_FM_CACHE_MIB` (256 by default); the oldest entries go first. `--no-cache`
turns it off.

## Designs taken, and from where

Nothing from yazi, helix, spacedrive or kitty is copied. Each design is cited in
`docs/specs/terminal-sources.policy.toml`; the third-party crates come from crates.io and are declared once in
`[workspace.dependencies]`.

## Limits

- Audio of films is not played: the explorer lists the tracks and their codecs.
- MPEG-2 and VP9 films get the metadata card only; the build has no decoder for them.
- WebP pictures are not decoded.
- Model previews are CPU-rendered and untextured.
