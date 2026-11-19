# ADR: two UI stacks, Bevy for the game, NSH for tools

Status: accepted, 2026-11-19. Supersedes the four-stack state described below.

## Context

nie ran four UI stacks. After the football layer and the game client were purged
(nie-game, nie-football, nie-render, nie-sim, nie-studio and the rest), what remains is:

| stack | crates |
|---|---|
| Bevy | nie-render3d, nie-camera, nie-bevy, nie-wasm |
| Dioxus 0.8 alpha | retired (nie-web and nie-ui-dioxus are deleted) |
| egui / eframe | retired (nie-studio is deleted) |
| GPUI shell contract | nie-shell (contract only, no `gpui` dependency) |

Each stack brings its own renderer path, its own widget set and its own dependency tree. Dioxus is
the only stack still on an alpha release.

## Decision

1. **Bevy owns rendering and assets**: assets over the VFS (`game://`), the 3D viewport, and the
   web build through wasm32 and WebGPU. Bevy comes from crates.io releases (0.20.1 today).
2. **NSH (the GPUI native shell) owns every out-of-game tool**: studio, launcher, editors
   (cfg.bin, Lua, textures, models), the zukan and wiki viewers, and the desktop, mobile and
   headless app surfaces. `nie-shell` becomes a real NSH app crate. A viewport embeds the Bevy
   render by sharing its wgpu texture.
3. **Dioxus is retired.** The web product is the rosegriffon Next.js site, which calls `nie serve`
   over HTTP (see `docs/serve.md`).
4. **egui is retired**: nie-studio is deleted and `eframe` is gone.

## Consequences

- Two UI stacks, each with one renderer path (wgpu): Bevy for rendering on desktop and web; NSH
  for tools.
- The Dioxus removal also drops `dioxus-desktop`, which today pulls the full `image` codec set
  (AVIF, EXR and others) into the lockfile.
- azalee-app loses its nie-ui-dioxus dependency and moves to NSH or to the rosegriffon site.
- Remaining work: azalee-app moves to NSH or to the rosegriffon site.
