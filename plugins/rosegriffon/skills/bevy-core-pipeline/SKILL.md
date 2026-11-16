---
name: bevy-core-pipeline
description: "Use when working with the `bevy_core_pipeline` crate of bevy: Provides a core render pipeline for Bevy Engine."
---

# bevy_core_pipeline

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_core_pipeline/` (manifest `third_party/bevy/crates/bevy_core_pipeline/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_core_pipeline
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides a core render pipeline for Bevy Engine.

## README (`third_party/bevy/crates/bevy_core_pipeline/README.md`)

# Bevy Core Pipeline

## Cargo features (from the manifest)

- `trace`
- `webgl`
- `webgpu`
- `tonemapping_luts` (enables 2)

## Depends on (same ecosystem)

`bevy_app`, `bevy_asset`, `bevy_camera`, `bevy_color`, `bevy_derive`, `bevy_diagnostic`, `bevy_ecs`, `bevy_extract`, `bevy_image`, `bevy_light`, `bevy_log`, `bevy_math`, `bevy_platform`, `bevy_reflect`, `bevy_render`, `bevy_shader`, `bevy_transform`, `bevy_utils`, `bevy_window`

## Public items by file

- `third_party/bevy/crates/bevy_core_pipeline/src/blit/mod.rs`: struct BlitPlugin, struct BlitPipeline, fn init_blit_pipeline, struct BlitPipelineKey
- `third_party/bevy/crates/bevy_core_pipeline/src/core_2d/main_opaque_pass_2d_node.rs`: fn main_opaque_pass_2d
- `third_party/bevy/crates/bevy_core_pipeline/src/core_2d/main_transparent_pass_2d_node.rs`: fn main_transparent_pass_2d
- `third_party/bevy/crates/bevy_core_pipeline/src/core_2d/mod.rs`: struct Core2dPlugin, struct Opaque2d, struct Opaque2dBinKey, struct BatchSetKey2d, struct AlphaMask2d, struct AlphaMask2dBinKey, struct Transparent2d, fn extract_core_2d_camera_phases, fn prepare_core_2d_depth_textures
- `third_party/bevy/crates/bevy_core_pipeline/src/core_3d/main_opaque_pass_3d_node.rs`: fn main_opaque_pass_3d
- `third_party/bevy/crates/bevy_core_pipeline/src/core_3d/main_transparent_pass_3d_node.rs`: fn main_transparent_pass_3d
- `third_party/bevy/crates/bevy_core_pipeline/src/core_3d/mod.rs`: struct Core3dPlugin, struct Opaque3d, struct Opaque3dBatchSetKey, struct Opaque3dBinKey, struct AlphaMask3d, struct Transparent3d, enum TransparentSortingInfo3d, fn extract_core_3d_camera_phases, fn extract_camera_prepass_phase, fn prepare_core_3d_depth_textures, fn check_msaa, fn prepare_prepass_textures
- `third_party/bevy/crates/bevy_core_pipeline/src/deferred/copy_lighting_id.rs`: struct CopyDeferredLightingIdPlugin, fn copy_deferred_lighting_id, struct CopyDeferredLightingIdPipeline, fn init_copy_deferred_lighting_id_pipeline, struct DeferredLightingIdDepthTexture
- `third_party/bevy/crates/bevy_core_pipeline/src/deferred/mod.rs`: mod copy_lighting_id, mod node, struct Opaque3dDeferred, struct AlphaMask3dDeferred
- `third_party/bevy/crates/bevy_core_pipeline/src/deferred/node.rs`: fn late_deferred_prepass
- `third_party/bevy/crates/bevy_core_pipeline/src/fullscreen_material.rs`: struct FullscreenMaterialPlugin, trait FullscreenMaterial, struct FullscreenMaterialPipeline, struct FullscreenMaterialPipelineId, struct FullscreenMaterialBindGroup, fn fullscreen_material_system
- `third_party/bevy/crates/bevy_core_pipeline/src/fullscreen_vertex_shader/mod.rs`: struct FullscreenShader
- `third_party/bevy/crates/bevy_core_pipeline/src/lib.rs`: mod blit, mod core_2d, mod core_3d, mod deferred, mod fullscreen_material, mod mip_generation, mod oit, mod prepass, mod schedule, mod skybox, mod tonemapping, mod upscaling, struct CorePipelinePlugin
- `third_party/bevy/crates/bevy_core_pipeline/src/mip_generation/experimental/depth.rs`: fn early_downsample_depth, fn late_downsample_depth, struct DownsampleDepthPipeline, struct DownsampleDepthPipelines, fn create_downsample_depth_pipelines, struct DepthPyramidDummyTexture, fn init_depth_pyramid_dummy_texture, fn create_depth_pyramid_dummy_texture, struct ViewDepthPyramid, fn prepare_view_depth_pyramids, struct ViewDownsampleDepthBindGroup, fn prepare_downsample_depth_view_bind_groups
- `third_party/bevy/crates/bevy_core_pipeline/src/mip_generation/experimental/mod.rs`: mod depth
- `third_party/bevy/crates/bevy_core_pipeline/src/mip_generation/mod.rs`: mod experimental, struct DownsampleShaders, struct DownsamplePipeline, struct DownsamplePipelineKey, enum DownsamplePass, struct MipGenerationJobs, struct MipGenerationPhase, struct MipGenerationPhaseId, struct MipGenerationPipelines, struct DownsamplingConstants, struct MipGenerationPlugin, fn generate_mips_for_phase, fn can_combine_downsampling_bind_groups
- `third_party/bevy/crates/bevy_core_pipeline/src/oit/mod.rs`: mod resolve, struct OrderIndependentTransparencySettings, struct OrderIndependentTransparencyPlugin, struct OitFragmentNode, struct OitBuffers, fn init_oit_buffers, struct OrderIndependentTransparencySettingsOffset, fn prepare_oit_buffers
- `third_party/bevy/crates/bevy_core_pipeline/src/oit/resolve/mod.rs`: mod node, struct OitResolvePlugin, fn is_oit_supported, struct OitResolveBindGroup, struct OitResolvePipeline, struct OitResolvePipelineId, struct OitResolvePipelineKey, fn queue_oit_resolve_pipeline, fn prepare_oit_resolve_bind_group
- `third_party/bevy/crates/bevy_core_pipeline/src/oit/resolve/node.rs`: fn oit_resolve
- `third_party/bevy/crates/bevy_core_pipeline/src/prepass/background_motion_vectors.rs`: struct NoBackgroundMotionVectors, struct BackgroundMotionVectorsPipelineId, struct BackgroundMotionVectorsBindGroup, struct BackgroundMotionVectorsPlugin
- `third_party/bevy/crates/bevy_core_pipeline/src/prepass/mod.rs`: mod background_motion_vectors, mod node, struct DepthPrepass, struct NormalPrepass, struct MotionVectorPrepass, struct DeferredPrepass, struct DepthPrepassDoubleBuffer, struct DeferredPrepassDoubleBuffer, struct PreviousViewData, struct PreviousViewUniforms, struct PreviousViewUniformOffset, struct ViewPrepassTextures, struct Opaque3dPrepass, struct OpaqueNoLightmap3dBatchSetKey, struct OpaqueNoLightmap3dBinKey, struct AlphaMask3dPrepass, fn prepass_target_descriptors
- `third_party/bevy/crates/bevy_core_pipeline/src/prepass/node.rs`: fn early_prepass, fn late_prepass
- `third_party/bevy/crates/bevy_core_pipeline/src/schedule.rs`: struct Core3d, enum Core3dSystems, struct Core2d, enum Core2dSystems, struct RootNonCameraView, fn camera_driver
- `third_party/bevy/crates/bevy_core_pipeline/src/skybox/mod.rs`: struct SkyboxPlugin, fn extract_skybox, struct SkyboxUniforms, struct SkyboxPipelineId, struct SkyboxBindGroup
- `third_party/bevy/crates/bevy_core_pipeline/src/tonemapping/mod.rs`: struct TonemappingLuts, struct TonemappingPlugin, fn check_tonemapping_none, struct TonemappingPipeline, struct TonemappingPipelineKey, fn init_tonemapping_pipeline, struct ViewTonemappingPipeline, fn prepare_view_tonemapping_pipelines, fn get_lut_bindings, fn get_lut_bind_group_layout_entries, fn lut_placeholder
- `third_party/bevy/crates/bevy_core_pipeline/src/tonemapping/node.rs`: struct CachedTonemappingBindGroup, fn tonemapping
- `third_party/bevy/crates/bevy_core_pipeline/src/upscaling/mod.rs`: struct UpscalingPlugin, struct ViewUpscalingPipeline
- `third_party/bevy/crates/bevy_core_pipeline/src/upscaling/node.rs`: struct UpscalingBindGroupCache, fn upscaling

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
