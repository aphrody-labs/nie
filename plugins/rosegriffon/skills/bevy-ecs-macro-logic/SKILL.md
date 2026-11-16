---
name: bevy-ecs-macro-logic
description: "Use when working with the `bevy_ecs_macro_logic` crate of bevy: Shared Bevy ECS Macro internals."
---

# bevy_ecs_macro_logic

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_ecs/macro_logic/` (manifest `third_party/bevy/crates/bevy_ecs/macro_logic/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_ecs/macro_logic
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Shared Bevy ECS Macro internals

## Crate docs (`third_party/bevy/crates/bevy_ecs/macro_logic/src/lib.rs`)

Reusable `bevy_ecs` macro logic. This enables defining derives that internally derive ECS traits
like `Component`.

## Depends on (same ecosystem)

`bevy_macro_utils`

## Public items by file

- `third_party/bevy/crates/bevy_ecs/macro_logic/src/component.rs`: enum StorageAttribute, struct DeriveComponent, enum HookAttributeKind, enum StorageTy, struct Require, struct Relationship, struct RelationshipTarget
- `third_party/bevy/crates/bevy_ecs/macro_logic/src/lib.rs`: mod component, mod map_entities
- `third_party/bevy/crates/bevy_ecs/macro_logic/src/map_entities.rs`: fn map_entities, enum MapEntitiesAttributeKind

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
