---
name: bevy-world-serialization
description: "Use when working with the `bevy_world_serialization` crate of bevy: Provides ECS World serialization functionality for Bevy Engine."
---

# bevy_world_serialization

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_world_serialization/` (manifest `third_party/bevy/crates/bevy_world_serialization/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_world_serialization
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides ECS World serialization functionality for Bevy Engine

## README (`third_party/bevy/crates/bevy_world_serialization/README.md`)

# Bevy World Serialization

## Cargo features (from the manifest)

- `default` (enables 1)
- `serialize` (enables 5)

## Depends on (same ecosystem)

`bevy_app`, `bevy_asset`, `bevy_camera`, `bevy_derive`, `bevy_ecs`, `bevy_platform`, `bevy_reflect`, `bevy_transform`, `bevy_utils`

## Public items by file

- `third_party/bevy/crates/bevy_world_serialization/src/components.rs`: struct WorldAssetRoot, struct DynamicWorldRoot
- `third_party/bevy/crates/bevy_world_serialization/src/dynamic_world.rs`: struct DynamicWorld, struct DynamicEntity, fn serialize_ron
- `third_party/bevy/crates/bevy_world_serialization/src/dynamic_world_builder.rs`: struct DynamicWorldBuilder
- `third_party/bevy/crates/bevy_world_serialization/src/lib.rs`: mod serde, mod prelude, struct WorldSerializationPlugin
- `third_party/bevy/crates/bevy_world_serialization/src/serde.rs`: struct DynamicWorldSerializer, struct EntitiesSerializer, struct EntitySerializer, struct WorldMapSerializer, struct WorldDeserializer, struct WorldEntitiesDeserializer, struct WorldEntityDeserializer, struct WorldMapDeserializer
- `third_party/bevy/crates/bevy_world_serialization/src/world_asset.rs`: struct WorldAsset
- `third_party/bevy/crates/bevy_world_serialization/src/world_asset_loader.rs`: struct WorldAssetLoader, enum WorldAssetLoaderError
- `third_party/bevy/crates/bevy_world_serialization/src/world_asset_spawner.rs`: struct WorldInstanceReady, struct InstanceId, struct WorldInstanceSpawner, enum WorldInstanceSpawnError, fn world_instance_spawner_system, struct WorldInstance, fn world_instance_spawner
- `third_party/bevy/crates/bevy_world_serialization/src/world_filter.rs`: enum WorldFilter

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
