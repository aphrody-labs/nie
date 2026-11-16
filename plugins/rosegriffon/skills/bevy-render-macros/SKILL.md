---
name: bevy-render-macros
description: "Use when working with the `bevy_render_macros` crate of bevy: Derive implementations for bevy_render."
---

# bevy_render_macros

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_render/macros/` (manifest `third_party/bevy/crates/bevy_render/macros/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_render/macros
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Derive implementations for bevy_render

## Depends on (same ecosystem)

`bevy_extract_macros`, `bevy_macro_utils`

## Public items by file

- `third_party/bevy/crates/bevy_render/macros/src/as_bind_group.rs`: fn derive_as_bind_group
- `third_party/bevy/crates/bevy_render/macros/src/lib.rs`: fn derive_as_bind_group, derive RenderLabel, fn derive_render_label, derive Specializer, fn derive_specialize, derive SpecializerKey, fn derive_specializer_key
- `third_party/bevy/crates/bevy_render/macros/src/specializer.rs`: fn impl_specializer, fn impl_specializer_key

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
