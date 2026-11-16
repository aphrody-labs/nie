---
name: bevy-gizmos-render
description: "Use when working with the `bevy_gizmos_render` crate of bevy: Provides gizmos rendering for Bevy Engine."
---

# bevy_gizmos_render

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_gizmos_render/` (manifest `third_party/bevy/crates/bevy_gizmos_render/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_gizmos_render
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides gizmos rendering for Bevy Engine

## README (`third_party/bevy/crates/bevy_gizmos_render/README.md`)

# Bevy Gizmos Render

## Cargo features (from the manifest)

- `webgl`
- `webgpu`

## Depends on (same ecosystem)

`bevy_app`, `bevy_asset`, `bevy_camera`, `bevy_color`, `bevy_core_pipeline`, `bevy_ecs`, `bevy_extract`, `bevy_gizmos`, `bevy_image`, `bevy_log`, `bevy_material`, `bevy_math`, `bevy_mesh`, `bevy_pbr`, `bevy_reflect`, `bevy_render`, `bevy_shader`, `bevy_shape`, `bevy_sprite_render`, `bevy_transform`, `bevy_utils`

## Public items by file

- `third_party/bevy/crates/bevy_gizmos_render/src/lib.rs`: enum GizmoRenderSystems, mod retained, mod transform_gizmo_render, struct GizmoRenderPlugin, struct LineGizmoEntities
- `third_party/bevy/crates/bevy_gizmos_render/src/pipeline_2d.rs`: struct LineGizmo2dPlugin
- `third_party/bevy/crates/bevy_gizmos_render/src/pipeline_3d.rs`: struct LineGizmo3dPlugin
- `third_party/bevy/crates/bevy_gizmos_render/src/transform_gizmo_render.rs`: struct TransformGizmoRenderPlugin

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
