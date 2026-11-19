# nie

A self-hosted Inazuma Eleven engine and studio, plus the football craft layer, in Rust on the Bevy
ecosystem. All the Rust of the Rose Griffon ecosystem lives here.

nie loads game data directly from the installation the player owns (VFS layers, CRI CPK archives
and the other containers of the game) and never ships, copies or commits a game asset. The
football layer is separate: original games built as Craft Anything jobs, one TOML manifest in
`games/` selecting shared engine crates and a frontend, with all content authored or generated
in this repository.

## Layout

```
crates/
  nie-formats nie-data nie-core nie-index nie-lua ...   game formats, data, VFS and CPK readers
  nie-assets    Bevy asset source `game://` over the VFS: local path, SSH, rclone (R2, Google Drive)
  nie-bevy      G4TX and G4MD/G4MG as Bevy assets
  nie-render3d nie-gpu nie-runtime nie-game nie-ui*   renderers and the wgpu host
  nie-cli nie-model-serve nie-explore nie-launcher    command line, services and mod tooling
  nie-shell     app and launcher contract for the aphrody native GPUI shell
  nie-re rg-steam nie-re-core   reverse engineering and Steam libraries (from rosegriffon)
  achillea-* azalee-app rg-ffi*   site, bot and FFI crates of the ecosystem
  nie-model nie-physics nie-input nie-ai nie-lockstep nie-sim   football simulation
  nie-render nie-football nie-studio nie-craft              football frontends and craft tool
tools/steam-index
games/          craft manifests, one TOML file per football game
contract/       nie-app.json and aphrody-store.toml (generated), openapi.json
design/         design tokens
docs/           bevy-ecosystem.toml: Bevy crates and their use in nie
iecode/         git submodules (private): aphrody-labs/iecode-dotnet and aphrody-labs/iecode-cpp
```

## Bevy base

Bevy 0.20.1 and every other dependency come from crates.io releases, declared once in
`[workspace.dependencies]`; nothing is forked or patched. `docs/bevy-ecosystem.toml` maps every
Bevy crate to its use in nie, with versions read from crates.io.

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
  game source settings, Bevy Remote port); `-- store` prints the catalog entry the shell merges
  from `$XDG_CONFIG_HOME/aphrody/awesome_gpui_store.toml`.

## Football layer

The simulation (`nie-sim`) depends only on `bevy_app`, `bevy_ecs` and `bevy_math`. It has no
window, no renderer and no wall-clock dependency: one run of the `SimTick` schedule is one
tick. The same plugin runs headless in CI, inside the Bevy game on a fixed timestep, inside the
Bevy game compiled to WebAssembly, and in lockstep netplay, where peers exchange only inputs
and compare a world checksum every tick.

Determinism rules: integer-only random stream (`DetRng`), only `+ - * /` and `sqrt` on floats,
entities processed in `(side, number)` order, no trigonometry in the simulation.

## Craft manifest

A game is one TOML file, validated against a typed schema (`deny_unknown_fields`):

```toml
schema = 1

[game]
id = "pocket-derby"        # lowercase letters, digits, dashes
name = "Pocket Derby"
version = "0.1.0"
mode = "exhibition"        # exhibition | series ([series] matches = N)
frontend = "bevy3d"        # bevy3d | studio | bevy_web

[rules]                    # every field optional
players_per_side = 5       # goalkeeper included, 2 to 11
tick_rate = 60
match_seconds = 60
pitch_length = 40.0
pitch_width = 24.0
goal_width = 6.0
seed = 7

[art]
style = "lowpoly"          # lowpoly | flat | toon
camera = "broadcast"       # broadcast | top_down
pitch_color = [46, 125, 62]

[teams.home]
formation = "1-2-1"        # outfield lines, defence to attack
controller = "human"       # human | ai
# name = "..."  seed = N   # optional overrides of the generated team

[teams.away]
formation = "2-1-1"
```

The `frontend` field resolves to a cargo target:

| frontend         | package      | how it starts                         |
|------------------|--------------|---------------------------------------|
| `bevy3d`         | `nie-football` | `cargo run -p nie-football -- --manifest` |
| `studio`         | `nie-studio` | `cargo run -p nie-studio -- --manifest` |
| `bevy_web`       | `nie-football` | `trunk serve crates/nie-football/index.html` (wasm32 and WebGPU, plays `games/web-cup.toml`) |

## The craft tool

```
cargo run -p nie-craft -- check  games/pocket-derby.toml   # parse and validate
cargo run -p nie-craft -- plan   games/pocket-derby.toml   # resolved build and launch commands (JSON)
cargo run -p nie-craft -- run    games/pocket-derby.toml --ticks 1200 --expect 1-2
cargo run -p nie-craft -- build  games/pocket-derby.toml   # build the frontend
cargo run -p nie-craft -- launch games/pocket-derby.toml   # build and start the frontend
```

`run` plays the manifest headless with the AI on both sides and prints a JSON report per match
(score, goals with their tick, world checksum). `--expect HOME-AWAY` turns it into an assertion.

## Vertical slice

A 5v5 match (goalkeeper plus four outfield players per side) on a 40 x 24 m pitch:

- `cargo test -p nie-sim` plays a full 3600-tick match headless and asserts the exact score and
  the world checksum, replays it twice, and runs two lockstep peers for 900 ticks without a desync.
- `craft run` over the manifests runs in CI with an exact expected score.
- `cargo check` covers the Bevy 3D game and the egui studio, plus the Bevy game with the `webgpu`
  feature for `wasm32-unknown-unknown`.

Not in the slice yet: throw-ins, corners, fouls, player collisions, a networked transport (only
the lockstep core and an in-process pipe exist), audio, and authored or generated art assets
beyond primitive meshes.

## Versions

Bevy 0.20.1 and everything else come from crates.io. Rust 1.100.0 (`rust-toolchain.toml`).

## Build

Linux needs the usual Bevy and winit libraries (ALSA, udev, Wayland, xkbcommon, X11, GL). The
engine crates and `craft` build with nothing but a Rust toolchain.

```
cargo test -p nie-model -p nie-physics -p nie-input -p nie-ai -p nie-lockstep -p nie-sim -p nie-craft
cargo run -p nie-football -- --manifest games/pocket-derby.toml
```

Heavy builds belong on a build host or in CI, not on a workstation.

## License

MIT OR Apache-2.0, see `LICENSE` and `LICENSE-APACHE`.
