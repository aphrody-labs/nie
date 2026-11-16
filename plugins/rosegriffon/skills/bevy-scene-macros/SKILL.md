---
name: bevy-scene-macros
description: "Use when working with the `bevy_scene_macros` crate of bevy: Derive implementations for bevy_scene."
---

# bevy_scene_macros

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_scene/macros/` (manifest `third_party/bevy/crates/bevy_scene/macros/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_scene/macros
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Derive implementations for bevy_scene

## Crate docs (`third_party/bevy/crates/bevy_scene/macros/src/lib.rs`)

Macros for [`bevy_scene`], including the `bsn!` scene notation macros and the
`SceneComponent` derive.

[`bevy_scene`]: https://docs.rs/bevy/latest/bevy/scene/index.html

## Depends on (same ecosystem)

`bevy_ecs_macro_logic`, `bevy_macro_utils`

## Public items by file

- `third_party/bevy/crates/bevy_scene/macros/src/bsn/codegen.rs`: trait BsnTokenStream
- `third_party/bevy/crates/bevy_scene/macros/src/bsn/mod.rs`: mod codegen, mod parse, mod types, fn bsn, fn bsn_list
- `third_party/bevy/crates/bevy_scene/macros/src/bsn/types.rs`: struct BsnRoot, struct BsnListRoot, struct Bsn, enum BsnEntry, struct BsnType, struct BsnStructUpdate, struct BsnRelatedSceneList, struct BsnSceneList, struct BsnSceneListItems, enum BsnSceneListItem, struct BsnSceneFn, enum BsnScene, struct BsnConstructor, struct BsnFnCall, enum BsnFields, struct BsnTuple, struct BsnNamedField, enum BsnNamedFieldOrStructUpdate, struct BsnUnnamedField, enum BsnValue, enum BsnFnArg, struct BsnFnArgs
- `third_party/bevy/crates/bevy_scene/macros/src/lib.rs`: macro bsn, macro bsn_list, fn derive_scene_component

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
