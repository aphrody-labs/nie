---
name: bevy-reflect-derive
description: "Use when working with the `bevy_reflect_derive` crate of bevy: Derive implementations for bevy_reflect."
---

# bevy_reflect_derive

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_reflect/derive/` (manifest `third_party/bevy/crates/bevy_reflect/derive/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_reflect/derive
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Derive implementations for bevy_reflect

## Crate docs (`third_party/bevy/crates/bevy_reflect/derive/src/lib.rs`)

This crate contains macros used by Bevy's `Reflect` API.

The main export of this crate is the derive macro for [`Reflect`]. This allows
types to easily implement `Reflect` along with other `bevy_reflect` traits,
such as `Struct`, `GetTypeRegistration`, and more— all with a single derive!

Some other noteworthy exports include the derive macros for [`FromReflect`] and
[`TypePath`], as well as the [`reflect_trait`] attribute macro.

[`Reflect`]: crate::derive_reflect
[`FromReflect`]: crate::derive_from_reflect
[`TypePath`]: crate::derive_type_path
[`reflect_trait`]: macro@reflect_trait

## Cargo features (from the manifest)

- `default`
- `reflect_documentation`: When enabled, allows documentation comments to be processed by the reflection macros
- `functions`: Enables macro logic related to function reflection
- `auto_register`: Enables automatic reflect registration. Does nothing by itself, must select `auto_register_inventory` or `auto_register_static` to make it work.
- `auto_register_inventory` (enables 1): Enables automatic reflection using inventory. Not supported on all platforms.
- `auto_register_static` (enables 2): Enables automatic reflection on platforms not supported by inventory. See `load_type_registrations` for more info.

## Depends on (same ecosystem)

`bevy_macro_utils`

## Public items by file

- `third_party/bevy/crates/bevy_reflect/derive/src/impls/common.rs`: fn impl_full_reflect, fn common_partial_reflect_methods, fn reflect_auto_registration
- `third_party/bevy/crates/bevy_reflect/derive/src/lib.rs`: derive Reflect, fn derive_reflect, derive FromReflect, fn derive_from_reflect, derive TypePath, fn derive_type_path, attribute reflect_trait, attribute reflect_remote, macro impl_reflect_opaque, macro impl_reflect, macro impl_from_reflect_opaque, macro impl_type_path, macro load_type_registrations

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
