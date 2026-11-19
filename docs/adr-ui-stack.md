# ADR: two UI stacks, Bevy for the game, NSH for tools

Status: accepted, 2026-11-19. Supersedes the four-stack state described below.

## Context

nie runs four UI stacks today:

| stack | crates |
|---|---|
| Bevy | nie-game, nie-football, nie-render, nie-render3d, nie-camera, nie-bevy, nie-wasm; nie-sim on `bevy_ecs` |
| Dioxus 0.8 alpha | nie-web, nie-ui-dioxus; nie-ui-bevy bridges into it; azalee-app depends on nie-ui-dioxus |
| egui / eframe | nie-studio |
| GPUI shell contract | nie-shell (contract only, no `gpui` dependency) |

Each stack brings its own renderer path, its own widget set and its own dependency tree. Dioxus is
the only stack still on an alpha release.

## Decision

1. **Bevy owns everything that runs in the game world**: simulation, rendering, assets over the
   VFS (`game://`), in-game menus and HUD (`bevy_ui` and `bevy_feathers`), headless CI, and the
   web build through wasm32 and WebGPU. `nie-sim` stays on `bevy_app`, `bevy_ecs` and `bevy_math`
   only. Bevy comes from crates.io releases (0.20.1 today).
2. **NSH (the GPUI native shell) owns every out-of-game tool**: studio, launcher, editors
   (cfg.bin, Lua, textures, models), the zukan and wiki viewers, and the desktop, mobile and
   headless app surfaces. `nie-shell` becomes a real NSH app crate. A viewport embeds the Bevy
   render by sharing its wgpu texture.
3. **Dioxus is retired.** The web product is the rosegriffon Next.js site, which calls `nie serve`
   over HTTP (see `docs/serve.md`). The in-browser game is the Bevy wasm build. Delete nie-web,
   nie-ui-dioxus, the Dioxus bridge of nie-ui-bevy and the `dioxus_web` and `dioxus_desktop`
   frontends of nie-craft; move `games/web-cup.toml` to the Bevy wasm frontend.
4. **egui is retired** once nie-studio runs on NSH; then drop `eframe`.

## Consequences

- Two UI stacks, each with one renderer path (wgpu): Bevy for the game on desktop, web and
  mobile; NSH for tools.
- The Dioxus removal also drops `dioxus-desktop`, which today pulls the full `image` codec set
  (AVIF, EXR and others) into the lockfile.
- azalee-app loses its nie-ui-dioxus dependency and moves to NSH or to the rosegriffon site.
- Order of work: retire Dioxus first (nie-craft frontend, web-cup manifest, azalee-app), then port
  nie-studio to NSH, then remove egui.
