---
name: bevy-post-process
description: "Use when working with the `bevy_post_process` crate of bevy: Provides post process effects for Bevy Engine."
---

# bevy_post_process

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_post_process/` (manifest `third_party/bevy/crates/bevy_post_process/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_post_process
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides post process effects for Bevy Engine.

## README (`third_party/bevy/crates/bevy_post_process/README.md`)

# Bevy Post Process

## Cargo features (from the manifest)

- `trace`
- `webgl`
- `webgpu`

## Depends on (same ecosystem)

`bevy_app`, `bevy_asset`, `bevy_camera`, `bevy_color`, `bevy_core_pipeline`, `bevy_curve`, `bevy_derive`, `bevy_ecs`, `bevy_extract`, `bevy_image`, `bevy_math`, `bevy_platform`, `bevy_reflect`, `bevy_render`, `bevy_shader`, `bevy_utils`

## Public items by file

- `third_party/bevy/crates/bevy_post_process/src/auto_exposure/compensation_curve.rs`: struct AutoExposureCompensationCurve, enum AutoExposureCompensationCurveError, struct GpuAutoExposureCompensationCurve
- `third_party/bevy/crates/bevy_post_process/src/auto_exposure/mod.rs`: struct AutoExposurePlugin, fn init_auto_exposure_resources
- `third_party/bevy/crates/bevy_post_process/src/auto_exposure/pipeline.rs`: struct AutoExposurePipeline, struct ViewAutoExposurePipeline, struct AutoExposureUniform, enum AutoExposurePass, fn init_auto_exposure_pipeline
- `third_party/bevy/crates/bevy_post_process/src/auto_exposure/settings.rs`: struct AutoExposure
- `third_party/bevy/crates/bevy_post_process/src/bloom/downsampling_pipeline.rs`: struct BloomDownsamplingPipelineIds, struct BloomDownsamplingPipeline, struct BloomDownsamplingPipelineKeys, struct BloomUniforms, fn init_bloom_downsampling_pipeline, fn prepare_downsampling_pipeline
- `third_party/bevy/crates/bevy_post_process/src/bloom/mod.rs`: struct BloomPlugin, fn bloom, struct BloomTexture, struct BloomBindGroups
- `third_party/bevy/crates/bevy_post_process/src/bloom/settings.rs`: struct Bloom, struct BloomPrefilter, enum BloomCompositeMode
- `third_party/bevy/crates/bevy_post_process/src/bloom/upsampling_pipeline.rs`: struct UpsamplingPipelineIds, struct BloomUpsamplingPipeline, struct BloomUpsamplingPipelineKeys, fn init_bloom_upscaling_pipeline, fn prepare_upsampling_pipeline
- `third_party/bevy/crates/bevy_post_process/src/dof/mod.rs`: struct DepthOfFieldPlugin, struct DepthOfField, enum DepthOfFieldMode, struct DepthOfFieldUniform, struct DepthOfFieldPipelineKey, struct DepthOfFieldGlobalBindGroupLayout, struct DepthOfFieldGlobalBindGroup, enum DepthOfFieldPipelines, struct AuxiliaryDepthOfFieldTexture, struct ViewDepthOfFieldBindGroupLayouts, struct DepthOfFieldPipeline, fn init_dof_global_bind_group_layout, fn prepare_depth_of_field_view_bind_group_layouts, fn configure_depth_of_field_view_targets, fn prepare_depth_of_field_global_bind_group, fn prepare_auxiliary_depth_of_field_textures, fn prepare_depth_of_field_pipelines, fn calculate_focal_length
- `third_party/bevy/crates/bevy_post_process/src/effect_stack/chromatic_aberration.rs`: struct ChromaticAberration, struct ChromaticAberrationUniform
- `third_party/bevy/crates/bevy_post_process/src/effect_stack/lens_distortion.rs`: struct LensDistortion, struct LensDistortionUniform
- `third_party/bevy/crates/bevy_post_process/src/effect_stack/mod.rs`: struct EffectStackPlugin, struct PostProcessingPipeline, struct PostProcessingPipelineKey, struct PostProcessingPipelineId, struct PostProcessingUniformBuffers, struct PostProcessingUniformBufferOffsets
- `third_party/bevy/crates/bevy_post_process/src/effect_stack/vignette.rs`: struct Vignette, struct VignetteUniform
- `third_party/bevy/crates/bevy_post_process/src/lib.rs`: mod auto_exposure, mod bloom, mod dof, mod effect_stack, mod motion_blur, mod msaa_writeback, struct PostProcessPlugin
- `third_party/bevy/crates/bevy_post_process/src/motion_blur/mod.rs`: mod pipeline, struct MotionBlur, struct MotionBlurUniform, struct MotionBlurPlugin, fn motion_blur
- `third_party/bevy/crates/bevy_post_process/src/motion_blur/pipeline.rs`: struct MotionBlurPipeline, fn init_motion_blur_pipeline, struct MotionBlurPipelineKey, struct MotionBlurPipelineId
- `third_party/bevy/crates/bevy_post_process/src/msaa_writeback.rs`: struct MsaaWritebackPlugin, struct MsaaWritebackBlitPipeline

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
