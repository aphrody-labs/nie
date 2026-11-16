---
name: bevy-extract
description: "Use when working with the `bevy_extract` crate of bevy: Provides extract functionality between ECS worlds for Bevy Engine."
---

# bevy_extract

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_extract/` (manifest `third_party/bevy/crates/bevy_extract/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_extract
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides extract functionality between ECS worlds for Bevy Engine

## README (`third_party/bevy/crates/bevy_extract/README.md`)

# Bevy Extract


This crate makes it easy to copy out main world state to a sub app's world.

## Cargo features (from the manifest)

- `default`
- `trace`

## Depends on (same ecosystem)

`bevy_app`, `bevy_camera`, `bevy_derive`, `bevy_ecs`, `bevy_extract_macros`, `bevy_log`, `bevy_platform`, `bevy_reflect`, `bevy_time`, `bevy_utils`

## Public items by file

- `third_party/bevy/crates/bevy_extract/src/extract_component.rs`: trait ExtractComponent, struct ExtractComponentPlugin
- `third_party/bevy/crates/bevy_extract/src/extract_instances.rs`: trait ExtractInstance, struct ExtractInstancesPlugin, struct ExtractedInstances
- `third_party/bevy/crates/bevy_extract/src/extract_param.rs`: struct Extract, struct ExtractState
- `third_party/bevy/crates/bevy_extract/src/extract_plugin.rs`: struct ExtractPlugin, struct ExtractSchedule, struct MainWorld, fn extract
- `third_party/bevy/crates/bevy_extract/src/extract_resource.rs`: trait ExtractResource, struct ExtractResourcePlugin, fn extract_resource
- `third_party/bevy/crates/bevy_extract/src/lib.rs`: mod extract_component, mod extract_instances, mod extract_param, mod extract_plugin, mod extract_resource, mod sync_component, mod sync_world
- `third_party/bevy/crates/bevy_extract/src/sync_component.rs`: struct SyncComponentPlugin, trait SyncComponent
- `third_party/bevy/crates/bevy_extract/src/sync_world.rs`: struct SyncWorldPlugin, struct SyncToSubWorld, struct SubEntity, struct MainEntity, type MainEntityHashMap, type MainEntityHashSet, struct TemporaryEntity

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
