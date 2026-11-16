---
name: bevy-material-macros
description: "Use when working with the `bevy_material_macros` crate of bevy: Derive implementations for bevy_material."
---

# bevy_material_macros

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_material/macros/` (manifest `third_party/bevy/crates/bevy_material/macros/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_material/macros
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Derive implementations for bevy_material

## Depends on (same ecosystem)

`bevy_macro_utils`

## Public items by file

- `third_party/bevy/crates/bevy_material/macros/src/lib.rs`: derive ShaderLabel, fn derive_shader_label, derive DrawFunctionLabel, fn derive_draw_function_label

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
