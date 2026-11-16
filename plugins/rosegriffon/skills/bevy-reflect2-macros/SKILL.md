---
name: bevy-reflect2-macros
description: "Use when working with the `bevy_reflect2_macros` crate of bevy: Bevy Reflect Macros."
---

# bevy_reflect2_macros

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_reflect2/macros/` (manifest `third_party/bevy/crates/bevy_reflect2/macros/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_reflect2/macros
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Bevy Reflect Macros

## Cargo features (from the manifest)

- `default`
- `auto_register`: Enables automatic reflect registration. Does nothing by itself, must select `auto_register_inventory` or `auto_register_static` to make it work.
- `auto_register_inventory` (enables 1): Enables automatic reflection using inventory. Not supported on all platforms.
- `auto_register_static` (enables 2): Enables automatic reflection on platforms not supported by inventory. See `load_type_registrations` for more info.

## Depends on (same ecosystem)

`bevy_macro_utils`

## Public items by file

- `third_party/bevy/crates/bevy_reflect2/macros/src/lib.rs`: derive Reflect, fn derive_reflect, macro impl_reflect, macro load_type_registrations

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
