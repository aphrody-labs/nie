# nie

An ecosystem of football (soccer) video games built with Rust, Bevy, a native GUI and Dioxus.

Games are not one-off projects. Each one is a job in the Craft Anything sense: a typed manifest
that selects reusable engine crates, rules, art and a frontend, and tooling that turns the
manifest into a playable target. The engine is shared; a new game is a new `games/*.toml`.

All content is original. Teams, players, names, colors and art are generated or authored in this
repository. No existing commercial game, character, music or level is reproduced.

## Layout

```
crates/
  nie-model     data model: teams, players, formations, rules, deterministic generator
  nie-physics   fixed-step ball and player physics (IEEE-exact operations only)
  nie-input     device-independent input command, 5 bytes per player and tick
  nie-ai        role-based AI that emits the same commands as a human
  nie-net       lockstep netcode: packets, input delay buffer, desync detection
  nie-sim       match simulation as a Bevy ECS plugin, runnable headless
  nie-render    Bevy 3D rendering: pitch, goals, players, ball, camera, scoreboard
  nie-craft     manifest, validation, plan, and the `craft` command line tool
  nie-game      frontend: native 3D game (Bevy)
  nie-studio    frontend: native GUI manager and editor (egui)
  nie-web       frontend: Dioxus app, web (WebAssembly) or desktop
games/          craft manifests, one TOML file per game
```

The simulation (`nie-sim`) depends only on `bevy_app`, `bevy_ecs` and `bevy_math`. It has no
window, no renderer and no wall-clock dependency: one run of the `SimTick` schedule is one
tick. The same plugin runs headless in CI, inside the Bevy game on a fixed timestep, inside the
Dioxus page compiled to WebAssembly, and in lockstep netplay, where peers exchange only inputs
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
frontend = "bevy3d"        # bevy3d | studio | dioxus_web | dioxus_desktop

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
| `bevy3d`         | `nie-game`   | `cargo run -p nie-game -- --manifest` |
| `studio`         | `nie-studio` | `cargo run -p nie-studio -- --manifest` |
| `dioxus_desktop` | `nie-web`    | `cargo run -p nie-web --no-default-features --features desktop` |
| `dioxus_web`     | `nie-web`    | `dx serve --package nie-web --platform web` |

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
- `cargo check` covers the Bevy 3D game, the egui studio and the Dioxus app, plus the Dioxus app
  for `wasm32-unknown-unknown`.

Not in the slice yet: throw-ins, corners, fouls, player collisions, a networked transport (only
the lockstep core and an in-process pipe exist), audio, and authored or generated art assets
beyond primitive meshes.

## Versions

Dependencies come from crates.io at the versions checked on 2026-11-16: Bevy 0.20.1, Dioxus
0.7.10 (0.8 is alpha), eframe/egui 0.37.0. Rust 1.100.0 (`rust-toolchain.toml`).

## Build

Linux needs the usual Bevy and winit libraries (ALSA, udev, Wayland, xkbcommon, X11, GL). The
engine crates and `craft` build with nothing but a Rust toolchain.

```
cargo test -p nie-model -p nie-physics -p nie-input -p nie-ai -p nie-net -p nie-sim -p nie-craft
cargo run -p nie-game -- --manifest games/pocket-derby.toml
```

Heavy builds belong on a build host or in CI, not on a workstation.

## License

MIT OR Apache-2.0, see `LICENSE` and `LICENSE-APACHE`.
