# Design system: the game's own UI, read from the VFS

The design system of nie is the UI that ships inside Inazuma Eleven: Victory Road. Nothing is
designed beside it: colours, type, layouts, icons and copy come from `nie.exe`'s data VFS and are
exported to Figma, Markdown and JSON from there. This file is the rule set for that export and for
any agent using the Figma MCP on this repository.

## 1. Tokens

| token family | VFS source | decoder | served at |
|---|---|---|---|
| text colours (70 named entries, text + ruby RGB) | `data/common/font/font_color.cfg.bin` (`FONT_COLOR`) | `nie_formats::cfgbin::to_iecode_json` + `nie_data::font_color::parse_font_colors` (the menu compositor reads the same file with `nie_formats::menu_layout::parse_font_palette`) | `GET /ui/theme.json` (nie-model-serve) |
| glyph metrics | `data/common/font/font/font_def/font.cfg.bin` | `nie_formats::cfgbin::parse_t2b` + `nie_formats::font::parse_metrics` | `GET /ui/font-metrics.json` |
| font atlases | `data/dx11/font/font_def/font.g4tx`, `font_ja*`, `font_zh_hans*`, `font_zh_hant*`, per-language `data/dx11/font/<lang>/` | `nie_formats::g4tx` | `GET /tex/dx11/font/...png` |
| button glyphs (gaiji) per platform | `data/dx11/font/gaiji_{game,key,xinput,multi,nx,ps4,ps5,xbox,SteamDeck}.g4tx`, `gaiji_hlp_*` | `nie_formats::g4tx` | `GET /tex/dx11/font/gaiji_*.png` |
| canvas | 1920 x 1080 (menu layouts) | `nie_formats::menu_layout` | `GET /menu-render/<screen>.png` |

Rules:
- A token name is the game's own identifier (the `FONT_COLOR` entry, the font file stem). Never
  rename to a generic scale (`primary`, `gray-500`).
- Token values are read from the VFS at export time. `design/tokens.json` and `design/rg-m3.json`
  are a hand-made Rose Griffon brand set, not the game's; they are scheduled for removal and are
  never a source for nie.

## 2. Components: menu screens

The VFS mirrors every screen in two trees with the same names:

- `data/common/menu/<NN_name>/` holds the logic and layout (`cfg.bin`: `*_setting`,
  `*_menu_setting`, OBJBIN layers).
- `data/dx11/menu/<NN_name>/` holds the textures (G4TX sprite atlases).

Screen families (top level, read with `nie vfs ls data/common/menu`): `00_soccer`, `01_rpg`,
`02_btl`, `100_mainmenu`, `100_topmenu`, `101_member`, `102_team`, `103_item`, `104_bookmark`,
`105_datafile`, `106_info`, `107_vs`, `108_option`, `109_system`, `10_win`, `110_playguide`,
`112_gallery`, `113_activity`, `115_calendar`, `11_loading`, `121_inacord`, `121_quest`,
`122_opponent`, `125_multi`, `13_map`, `150_list`, `150_shop`, `160_town`, `161_avatar`,
`163_help`, `20_cmn`, `30_vsroute`, `31_universe`, `50_title`, `51_save`, `55_information`,
`600_event`, `60_story`. `data/dx11/menu` adds `200_icon`, `210_minimap` and `220_img`.

A Figma component maps 1:1 to a screen layer: component name = `<NN_name>/<screen>/<layer>`,
layer identity = `CRC32(name)` as in the game (`/menu-tree/<screen>.json` reports
`consistent: true` when every hash matches). `20_cmn` is the shared library (windows, buttons,
frames); `10_win` the dialog shells.

Code:
- `crates/nie-formats/src/menu_layout.rs`: composes a screen from its layers (`MenuAssets`,
  `ComposeReport`).
- `crates/nie-formats/src/menu_screen.rs`: `ScreenSpec`, `ScreenItem`, `build`.
- `crates/nie-formats/src/sprite_sheet.rs`: G4TX atlas to named sprites (`depuis_g4tx`).
- `crates/nie-data` `menu_setting`: the `*_menu_setting` tables (layers, resources, commands).
- `crates/nie-lua` `menu_host`: runs the menu Lua (`data/common/script/lua/`, Lua 5.2 bytecode)
  that drives screen behaviour.

## 3. Frameworks

- Decoding and export: Rust (`nie-formats`, `nie-data`, `nie-explore`), no UI framework.
- Rendering: Bevy (game world, `nie-bevy`) and NSH for tools, see `docs/adr-ui-stack.md`.
- Serving: `nie serve` and `nie-model-serve` (`docs/serve.md`), images on ghcr.
- `crates/nie-ui-tree`: a generic UI tree (`UiNode`, `Theme`, `render_html`) used by azalee-app;
  it is not the game's UI and does not feed the Figma export.

## 4. Assets

- Assets are never stored in the repository. They are read in place from the installation
  (`NIE_GAME_SOURCE`: local path, `ssh://`, `r2://`, `gdrive://`, `https://`) and decoded on
  request.
- Textures: `GET /tex/<vfs path>.png` (largest texture of a G4TX) and
  `GET /tex/<container>.g4tx/<name>.png` (a named texture); metadata `GET /tex-info/<path>.g4tx`.
- CDN: `cdn.rosegriffon.fr/dx11/**` is served by nie-model-serve `/tex/dx11/**`.
- Figma import: upload the PNG decoded from `/tex`, keep the VFS path as the layer description.

## 5. Icons

- Source: `data/dx11/menu/200_icon/` atlases and per-screen atlases under `220_img/`.
- Index: `nie icons index` (`nie_explore::menu_icons::build_icon_index`) writes name -> atlas +
  rectangle; `GET /icons/index.json` serves the written file.
- Naming: the game's sprite name inside the atlas (e.g. `load_ball01`), never a new name.

## 6. Styling and localisation

- Colours come only from `FONT_COLOR`; sizes and positions only from the layout `cfg.bin`.
- Nine languages ship: `de, en, es, fr, it, ja, pt, zh_hans, zh_hant` (BCP 47 `zh-Hans`,
  `zh-Hant`). Text: `data/common/text/<lang>/`; localized textures:
  `data/dx11/menu/**/<lang>/` (e.g. `220_img/telop_waza/<lang>/`); fonts: `data/dx11/font/<lang>/`.
- In Figma, each language is a variable mode; text layers bind to the string id, never to pasted
  copy.

## 7. Project structure for the export

`VFS -> nie decoder -> JSON/PNG (nie serve, nie-model-serve, nie CLI) -> Figma / Markdown / JSON`.
Every exported item carries its provenance: VFS path, record (cfg.bin entry or sprite name) and
the decoder used.

## Gaps (unverified or not yet from the VFS)

- `crates/nie-formats/src/menu_scenes/*.json` are hand-placed layouts carried over from an earlier
  TypeScript app (`OpeningVisual.tsx`); they must be replaced by layouts decoded from
  `data/common/menu/**` cfg.bin.
- `/icons/index.json` reads a prebuilt file (`NIE_ICONS_INDEX`, default `var/icons-index.json`),
  `/menu-render` a layout directory (`--layout-dir`) and `/menu-tree` a directory of
  `*_menu_setting.cfg.bin.json` dumps (`--menu-cfg-dir`); all three must be computed from the VFS
  at request time.
- `nie vfs ls data/common/font` reports fonts as "not exploited" and `data/common/menu` as
  "partial": coverage comes from `nie vfs formats`, not from this file.
