---
name: bevy-solari
description: "Use when working with the `bevy_solari` crate of bevy: Provides raytraced lighting for Bevy Engine."
---

# bevy_solari

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_solari/` (manifest `third_party/bevy/crates/bevy_solari/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_solari
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides raytraced lighting for Bevy Engine

## README (`third_party/bevy/crates/bevy_solari/README.md`)

# Bevy Solari

## Cargo features (from the manifest)

- `dlss` (enables 1)
- `force_disable_dlss` (enables 1)

## Depends on (same ecosystem)

`bevy_anti_alias`, `bevy_app`, `bevy_asset`, `bevy_camera`, `bevy_color`, `bevy_core_pipeline`, `bevy_derive`, `bevy_diagnostic`, `bevy_ecs`, `bevy_image`, `bevy_light`, `bevy_material`, `bevy_math`, `bevy_mesh`, `bevy_pbr`, `bevy_platform`, `bevy_reflect`, `bevy_render`, `bevy_shader`, `bevy_transform`, `bevy_utils`

## Public items by file

- `third_party/bevy/crates/bevy_solari/src/lib.rs`: mod pathtracer, mod realtime, mod scene, mod prelude, struct SolariPlugins
- `third_party/bevy/crates/bevy_solari/src/pathtracer/extract.rs`: fn extract_pathtracer
- `third_party/bevy/crates/bevy_solari/src/pathtracer/mod.rs`: struct PathtracingPlugin, struct Pathtracer
- `third_party/bevy/crates/bevy_solari/src/pathtracer/node.rs`: struct PathtracerPipelines, fn init_pathtracer_pipelines, fn pathtracer
- `third_party/bevy/crates/bevy_solari/src/pathtracer/prepare.rs`: struct PathtracerAccumulationTexture, fn prepare_pathtracer_accumulation_texture
- `third_party/bevy/crates/bevy_solari/src/realtime/extract.rs`: fn extract_solari_lighting
- `third_party/bevy/crates/bevy_solari/src/realtime/mod.rs`: struct SolariLightingPlugin, struct SolariLighting
- `third_party/bevy/crates/bevy_solari/src/realtime/node.rs`: struct SolariLightingPipelines, fn solari_lighting, fn init_solari_lighting_pipelines
- `third_party/bevy/crates/bevy_solari/src/realtime/prepare.rs`: fn setup_raytracing_scene_needs_previous_frame_data, struct SolariLightingResources, struct SolariReservoirBuffers, fn prepare_solari_lighting_resources
- `third_party/bevy/crates/bevy_solari/src/scene/binder/allocator.rs`: struct IndexAllocator, struct SlotAllocator, struct RetainedBindingArray
- `third_party/bevy/crates/bevy_solari/src/scene/binder/assets.rs`: struct GpuMaterial, struct AssetState
- `third_party/bevy/crates/bevy_solari/src/scene/binder/bind_group.rs`: struct BindGroupCacheState, fn prepare_raytracing_scene_bind_group, struct GpuEnvironmentMapLight
- `third_party/bevy/crates/bevy_solari/src/scene/binder/instances.rs`: struct GpuInstanceGeometryIds, struct GpuTransform, struct GpuBlasRef, struct InstanceState, type InstanceQueryData, type ChangedInstanceFilter, struct InstanceInputs
- `third_party/bevy/crates/bevy_solari/src/scene/binder/lights.rs`: struct GpuLightSource, enum LightSourceId, struct LightIndex, struct GpuDirectionalLight, struct GpuPointLight, struct GpuRectLight, struct LightState
- `third_party/bevy/crates/bevy_solari/src/scene/binder/mod.rs`: struct RaytracingSceneNeedsPreviousFrameData, struct RaytracingSceneBindings, fn prepare_raytracing_scene_resources
- `third_party/bevy/crates/bevy_solari/src/scene/binder/tlas.rs`: struct TlasInstanceSetupPipeline, struct TlasState, fn build_raytracing_tlas
- `third_party/bevy/crates/bevy_solari/src/scene/binder/tlas_build.rs`: trait RawTlasBackend, fn resolve
- `third_party/bevy/crates/bevy_solari/src/scene/blas.rs`: struct BlasManager, struct BlasKey, enum BlasOpacity, fn prepare_raytracing_blas, fn compact_raytracing_blas, fn delete_raytracing_blas
- `third_party/bevy/crates/bevy_solari/src/scene/extract.rs`: fn extract_raytracing_scene_structural, fn extract_raytracing_scene_transforms, fn extract_raytracing_scene_meshes_and_materials, struct StandardMaterialAssets, fn extract_raytracing_material_assets, struct ExtractedRaytracingDirectionalLight, struct ExtractedRaytracingPointLight, struct ExtractedRaytracingSpotLight, struct ExtractedRaytracingRectLight, struct ExtractedEnvironmentMapLight, fn extract_raytracing_directional_lights, fn extract_raytracing_point_lights, fn extract_raytracing_spot_lights, fn extract_raytracing_rect_lights, fn extract_raytracing_environment_map_light
- `third_party/bevy/crates/bevy_solari/src/scene/mod.rs`: struct RaytracingScenePlugin
- `third_party/bevy/crates/bevy_solari/src/scene/types.rs`: struct RaytracingMesh3d

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
