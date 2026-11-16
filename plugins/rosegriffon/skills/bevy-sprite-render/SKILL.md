---
name: bevy-sprite-render
description: "Use when working with the `bevy_sprite_render` crate of bevy: Provides sprite rendering functionality for Bevy Engine."
---

# bevy_sprite_render

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_sprite_render/` (manifest `third_party/bevy/crates/bevy_sprite_render/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_sprite_render
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides sprite rendering functionality for Bevy Engine

## README (`third_party/bevy/crates/bevy_sprite_render/README.md`)

# Bevy Sprite

## Cargo features (from the manifest)

- `webgl`
- `webgpu`
- `bevy_text` (enables 2)

## Depends on (same ecosystem)

`bevy_app`, `bevy_asset`, `bevy_camera`, `bevy_color`, `bevy_core_pipeline`, `bevy_derive`, `bevy_ecs`, `bevy_extract`, `bevy_image`, `bevy_material`, `bevy_math`, `bevy_mesh`, `bevy_platform`, `bevy_reflect`, `bevy_render`, `bevy_shader`, `bevy_shape`, `bevy_sprite`, `bevy_text`, `bevy_transform`, `bevy_utils`

## Public items by file

- `third_party/bevy/crates/bevy_sprite_render/src/lib.rs`: mod prelude, struct SpriteRenderPlugin
- `third_party/bevy/crates/bevy_sprite_render/src/mesh2d/color_material.rs`: struct ColorMaterialPlugin, struct ColorMaterial, struct ColorMaterialUniform
- `third_party/bevy/crates/bevy_sprite_render/src/mesh2d/extended_material.rs`: trait MaterialExtension2d, struct ExtendedMaterial2d
- `third_party/bevy/crates/bevy_sprite_render/src/mesh2d/material.rs`: trait Material2d, struct MeshMaterial2d, enum AlphaMode2d, struct Materials2dPlugin, struct Material2dPlugin, struct RenderMaterial2dInstances, fn extract_mesh_materials_2d, struct Material2dPipeline, struct Material2dPipelineSpecializer, struct Material2dKey, fn init_material_2d_pipeline, struct SetMaterial2dBindGroup, fn alpha_mode_pipeline_key_2d, fn tonemapping_pipeline_key, fn extract_entities_needs_specialization, fn extract_entities_that_need_specializations_removed, struct EntitiesNeedingSpecialization, struct SpecializedMaterial2dPipelineCache, struct SpecializedMaterial2dViewPipelineCache, fn check_entities_needing_specialization, struct PendingMeshMaterial2dQueues, fn prepare_pending_mesh_material2d_queues, struct SpecializeMaterial2dMeshesSystemParam, fn specialize_material2d_meshes, ...
- `third_party/bevy/crates/bevy_sprite_render/src/mesh2d/mesh.rs`: struct Mesh2dRenderPlugin, struct ViewKeyCache, fn check_views_need_specialization, fn init_batched_instance_buffer, struct Mesh2dTransforms, struct Mesh2dUniform, struct RenderMesh2dInstance, struct RenderMesh2dInstances, struct Mesh2dMarker, fn extract_2d_meshes, struct Mesh2dPipeline, fn init_mesh_2d_pipeline, struct Mesh2dBindGroup, fn prepare_mesh2d_bind_group, struct Mesh2dViewBindGroup, fn prepare_mesh2d_view_bind_groups, struct SetMesh2dViewBindGroup, struct SetMesh2dBindGroup, struct DrawMesh2d
- `third_party/bevy/crates/bevy_sprite_render/src/mesh2d/wireframe2d.rs`: struct Wireframe2dPlugin, struct Wireframe2d, struct Wireframe2dPhaseItem, struct Wireframe2dBatchSetKey, struct Wireframe2dBinKey, struct SetWireframe2dImmediates, type DrawWireframe2d, struct Wireframe2dPipeline, fn init_wireframe_2d_pipeline, struct Wireframe2dColor, struct ExtractedWireframeColor, struct NoWireframe2d, struct Wireframe2dConfig, struct Wireframe2dMaterial, struct RenderWireframeMaterial, struct Mesh2dWireframe, struct RenderWireframeInstances, struct WireframeEntitiesNeedingSpecialization, struct SpecializedWireframePipelineCache, struct SpecializedWireframeViewPipelineCache, fn extract_wireframe_materials, fn extract_wireframe_2d_entities_needing_specialization, fn extract_wireframe_2d_entities_that_need_specializations_removed, fn check_wireframe_entities_needing_specialization, ...
- `third_party/bevy/crates/bevy_sprite_render/src/render/mod.rs`: struct SpritePipeline, fn init_sprite_pipeline, struct ExtractedSlice, struct ExtractedSprite, enum ExtractedSpriteKind, struct ExtractedSprites, struct ExtractedSlices, struct SpriteAssetEvents, fn extract_sprite_events, struct SpriteMeta, struct SpriteViewBindGroup, struct SpriteBatches, struct SpriteBatch, struct ImageBindGroups, fn queue_sprites, fn prepare_sprite_view_bind_groups, fn prepare_sprite_image_bind_groups, type DrawSprite, struct SetSpriteViewBindGroup, struct SetSpriteTextureBindGroup, struct DrawSpriteBatch
- `third_party/bevy/crates/bevy_sprite_render/src/sprite_mesh/mod.rs`: struct SpriteMeshPlugin
- `third_party/bevy/crates/bevy_sprite_render/src/sprite_mesh/sprite_extended_material.rs`: struct SpriteMaterialPlugin, struct SpriteMaterial
- `third_party/bevy/crates/bevy_sprite_render/src/sprite_mesh/sprite_mesh_material.rs`: struct SpriteMeshMaterialPlugin, struct SpriteMeshMaterial, struct SpriteMeshMaterialUniform
- `third_party/bevy/crates/bevy_sprite_render/src/text2d/mod.rs`: fn extract_text2d_sprite
- `third_party/bevy/crates/bevy_sprite_render/src/tilemap_chunk/mod.rs`: struct TilemapChunkPlugin, struct TilemapChunkMeshCache, struct TilemapChunk, struct TileData, struct TilemapChunkTileData, fn update_tilemap_chunk_indices
- `third_party/bevy/crates/bevy_sprite_render/src/tilemap_chunk/tile_orientation.rs`: enum TileOrientation
- `third_party/bevy/crates/bevy_sprite_render/src/tilemap_chunk/tilemap_chunk_material.rs`: struct TilemapChunkMaterialPlugin, struct TilemapChunkMaterial, struct PackedTileData, fn make_chunk_tile_data_image

## Examples in the source tree

- `third_party/bevy/examples/2d/mesh2d_manual.rs`

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
