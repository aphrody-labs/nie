---
name: bevy
description: "Use when working with the `bevy` crate of bevy: A refreshingly simple data-driven game engine and app framework."
---

# bevy

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/` (manifest `third_party/bevy/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Orientation

Entry point of the engine: the `bevy` crate re-exports `bevy_internal`, which wires the official `bevy_*` crates behind Cargo features (`third_party/bevy/Cargo.toml`, `[features]`; profiles `default`, `2d`, `3d`, `ui`, and the Rose Griffon `rg_web` set for WASM and WebGPU UI builds without 3d or audio).

Reading order at the vendored commit: `third_party/bevy/docs/cargo_features.md` (feature catalogue), `docs/profiling.md`, `docs/debugging.md`, `docs/linux_dependencies.md`, then `examples/` (by topic: `examples/ecs`, `examples/ui`, `examples/2d`, `examples/window`). The ECS lives in `crates/bevy_ecs`, scheduling in `crates/bevy_app`, rendering in `crates/bevy_render`, UI layout in `crates/bevy_ui` (Taffy).

Monorepo rules: the workspace pins `=0.20.0-dev` through `[workspace.dependencies]` and patches every `bevy_*` crate to `third_party/bevy`; add a Bevy crate by adding it to `[workspace.dependencies]` with the same pin, never with a registry version. The fork carries `[profile.release]` fat LTO with `codegen-units = 1` and a smaller `wasm-release`; the consumer workspace profiles (`wasm-release`, `web-craft` in the root `Cargo.toml`) are the ones that apply to our binaries.

## Purpose

A refreshingly simple data-driven game engine and app framework

## README (`third_party/bevy/README.md`)

# [![Bevy](https://bevy.org/assets/bevy_logo_light_dark_and_dimmed.svg)](https://bevy.org)


## What is Bevy?

Bevy is a refreshingly simple data-driven game engine built in Rust. It is free and open-source forever!

## WARNING

