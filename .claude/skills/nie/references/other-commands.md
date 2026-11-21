# Every other command, by area

Names, features, MCP tools and owners of all commands are in [commands.md](commands.md) (generated). This page says
what the commands not covered by another reference do. Every command has `--help`; that output wins over this page.

The MCP tool of command `X` is `cli_X` with `-` replaced by `_`, taking `{"args": [...]}` (the tail after `X`). The
`oc` command has two tools, `cli_oc` and `cli_ocgen`.

## Framework commands (core, always built)

| command | use |
|---|---|
| `nie new <app> --template cli\|serve\|lib\|ui\|wasm [--path DIR] [--framework DIR]` | scaffold `apps/<app>` (or an app anywhere) with its `nie.toml`; `NIE_FRAMEWORK` names the checkout to depend on |
| `nie bindgen [--contract abi.json] [--out DIR] [--lang a,b] [--check]` | generate C, C++, Rust, Python, C#, Go, Bun and Zig bindings from a C ABI contract; `--check` fails on drift (`just bindgen`, gate `just ci-bindings`) |
| `nie deps upstream ...` / `nie deps audit [--check]` | dependency decisions read live from GitHub and crates.io / measure `nie.exe` imports, external process calls and `Cargo.lock`, write the research log entry |
| `nie serve [--strict] [--stateless]` | HTTP service: VFS, formats, game-data, wiki (JSON `/v1/wiki/...`, HTML `/wiki`), `/openapi.json`, `/health` (JSON with `vfsReady`), `/ready`; default `127.0.0.1:8790`, `NIE_HOST`, `NIE_PORT`; `docs/serve.md` |
| `nie plugins [--json]` | plugins compiled into this build and the commands each owns |
| `nie mcp` | the native MCP server on stdio (`.mcp.json` starts it through `tools/nie-mcp-launch`) |
| `nie info`, `nie locales`, `nie format`, `nie decode`, `nie convert`, `nie refresh-typed-json` | [vfs.md](vfs.md) |
| `nie wiki ...` | [knowledge-docs.md](knowledge-docs.md) |
| `nie save`, `nie steam` | [nie-exe.md](nie-exe.md) |

`nie-model-serve` (own binary) is the asset origin for a CDN: decoded textures (PNG), audio (WAV), video (MP4), cfg as
JSON and typed, models (GLB), raw bytes behind a token (`NIE_RAW_VFS_TOKEN`, at least 32 bytes). `nie-deploy` ships and
supervises the services: `nie-deploy up --host prd --bin-dir <dir>`, `nie-deploy status --host prd --json`; the host and
directories come from `deploy/supervise.toml`, never from code. Production (`prd`) is touched only through that tool.

## Files and search (host build)

| command | use |
|---|---|
| `nie find [pattern] [-C dir] [-g glob] [-e ext] [-t f\|d] [-H] [-I] [--depth N] [-n limit] [-s] [-c]` | find files on disk by path (the `ignore` engine of ripgrep/fd); inside CPK use `nie vfs find` |
| `nie grep <regex> [-C dir] [-g glob] [-e ext] [-i] [-H] [-I] [-n limit] [-l]` | search file contents (ripgrep engine) |
| `nie img info\|resize\|recolor\|crop\|convert\|composite\|planche\|diff` | native image tools; `diff` compares a render to a reference capture: identity, DeltaE2000, SSIM per region, a difference map |
| `nie render glb-png\|glb-gif` | render a GLB to PNG captures or a turntable GIF (`nie-render3d`, deterministic CPU rasterizer, optional wgpu) for visual QA |

## Game content catalogues (host build, VFS read in place)

| command | use |
|---|---|
| `nie mode index\|coverage\|export\|contenu` | catalogue of game modes: screens, layers, objects, assets and scripts per mode; `index` writes the database |
| `nie icons index\|dict\|extract` | icon atlases: name to atlas and rectangle, decode to PNG |
| `nie avatar catalog\|parts\|preset\|export\|roi\|planches\|depliage\|icons` | avatar editor (`chara_edit`): catalogue, parts, preset recipes, resolved export, regions, sheets, UVs |
| `nie oc ...` (alias `ocgen`) | generate an original 3D character: morphology, `chara_edit` parts, measured colours (`nie-ocgen`, also its own binary) |
| `nie textures [--limit N] [--manifest F]` | NDJSON manifest of `.g4tx` headers (bounded, default 500; `--redis` mirrors to Redis db 3) |
| `nie uniform-map [--out F]` | NDJSON CRC-32 to path manifest of every `.g4md`/`.g4mg`, resolves uniform model ids |
| `nie menu-predecode` | pre-decode menu sprites into a disk dump (idempotent); needs the Redis index |
| `nie vn casting\|export` | feed the visual-novel engine from real voices, textures and music; local catalogue, never committed |
| `nie video info\|liste\|export\|catalogue` | USM/Sofdec2 cutscenes: inventory, metadata, remux to MP4 without re-encoding, catalogue |

Everything these write goes to a path you give, outside the repository; never commit it.

## Terminal and desktop (plugin `tui`)

`nie fm` is the three-column VFS explorer (no editing mode yet) (parent, current, preview; `--cd`, `--select`, `--snapshot WxH`,
`--protocol kitty|sixel|iterm2|halfblocks`, `--metrics f.json`), `docs/fm.md`. `nie term` (feature `term`) is the GPU terminal
that hosts `nie fm` or any command after `--`; `--screenshot <png>` renders one frame offscreen, `docs/term.md`. The
`shell-desktop` plugin owns no CLI command: it is the model of the desktop shell, the programs are `nie-shell` and
`nie-comp` (GPUI toolkit under `gui/`, its own workspace, built and tested on the build host against the sibling `../aphrody` checkout (`just _gpui`)).

## Release, CI and ops (`just`, not `nie`)

`just` lists recipes. `check lint fmt test compile repro map skill bindgen app-pack libernt-closure fr1 web-pack web-serve web-shot deploy`
are developer entrypoints; `ci-*` recipes are the jobs of `.github/workflows`, one each. `just fr1
<recipe>` runs any recipe on the build host in a detached tmux session. Contract and ABI regeneration:
`cargo run -p nie-shell -- contract > contract/nie-app.json`, `cargo run -p nie-abi -- crates/nie-ffi`,
`cargo run -p nie-abi -- --crlib .`; each has a drift test that fails when the committed files differ.
After a push check `gh run list -R aphrody-labs/nie`; run `gitleaks detect` before every push.
