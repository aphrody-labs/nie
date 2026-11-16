---
name: bevy-sprite
description: "Use when working with the `bevy_sprite` crate of bevy: Provides sprite functionality for Bevy Engine."
---

# bevy_sprite

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_sprite/` (manifest `third_party/bevy/crates/bevy_sprite/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_sprite
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides sprite functionality for Bevy Engine

## README (`third_party/bevy/crates/bevy_sprite/README.md`)

# Bevy Sprite

## Cargo features (from the manifest)

- `bevy_picking` (enables 2)
- `bevy_text` (enables 2)

## Depends on (same ecosystem)

`bevy_app`, `bevy_asset`, `bevy_camera`, `bevy_color`, `bevy_derive`, `bevy_ecs`, `bevy_image`, `bevy_log`, `bevy_math`, `bevy_mesh`, `bevy_picking`, `bevy_reflect`, `bevy_text`, `bevy_transform`, `bevy_window`

## Public items by file

- `third_party/bevy/crates/bevy_sprite/src/lib.rs`: mod prelude, struct SpritePlugin
- `third_party/bevy/crates/bevy_sprite/src/picking_backend.rs`: struct SpritePickingCamera, enum SpritePickingMode, struct SpritePickingSettings, struct SpritePickingPlugin
- `third_party/bevy/crates/bevy_sprite/src/sprite.rs`: struct Sprite, enum SpriteAlphaMode, enum SpriteImageMode, enum SpriteScalingMode, struct Anchor
- `third_party/bevy/crates/bevy_sprite/src/text2d.rs`: struct Text2d, type Text2dReader, type Text2dWriter, struct Text2dShadow, fn update_text2d_layout, fn calculate_bounds_text2d
- `third_party/bevy/crates/bevy_sprite/src/texture_slice/border_rect.rs`: struct BorderRect
- `third_party/bevy/crates/bevy_sprite/src/texture_slice/slicer.rs`: struct TextureSlicer, enum SliceScaleMode

## Examples in the source tree

- `third_party/bevy/examples/reflection/mutation_by_reflection.rs`

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