Bevy is still in the early stages of development. Important features are missing. Documentation is sparse. A new version of Bevy containing breaking changes to the API is released [approximately once every 3 months](https://bevy.org/news/bevy-0-6/#the-train-release-schedule). We provide [migration guides](https://bevy.org/learn/migration-guides/), but we can't guarantee migrations will always be easy. Use only if you are willing to work in this environment.

**MSRV:** Bevy relies heavily on improvements in the Rust language and compiler.
As a result, the Minimum Supported Rust Version (MSRV) is generally close to "the latest stable release" of Rust.

## Design Goals

* **Capable**: Offer a complete 2D and 3D feature set
* **Simple**: Easy for newbies to pick up, but infinitely flexible for power users
* **Data Focused**: Data-oriented architecture using the Entity Component System paradigm
* **Modular**: Use only what you need. Replace what you don't like
* **Fast**: App logic should run quickly, and when possible, in parallel
* **Productive**: Changes should compile quickly ... waiting isn't fun

## About

* **[Features](https://bevy.org):** A quick overview of Bevy's features.
* **[News](https://bevy.org/news/)**: A development blog that covers our progress, plans and shiny new features.

## Docs

* **[Quick Start Guide](https://bevy.org/learn/quick-start/introduction):** Bevy's official Quick Start Guide. The best place to start learning Bevy.
* **[Bevy Rust API Docs](https://docs.rs/bevy):** Bevy's Rust API docs, which are automatically generated from the doc comments in this repo.
* **[Official Examples](https://github.com/bevyengine/bevy/tree/latest/examples):** Bevy's dedicated, runnable examples, which are great for digging into specific concepts.
* **[Community-Made Learning Resources](https://bevy.org/assets/#learning)**: More tutorials, documentation, and examples made by the Bevy community.

## Community

Before contributing or participating in discussions with the community, you should familiarize yourself with our [**Code of Conduct**](./CODE_OF_CONDUCT.md).

* **[Discord](https://discord.gg/bevy):** Bevy's official discord server.
* **[Reddit](https://reddit.com/r/bevy):** Bevy's official subreddit.
* **[GitHub Discussions](https://github.com/bevyengine/bevy/discussions):** The best place for questions about Bevy, answered right here!
* **[Bevy Assets](https://bevy.org/assets/):** A collection of awesome Bevy projects, tools, plugins and learning materials.

### Contributing

If you'd like to help build Bevy, check out the **[Contributor's Guide](https://bevy.org/learn/contribute/introduction)**.
For simple problems, feel free to [open an issue](https://github.com/bevyengine/bevy/issues) or
[PR](https://github.com/bevyengine/bevy/pulls) and tackle it yourself!

For more complex architecture decisions and experimental mad science, please open a [GitHub Discussion](https://github.com/bevyengine/bevy/discussions) so we can brainstorm together effectively!

## Getting Started

We recommend checking out the [Quick Start Guide](https://bevy.org/learn/quick-start/introduction) for a brief introduction.

Follow the [Setup guide](https://bevy.org/learn/quick-start/getting-started/setup) to ensure your development environment is set up correctly.
Once set up, you can quickly try out the [examples](https://github.com/bevyengine/bevy/tree/latest/examples) by cloning this repo and running the following commands:

```sh
# Switch to the correct version (latest release, default is main development branch)
git checkout latest
# Runs the "breakout" example
cargo run --example breakout
```

To draw a window with standard functionality enabled, use:

```rust
use bevy::prelude::*;

## Cargo features (from the manifest)

- `default` (enables 4)
- `rg_web` (enables 9): Rose Griffon: trimmed set for WASM and WebGPU UI builds (no 3d, audio, gizmos, animation).
- `2d` (enables 5): PROFILE: The default 2D Bevy experience. This includes the core Bevy framework, 2D functionality, scenes and picking.
- `3d` (enables 5): PROFILE: The default 3D Bevy experience. This includes the core Bevy framework, 3D functionality, scenes and picking.
- `ui` (enables 7): PROFILE: The default Bevy UI experience. This includes the core Bevy framework, Bevy UI, scenes, and picking.
- `dev` (enables 6): COLLECTION: Enable this feature during development to improve the development experience. This adds features like asset hot-reloading and debugging tools. This should not be enabled for published apps
- `audio` (enables 2): COLLECTION: Features used to build audio Bevy apps.
- `audio-all-formats` (enables 7): COLLECTION: Enables audio features and all supported formats.
- `scene` (enables 2): COLLECTION: Features used to compose Bevy scenes.
- `picking` (enables 4): COLLECTION: Enables picking with all backends.
- `default_app` (enables 5): COLLECTION: The core pieces that most apps need. This serves as a baseline feature set for other higher level feature collections (such as "2d" and "3d"). It is also useful as a baseline feature set f
- `default_platform` (enables 11): COLLECTION: These are platform support features, such as OS support/features, windowing and input backends, etc.
- `common_api` (enables 13): COLLECTION: Default scene definition features. Note that this does not include an actual renderer, such as bevy_render (Bevy's default render backend).
- `2d_api` (enables 2): COLLECTION: Features used to build 2D Bevy apps (does not include a render backend). You generally don't need to worry about this unless you are using a custom renderer.
- `2d_bevy_render` (enables 6): COLLECTION: Bevy's built-in 2D renderer, built on top of `bevy_render`.
- `3d_api` (enables 9): COLLECTION: Features used to build 3D Bevy apps (does not include a render backend). You generally don't need to worry about this unless you are using a custom renderer.
- `3d_bevy_render` (enables 9): COLLECTION: Bevy's built-in 3D renderer, built on top of `bevy_render`.
- `ui_api` (enables 4): COLLECTION: Features used to build UI Bevy apps (does not include a render backend). You generally don't need to worry about this unless you are using a custom renderer.
- `ui_bevy_render` (enables 4): COLLECTION: Bevy's built-in UI renderer, built on top of `bevy_render`.
- `default_no_std` (enables 4): COLLECTION: Recommended defaults for `no_std` applications.
- `mesh_picking` (enables 1): Provides an implementation for picking meshes
- `sprite_picking` (enables 1): Provides an implementation for picking sprites
- `ui_picking` (enables 1): Provides an implementation for picking UI
- `bevy_ui_debug` (enables 1): Provides a debug overlay for Bevy UI
- `dynamic_linking` (enables 2): Force dynamic linking, which improves iterative compile times
- `sysinfo_plugin` (enables 1): Enables system information diagnostic plugin
- `bevy_animation` (enables 1): Provides animation functionality
- `bevy_curve` (enables 1): Provides mathematical curve functionality
- `bevy_asset` (enables 1): Provides asset functionality
- `bevy_audio` (enables 1): Provides audio functionality
- `bevy_color` (enables 1): Provides shared color types and operations
- `bevy_core_pipeline` (enables 1): Provides cameras and other basic render pipeline features
- `bevy_post_process` (enables 1): Provides post process effects such as depth of field, bloom, chromatic aberration.
- `bevy_anti_alias` (enables 1): Provides various anti-aliasing solutions
- `bevy_extract` (enables 1): Provides the ability to extract entities from an ECS main world to a sub world
- `bevy_gilrs` (enables 2): Adds gamepad support
- `bevy_gltf` (enables 1): [glTF](https://www.khronos.org/gltf/) support
- `bevy_pbr` (enables 1): Adds PBR rendering
- `bevy_picking` (enables 1): Provides picking functionality without any backend
- `bevy_render` (enables 1): Provides rendering functionality
- `bevy_world_serialization` (enables 1): Provides ECS serialization functionality
- `bevy_scene` (enables 1): Provides scene functionality
- `bevy_bsn` (enables 1): Provides BSN AST / parsing functionality
- `bevy_solari` (enables 1): Provides raytraced lighting (experimental)
- `bevy_sprite` (enables 1): Provides sprite functionality
- `bevy_sprite_render` (enables 1): Provides sprite rendering functionality
- `bevy_text` (enables 1): Provides text functionality
- `bevy_ui` (enables 1): A custom ECS-driven UI framework
- `bevy_ui_render` (enables 1): Provides rendering functionality for bevy_ui
- `bevy_window` (enables 1): Windowing layer
- `framepace_debug` (enables 2): Adds frame pacing diagnostics and logs
- `bevy_winit` (enables 1): winit window and input backend
- `bevy_image` (enables 1): Load and access image data. Usually added by an image format
- `bevy_mesh` (enables 1): Provides a mesh format and some primitive meshing routines.
- `bevy_mikktspace` (enables 1): Provides vertex tangent generation for use with bevy_mesh.
- `bevy_camera` (enables 1): Provides camera and visibility types, as well as culling primitives.
- `bevy_light` (enables 1): Provides light types such as point lights, directional lights, spotlights.
- `bevy_shader` (enables 1): Provides shaders usable through asset handles.
- `bevy_material` (enables 1): Provides materials.
- `bevy_gizmos` (enables 1): Adds support for gizmos

## Depends on (same ecosystem)

`bevy_internal`

## Examples in the source tree

- `third_party/bevy/examples/2d/2d_shapes.rs`
- `third_party/bevy/examples/2d/2d_viewport_to_world.rs`
- `third_party/bevy/examples/2d/bloom_2d.rs`
- `third_party/bevy/examples/2d/cpu_draw.rs`
- `third_party/bevy/examples/2d/dynamic_mip_generation.rs`
- `third_party/bevy/examples/2d/mesh2d.rs`
- `third_party/bevy/examples/2d/mesh2d_alpha_mode.rs`
- `third_party/bevy/examples/2d/mesh2d_arcs.rs`

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
