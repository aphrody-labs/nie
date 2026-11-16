---
name: bevy-derive
description: "Use when working with the `bevy_derive` crate of bevy: Provides derive implementations for Bevy Engine."
---

# bevy_derive

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_derive/` (manifest `third_party/bevy/crates/bevy_derive/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_derive
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides derive implementations for Bevy Engine

## README (`third_party/bevy/crates/bevy_derive/README.md`)

# Bevy Derive

## Depends on (same ecosystem)

`bevy_macro_utils`

## Public items by file

- `third_party/bevy/crates/bevy_derive/src/bevy_main.rs`: fn bevy_main
- `third_party/bevy/crates/bevy_derive/src/derefs.rs`: fn derive_deref, fn derive_deref_mut
- `third_party/bevy/crates/bevy_derive/src/enum_variant_meta.rs`: fn derive_enum_variant_meta
- `third_party/bevy/crates/bevy_derive/src/lib.rs`: derive Deref, fn derive_deref, derive DerefMut, fn derive_deref_mut, attribute bevy_main, derive EnumVariantMeta, fn derive_enum_variant_meta, derive AppLabel, fn derive_app_label

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
