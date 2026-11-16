---
name: bevy-transform
description: "Use when working with the `bevy_transform` crate of bevy: Provides transform functionality for Bevy Engine."
---

# bevy_transform

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_transform/` (manifest `third_party/bevy/crates/bevy_transform/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_transform
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides transform functionality for Bevy Engine

## README (`third_party/bevy/crates/bevy_transform/README.md`)

# Bevy Transform


This crate contains types and functions associated with the `Transform` component.

## Cargo features (from the manifest)

- `default` (enables 4): Turning off default features leaves you with a barebones definition of transform.
- `multi_threaded` (enables 1): Enables multithreading support. Schedules will attempt to run systems on multiple threads whenever possible.
- `bevy-support` (enables 3): Adds normal Bevy impls like deriving components, bundles, reflection, as well as adding systems for transform propagation and more. This exists because it allows opting out of all of this, leaving onl
- `serialize` (enables 2): Adds serialization support through `serde`.
- `bevy_reflect` (enables 5): Adds runtime reflection support using `bevy_reflect`.
- `trace` (enables 2): Enables `tracing` integration, allowing spans and other metrics to be reported through that framework.
- `async_executor` (enables 2): Uses `async-executor` as a task execution backend. This backend is incompatible with `no_std` targets.
- `std` (enables 8): Allows access to the `std` crate. Enabling this feature will prevent compilation on `no_std` targets, but provides access to certain additional features on supported platforms.
- `critical-section` (enables 3): `critical-section` provides the building blocks for synchronization primitives on all platforms, including `no_std`.
- `alloc` (enables 1): Allows access to the `alloc` crate.
- `libm` (enables 1): Uses the `libm` maths library instead of the one provided in `std` and `core`.

## Depends on (same ecosystem)

`bevy_app`, `bevy_ecs`, `bevy_math`, `bevy_reflect`, `bevy_tasks`, `bevy_utils`

## Public items by file

- `third_party/bevy/crates/bevy_transform/src/commands.rs`: trait BuildChildrenTransformExt
- `third_party/bevy/crates/bevy_transform/src/components/global_transform.rs`: struct GlobalTransform
- `third_party/bevy/crates/bevy_transform/src/components/transform.rs`: struct Transform, struct TransformTreeChanged
- `third_party/bevy/crates/bevy_transform/src/helper.rs`: struct TransformHelper, enum ComputeGlobalTransformError
- `third_party/bevy/crates/bevy_transform/src/lib.rs`: mod commands, mod components, mod traits, mod plugins, mod helper, mod systems, mod prelude
- `third_party/bevy/crates/bevy_transform/src/plugins.rs`: enum TransformSystems, struct TransformPlugin
- `third_party/bevy/crates/bevy_transform/src/systems.rs`: fn propagate_transforms_for, fn sync_simple_transforms, enum StaticTransformOptimizations, fn mark_dirty_trees
- `third_party/bevy/crates/bevy_transform/src/traits.rs`: trait TransformPoint

## Examples in the source tree

- `third_party/bevy/examples/reflection/mutation_by_reflection.rs`

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
