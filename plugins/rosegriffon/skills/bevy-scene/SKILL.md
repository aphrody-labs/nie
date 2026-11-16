---
name: bevy-scene
description: "Use when working with the `bevy_scene` crate of bevy: Provides scene functionality for Bevy Engine."
---

# bevy_scene

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_scene/` (manifest `third_party/bevy/crates/bevy_scene/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_scene
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides scene functionality for Bevy Engine

## Crate docs (`third_party/bevy/crates/bevy_scene/src/lib.rs`)

Composable scene authoring for Bevy, defined using the Bevy Scene Notation (BSN) format.

Game entities rarely exist in isolation.
A 3D level might be made up of walls, floors, props and enemies.
A 2D character might need a distinct sprite entity for weapon, hat and boots.
A UI popup might need text and multiple buttons for accept, cancel, minimize and close actions.
Spawning these collections as individual, disjointed entities is tedious, error-prone, and hard to reuse.
A **scene** lets you describe a conceptual **object**, made of an entity, its components, children, and assets, once
and spawn it wherever you need it.

Any scene system must overcome three challenges:

- **Composability**: combining smaller scenes into larger ones without
  duplicating shared constants and setup code.
- **Granular overrides**: when reusing a scene, overriding *individual fields*
  on a component (like changing just a button's width) without having to respecify
  every other field on that component.
- **Asset integration**: referencing assets (meshes, textures, sounds) from within
  scenes without manually wiring up asset handles.

This crate tackles all three, via [`Scene`] composition, [`Template`]-based
field-level patching, and automatic string-to-asset-handle resolution.

The [`bsn!`] macro exposes these ideas, and makes the process of scene-authoring pleasant
by providing a terse syntax for defining [`Scene`]s inline.
This brevity is essential: making it easier to review and understand scenes at a glance,
resolve merge conflicts and keep file sizes under control.
The macro includes best-effort Rust-Analyzer support. Autocomplete, go-to-definition, and hover docs
should work inside the macros, and this effort should transfer over correctly to other LSPs!

## BSN syntax reference

For a quick rundown on how to read and write BSN syntax, see the docs for [`bsn!`].

## Quick Start

Spawn entities in a [`Scene`] by calling [`World::spawn_scene`],
wrapping a call to the [`bsn!`] macro.

```
# use bevy_app::App;
# use bevy_scene::{prelude::*, ScenePlugin};
# use bevy_ecs::prelude::*;
# use bevy_asset::AssetPlugin;
# use bevy_app::TaskPoolPlugin;
# let mut app = App::new();
# app.add_plugins((
#     TaskPoolPlugin::default(),
#     AssetPlugin::default(),
#     ScenePlugin::default(),
# ));
# let world = app.world_mut();
#[derive(Component, Default, Clone)]
struct Score(usize);

#[derive(Component, Default, Clone)]
struct Sword;

#[derive(Component, Default, Clone)]
struct Shield;

## Depends on (same ecosystem)

`bevy_app`, `bevy_asset`, `bevy_derive`, `bevy_ecs`, `bevy_platform`, `bevy_reflect`, `bevy_scene_macros`, `bevy_utils`

## Public items by file

- `third_party/bevy/crates/bevy_scene/src/lib.rs`: mod prelude, mod macro_utils, struct ScenePlugin, struct Ready
- `third_party/bevy/crates/bevy_scene/src/macro_utils.rs`: fn touch_type, type PathResolveHelper, struct CallCounter
- `third_party/bevy/crates/bevy_scene/src/resolved_scene.rs`: struct ResolvedSceneRoot, struct ResolvedSceneListRoot, struct ResolvedScene, enum CachedSceneError, enum ApplySceneError, struct RelatedResolvedScenes, trait ErasedTemplate, trait SceneEffect, struct EmptySceneEffect, trait ErasedBundleTemplate
- `third_party/bevy/crates/bevy_scene/src/scene.rs`: trait Scene, trait SceneBox, struct SceneDependencies, struct SceneDependency, enum ResolveSceneError, struct ResolveContext, struct TemplatePatch, trait PatchFromTemplate, trait PatchTemplate, struct InsertTemplate, struct RelatedScenes, struct CachedSceneAsset, struct NameEntityReference, struct SceneScope, struct SceneListScope, struct OnTemplate, fn on, struct InitTemplate, struct SceneFunction
- `third_party/bevy/crates/bevy_scene/src/scene_component.rs`: trait SceneComponent, struct SceneComponentInfo
- `third_party/bevy/crates/bevy_scene/src/scene_list.rs`: trait SceneList, trait SceneListBox, struct EntityScene
- `third_party/bevy/crates/bevy_scene/src/scene_patch.rs`: struct ScenePatch, enum SpawnSceneError, struct ScenePatchInstance, struct SceneListPatch
- `third_party/bevy/crates/bevy_scene/src/spawn.rs`: trait WorldSceneExt, trait CommandsSceneExt, trait EntityWorldMutSceneExt, trait EntityCommandsSceneExt, fn resolve_scene_patches, struct QueuedScenes, struct WaitingScenes, fn on_add_scene_patch_instance, fn spawn_queued
- `third_party/bevy/crates/bevy_scene/src/spawn_system.rs`: trait SpawnSystem, trait SpawnListSystem

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
