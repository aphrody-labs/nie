# nie

A self-hosted Inazuma Eleven engine and studio in Rust on the Bevy ecosystem. All the Rust of the
Rose Griffon ecosystem lives here.

nie loads game data directly from the installation the player owns (VFS layers, CRI CPK archives
and the other containers of the game) and never ships, copies or commits a game asset.

## Layout

```
crates/
  nie-formats nie-data nie-explore nie-index nie-lua ...   game formats, data, VFS and CPK readers
  nie-assets    Bevy asset source `game://` over the VFS: local path, SSH, rclone (R2, Google Drive)
  nie-bevy      G4TX and G4MD/G4MG as Bevy assets
  nie-render3d nie-gpu nie-camera   renderers and the wgpu host
  nie-cli nie-model-serve nie-explore nie-launcher    command line, services and mod tooling
  nie-shell     app and launcher contract for the aphrody native GPUI shell
  nie-re nie-steam nie-re-core   reverse engineering and Steam libraries (from rosegriffon)
  rg-ffi*   generated C ABI
tools/steam-index
contract/       nie-app.json and aphrody-store.toml (generated), openapi.json
design/         design tokens
docs/           service and design documents
iecode/         git submodules (private): aphrody-labs/iecode-dotnet and aphrody-labs/iecode-cpp
```

## Bevy base

Bevy 0.20.1 and every other dependency come from crates.io releases, declared once in
`[workspace.dependencies]`; nothing is forked or patched.

The iecode engines are private submodules marked `update = none`, so a plain clone and CI skip
them. With access to aphrody-labs:

```
git submodule update --init --checkout iecode/dotnet iecode/cpp
```

## Game assets

`nie-assets` registers the asset source `game://`. Point it at the installation with
`NIE_GAME_SOURCE` (or a TOML file named by `NIE_ASSETS_CONFIG`):

| source                          | transport                                             |
|---------------------------------|-------------------------------------------------------|
| `D:\Games\ievr`, `/srv/game`    | local files, CPK ranges read in place                 |
| `ssh://host/path`, `sftp://...` | `ssh` with your SSH config, range reads with `dd`     |
| `r2://remote/bucket/path`       | rclone remote, `rclone cat --offset --count`          |
| `gdrive://remote/path`, `rclone://remote/path` | rclone remote (Google Drive and others)   |
| `https://host/path`             | HTTP range requests                                   |

Only the CPK tables and the requested ranges are transferred. `NIE_ASSETS_CACHE_DIR` adds a
size-bounded read-through cache outside the repository. Credentials stay in `~/.ssh` and
`rclone.conf`, never in a URI. The synthetic-CPK test (`cargo test -p nie-assets`) builds its own
archive in a temp directory; `NIE_ASSETS_PRD_SOURCE=ssh://prd/srv/rosegriffon/game/ievr cargo test -p nie-assets --test prd_stream`
lists and reads a CPK of the real installation over SSH.

## Targets

- Native: Windows, Linux and macOS (CI matrix).
- Web: `wasm32-unknown-unknown` with WebGPU (`bevy/webgpu`).
- aphrody shell: `cargo run -p nie-shell -- contract` prints `contract/nie-app.json` (commands,
  game source settings, build targets); `-- store` prints the catalog entry the shell merges
  from `$XDG_CONFIG_HOME/aphrody/awesome_gpui_store.toml`.

## Versions

Bevy 0.20.1 and everything else come from crates.io. Rust 1.100.0 (`rust-toolchain.toml`).

## Build

Linux needs the usual Bevy and winit libraries (ALSA, udev, Wayland, xkbcommon, X11, GL).

Heavy builds belong on a build host or in CI, not on a workstation.

## License

MIT OR Apache-2.0, see `LICENSE` and `LICENSE-APACHE`.
