# ADR: Bevy owns game rendering and assets

Status: accepted, 2026-11-19; amended by `docs/adr-gpui.md` (every other UI is GPUI). Supersedes the four-stack state described below.

## Context

nie ran four UI stacks. After the football layer and the game client were purged
(nie-game, nie-football, nie-render, nie-sim, nie-studio and the rest), what remains is:

| stack | crates |
|---|---|
| Bevy | nie-render3d, nie-camera, nie-bevy, nie-wasm |
| Dioxus 0.8 alpha | retired (nie-web and nie-ui-dioxus are deleted) |
| egui / eframe | retired (nie-studio is deleted) |
| app contract | nie-shell (contract only, no UI dependency) |

Each stack brings its own renderer path, its own widget set and its own dependency tree. Dioxus is
the only stack still on an alpha release.

## Decision

1. **Bevy owns rendering and assets**: assets over the VFS (`game://`), the 3D viewport, and the
   web build through wasm32 and WebGPU. Bevy comes from crates.io releases (0.20.1 today).
2. **Tools outside the game are consumers, not part of nie.** A native shell reads
   `contract/nie-app.json` (`nie-shell`) and talks to `nie serve` over HTTP or to the C ABI
   libraries (`docs/crlib.md`); nie names no shell and depends on none.
3. **Dioxus is retired.** A web front end calls `nie serve` over HTTP (see `docs/serve.md`).
4. **egui is retired**: nie-studio is deleted and `eframe` is gone.

## Consequences

- One renderer path (wgpu): Bevy renders the game on desktop and web; the tool UI is GPUI (`docs/adr-gpui.md`).
- The Dioxus removal also drops `dioxus-desktop`, which today pulls the full `image` codec set
  (AVIF, EXR and others) into the lockfile.
- The old web app crate is deleted; a web front end is an HTTP consumer.
