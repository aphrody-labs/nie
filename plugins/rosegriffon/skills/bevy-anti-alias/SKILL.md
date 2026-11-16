---
name: bevy-anti-alias
description: "Use when working with the `bevy_anti_alias` crate of bevy: Provides various anti-aliasing implementations for Bevy Engine."
---

# bevy_anti_alias

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_anti_alias/` (manifest `third_party/bevy/crates/bevy_anti_alias/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_anti_alias
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides various anti-aliasing implementations for Bevy Engine

## README (`third_party/bevy/crates/bevy_anti_alias/README.md`)

# Bevy Anti-Alias

## Cargo features (from the manifest)

- `trace`
- `webgl`
- `webgpu`
- `smaa_luts` (enables 2)
- `dlss` (enables 3)
- `force_disable_dlss` (enables 1)

## Depends on (same ecosystem)

`bevy_app`, `bevy_asset`, `bevy_camera`, `bevy_core_pipeline`, `bevy_derive`, `bevy_diagnostic`, `bevy_ecs`, `bevy_extract`, `bevy_image`, `bevy_math`, `bevy_reflect`, `bevy_render`, `bevy_shader`, `bevy_utils`

## Public items by file

- `third_party/bevy/crates/bevy_anti_alias/src/contrast_adaptive_sharpening/mod.rs`: struct ContrastAdaptiveSharpening, struct DenoiseCas, struct CasUniform, struct CasPlugin, struct CasPipeline, fn init_cas_pipeline, struct CasPipelineKey, struct CasPipelineSpecializer, struct ViewCasPipeline
- `third_party/bevy/crates/bevy_anti_alias/src/contrast_adaptive_sharpening/node.rs`: fn cas
- `third_party/bevy/crates/bevy_anti_alias/src/dlss/extract.rs`: fn extract_dlss
- `third_party/bevy/crates/bevy_anti_alias/src/dlss/mod.rs`: struct DlssInitPlugin, struct DlssPlugin, struct Dlss, trait DlssFeature, struct DlssSuperResolutionFeature, struct DlssRayReconstructionFeature, struct ViewDlssRayReconstructionTextures, struct DlssProjectId, struct DlssSuperResolutionSupported, struct DlssRayReconstructionSupported
- `third_party/bevy/crates/bevy_anti_alias/src/dlss/node.rs`: fn dlss_super_resolution, fn dlss_ray_reconstruction
- `third_party/bevy/crates/bevy_anti_alias/src/dlss/prepare.rs`: struct DlssRenderContext, fn prepare_dlss
- `third_party/bevy/crates/bevy_anti_alias/src/fxaa/mod.rs`: enum Sensitivity, struct Fxaa, struct FxaaPlugin, struct FxaaPipeline, fn init_fxaa_pipeline, struct CameraFxaaPipeline, struct FxaaPipelineKey, fn prepare_fxaa_pipelines
- `third_party/bevy/crates/bevy_anti_alias/src/fxaa/node.rs`: fn fxaa
- `third_party/bevy/crates/bevy_anti_alias/src/lib.rs`: mod contrast_adaptive_sharpening, mod dlss, mod fxaa, mod smaa, mod taa, struct AntiAliasPlugin
- `third_party/bevy/crates/bevy_anti_alias/src/smaa/mod.rs`: struct SmaaPlugin, struct Smaa, enum SmaaPreset, struct SmaaPipelines, struct SmaaNeighborhoodBlendingPipelineKey, struct ViewSmaaPipelines, struct SmaaInfoUniform, struct SmaaInfoUniformOffset, struct SmaaInfoUniformBuffer, struct SmaaTextures, struct SmaaBindGroups, struct SmaaSpecializedRenderPipelines, fn init_smaa_pipelines, fn smaa
- `third_party/bevy/crates/bevy_anti_alias/src/taa/mod.rs`: struct TemporalAntiAliasPlugin, struct TemporalAntiAliasing, fn temporal_anti_alias, struct TaaPipeline, struct TemporalAntiAliasHistoryTextures, struct TemporalAntiAliasPipelineId

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
