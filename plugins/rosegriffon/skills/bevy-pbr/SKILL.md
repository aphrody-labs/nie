---
name: bevy-pbr
description: "Use when working with the `bevy_pbr` crate of bevy: Adds PBR rendering to Bevy Engine."
---

# bevy_pbr

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_pbr/` (manifest `third_party/bevy/crates/bevy_pbr/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_pbr
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Adds PBR rendering to Bevy Engine

## README (`third_party/bevy/crates/bevy_pbr/README.md`)

# Bevy PBR

## Cargo features (from the manifest)

- `webgl` (enables 1)
- `webgpu` (enables 1)
- `pbr_transmission_textures` (enables 1)
- `pbr_multi_layer_material_textures` (enables 1)
- `pbr_anisotropy_texture` (enables 1)
- `pbr_specular_textures` (enables 1)
- `experimental_pbr_pcss` (enables 1)
- `pbr_clustered_decals`
- `pbr_light_textures`
- `bluenoise_texture` (enables 2)
- `dfg_lut` (enables 2)
- `area_light_luts` (enables 2)
- `trace` (enables 1)
- `meshlet` (enables 2): Enables the meshlet renderer for dense high-poly scenes (experimental)
- `meshlet_processor` (enables 5): Enables processing meshes into meshlet meshes

## Depends on (same ecosystem)

`bevy_app`, `bevy_asset`, `bevy_camera`, `bevy_color`, `bevy_core_pipeline`, `bevy_derive`, `bevy_diagnostic`, `bevy_ecs`, `bevy_extract`, `bevy_gltf`, `bevy_image`, `bevy_light`, `bevy_log`, `bevy_material`, `bevy_math`, `bevy_mesh`, `bevy_platform`, `bevy_reflect`, `bevy_render`, `bevy_shader`, `bevy_shape`, `bevy_tasks`, `bevy_transform`, `bevy_utils`

## Public items by file

- `third_party/bevy/crates/bevy_pbr/src/atmosphere/environment.rs`: struct AtmosphereEnvironmentMap, struct AtmosphereProbeTextures, struct AtmosphereProbeBindGroups, struct AtmosphereProbeLayouts, struct AtmosphereProbePipeline, fn init_atmosphere_probe_layout, fn init_atmosphere_probe_pipeline, fn validate_environment_map_size, fn on_insert_atmosphere_environment_map_light, fn on_remove_atmosphere_environment_map_light, fn atmosphere_environment
- `third_party/bevy/crates/bevy_pbr/src/atmosphere/mod.rs`: mod resources, struct AtmospherePlugin, fn extract_atmosphere, struct ExtractedAtmosphere, struct AtmosphereSettings, struct GpuAtmosphereSettings, enum AtmosphereMode
- `third_party/bevy/crates/bevy_pbr/src/atmosphere/node.rs`: fn atmosphere_luts, fn render_sky
- `third_party/bevy/crates/bevy_pbr/src/atmosphere/resources.rs`: struct AtmosphereTextures, struct GpuAtmosphere, fn prepare_atmosphere_uniforms, struct AtmosphereTransforms, struct AtmosphereTransform, struct AtmosphereTransformsOffset, struct AtmosphereBuffer
- `third_party/bevy/crates/bevy_pbr/src/cluster/gpu.rs`: struct GpuClusteringPlugin, struct ViewClusteringDummyTexture, struct ViewClusteringBindGroups, struct ClusterableObjectZSlice, struct ClusterMetadata, struct ClusterRasterIndirectDrawParams, struct ViewGpuClusteringBuffers, struct ClusteringRasterPipeline, struct ClusteringZSlicingPipeline, struct ClusteringAllocationPipeline, struct ClusteringRasterPipelineKey, struct ClusteringAllocationPipelineKey, struct ViewGpuClusteringPipelineIds, fn extract_clusters_for_gpu_clustering
- `third_party/bevy/crates/bevy_pbr/src/cluster/mod.rs`: struct GpuClusteredLight, struct GlobalClusterableObjectMeta, struct GpuClusteredLights, struct ExtractedClusterConfig, struct ExtractedClusterableObjects, struct ViewClusterBindings, fn init_global_clusterable_object_meta, fn gpu_clustering_is_enabled_during_extraction, fn gpu_clustering_is_enabled, fn extract_clusters_for_cpu_clustering, fn prepare_clusters_for_cpu_clustering
- `third_party/bevy/crates/bevy_pbr/src/contact_shadows.rs`: struct ContactShadowsPlugin, struct ContactShadows, struct ContactShadowsUniform, struct ContactShadowsBuffer, struct ViewContactShadowsUniformOffset
- `third_party/bevy/crates/bevy_pbr/src/decal/clustered.rs`: struct ClusteredDecalPlugin, struct RenderClusteredDecals, struct DecalsBuffer, struct RenderClusteredDecal, fn extract_decals, fn clustered_decals_are_usable
- `third_party/bevy/crates/bevy_pbr/src/decal/forward.rs`: struct ForwardDecalPlugin, struct ForwardDecal, type ForwardDecalMaterial, struct ForwardDecalMaterialExt, struct ForwardDecalMaterialExtUniform
- `third_party/bevy/crates/bevy_pbr/src/decal/mod.rs`: mod clustered
- `third_party/bevy/crates/bevy_pbr/src/deferred/mod.rs`: struct DeferredPbrLightingPlugin, struct PbrDeferredLightingDepthId, fn deferred_lighting, struct DeferredLightingLayout, struct DeferredLightingPipeline, fn init_deferred_lighting_layout, fn insert_deferred_lighting_pass_id_component, fn prepare_deferred_lighting_pipelines, struct SkipDeferredLighting
- `third_party/bevy/crates/bevy_pbr/src/diagnostic.rs`: struct MaterialAllocatorDiagnosticPlugin
- `third_party/bevy/crates/bevy_pbr/src/extended_material.rs`: struct MaterialExtensionPipeline, struct MaterialExtensionKey, trait MaterialExtension, struct ExtendedMaterial
- `third_party/bevy/crates/bevy_pbr/src/fog.rs`: struct DistanceFog, enum FogFalloff
- `third_party/bevy/crates/bevy_pbr/src/gltf.rs`: fn standard_material_from_gltf_material
- `third_party/bevy/crates/bevy_pbr/src/lib.rs`: mod wireframe, mod experimental, mod contact_shadows, mod gltf, mod decal, mod deferred, mod diagnostic, mod prelude, struct PbrPlugin, struct Bluenoise, struct AreaLightLuts, struct DfgLut, fn stbn_placeholder, fn area_light_luts_placeholder
- `third_party/bevy/crates/bevy_pbr/src/light_probe/environment_map.rs`: struct EnvironmentMapIds, struct EnvironmentMapViewLightProbeInfo
- `third_party/bevy/crates/bevy_pbr/src/light_probe/generate.rs`: struct GeneratorBindGroupLayouts, struct GeneratorSamplers, struct GeneratorPipelines, struct DownsamplingConfig, struct EnvironmentMapGenerationPlugin, fn initialize_generated_environment_map_resources, fn extract_generated_environment_map_entities, struct RenderEnvironmentMap, struct IntermediateTextures, fn prepare_generated_environment_map_intermediate_textures, struct FilteringConstants, struct GeneratorBindGroups, fn prepare_generated_environment_map_bind_groups, fn downsampling_system, fn filtering_system, fn generate_environment_map_light
- `third_party/bevy/crates/bevy_pbr/src/light_probe/mod.rs`: mod environment_map, mod generate, mod irradiance_volume, struct LightProbePlugin, struct LightProbesUniform, struct LightProbesBuffer, struct ViewLightProbesUniformOffset, struct RenderViewLightProbes, trait LightProbeComponent
- `third_party/bevy/crates/bevy_pbr/src/lightmap/mod.rs`: struct LightmapPlugin, struct Lightmap, struct RenderLightmaps, struct LightmapSlab, struct LightmapSlabIndex, struct LightmapSlotIndex, fn init_render_lightmaps
- `third_party/bevy/crates/bevy_pbr/src/material.rs`: trait Material, struct MaterialShaders, struct MaterialsPlugin, struct MaterialPlugin, struct MaterialPipelineKey, struct MaterialPipeline, struct MaterialPipelineSpecializer, fn init_material_pipeline, type DrawMaterial, struct SetMaterialBindGroup, struct RenderMaterialInstances, struct RenderMaterialInstance, struct MaterialExtractionSystems, fn alpha_mode_pipeline_key, fn tonemapping_pipeline_key, fn late_sweep_material_instances, fn extract_entities_needs_specialization, fn extract_entities_that_need_specializations_removed, struct EntitiesNeedingSpecialization, struct SpecializedMaterialPipelineCache, struct SpecializedMaterialViewPipelineCache, fn check_entities_needing_specialization, struct PendingMeshMaterialQueues, fn queue_material_meshes, ...
- `third_party/bevy/crates/bevy_pbr/src/medium.rs`: struct ScatteringMediumPlugin, struct GpuScatteringMedium, struct ScatteringMediumSampler
- `third_party/bevy/crates/bevy_pbr/src/mesh_material.rs`: struct MeshMaterial3d
- `third_party/bevy/crates/bevy_pbr/src/meshlet/asset.rs`: struct MeshletMesh, struct BvhNode, struct Meshlet, struct MeshletCullData, struct MeshletAabb, struct MeshletAabbErrorOffset, struct MeshletBoundingSphere, struct MeshletMeshSaver, struct MeshletMeshLoader, enum MeshletMeshSaveOrLoadError
- `third_party/bevy/crates/bevy_pbr/src/meshlet/from_mesh.rs`: enum MeshToMeshletMeshConversionError
- `third_party/bevy/crates/bevy_pbr/src/meshlet/instance_manager.rs`: struct InstanceManager, fn extract_meshlet_mesh_entities, fn queue_material_meshlet_meshes
- `third_party/bevy/crates/bevy_pbr/src/meshlet/material_pipeline_prepare.rs`: struct MeshletViewMaterialsMainOpaquePass, fn prepare_material_meshlet_meshes_main_opaque_pass, struct MeshletViewMaterialsPrepass, struct MeshletViewMaterialsDeferredGBufferPrepass, fn prepare_material_meshlet_meshes_prepass
- `third_party/bevy/crates/bevy_pbr/src/meshlet/material_shade_nodes.rs`: fn meshlet_main_opaque_pass, fn meshlet_prepass, fn meshlet_deferred_gbuffer_prepass
- `third_party/bevy/crates/bevy_pbr/src/meshlet/meshlet_mesh_manager.rs`: struct MeshletMeshManager, fn init_meshlet_mesh_manager, fn perform_pending_meshlet_mesh_writes
- `third_party/bevy/crates/bevy_pbr/src/meshlet/mod.rs`: struct MeshletPlugin, struct MeshletMesh3d
- `third_party/bevy/crates/bevy_pbr/src/meshlet/persistent_buffer.rs`: struct PersistentGpuBuffer, trait PersistentGpuBufferable
- `third_party/bevy/crates/bevy_pbr/src/meshlet/pipelines.rs`: struct MeshletPipelines, fn init_meshlet_pipelines
- `third_party/bevy/crates/bevy_pbr/src/meshlet/resource_manager.rs`: struct ResourceManager, struct MeshletViewResources, struct MeshletViewBindGroups, fn prepare_meshlet_per_frame_resources, fn prepare_meshlet_view_bind_groups
- `third_party/bevy/crates/bevy_pbr/src/meshlet/visibility_buffer_raster_node.rs`: fn meshlet_visibility_buffer_raster
- `third_party/bevy/crates/bevy_pbr/src/parallax.rs`: enum ParallaxMappingMethod
- `third_party/bevy/crates/bevy_pbr/src/pbr_material.rs`: struct StandardMaterial, struct StandardMaterialUniform
- `third_party/bevy/crates/bevy_pbr/src/prepass/mod.rs`: struct PrepassPipelinePlugin, struct PrepassPlugin, fn update_previous_view_data, struct PreviousGlobalTransform, fn update_mesh_previous_global_transforms, struct PrepassPipeline, fn init_prepass_pipeline, struct PrepassPipelineSpecializer, fn extract_camera_previous_view_data, fn prepare_previous_view_uniforms, struct PreviousTemporalJitter, struct PrepassViewBindGroup, fn init_prepass_view_bind_group, fn prepare_prepass_view_bind_group, struct SpecializedPrepassMaterialPipelineCache, struct SpecializedPrepassMaterialViewPipelineCache, struct ViewKeyPrepassCache, fn check_prepass_views_need_specialization, struct PendingPrepassMeshMaterialQueues, fn queue_prepass_material_meshes, struct SetPrepassViewBindGroup, struct SetPrepassViewEmptyBindGroup, struct SetPrepassEmptyMaterialBindGroup, type DrawPrepass, ...
- `third_party/bevy/crates/bevy_pbr/src/prepass/prepass_bindings.rs`: fn get_bind_group_layout_entries, fn get_bindings
- `third_party/bevy/crates/bevy_pbr/src/render/fog.rs`: struct GpuFog, struct FogMeta, fn prepare_fog, struct ViewFogUniformOffset, struct FogPlugin
- `third_party/bevy/crates/bevy_pbr/src/render/gpu_preprocess.rs`: struct GpuMeshPreprocessPlugin, struct PreprocessPipelines, struct PreprocessPhasePipelines, struct PreprocessPipeline, struct ResetIndirectBatchSetsPipeline, struct BuildIndirectParametersPipeline, struct BinUnpackingPipeline, struct UniformAllocationPipelines, struct UniformAllocationLocalScanPipeline, struct UniformAllocationGlobalScanPipeline, struct UniformAllocationFanPipeline, struct PreprocessBindGroups, enum PhasePreprocessBindGroups, struct BuildIndirectParametersBindGroups, struct PhaseBuildIndirectParametersBindGroups, struct BinUnpackingBindGroups, struct ViewPhaseBinUnpackingBindGroups, struct ViewPhaseBinUnpackingBindGroup, struct UniformAllocationBindGroups, struct ViewPhaseUniformAllocationBindGroups, struct ViewPhaseUniformAllocationBindGroup, struct SkipGpuPreprocess, fn allocate_uniforms, fn unpack_bins, ...
- (15 more files)

## Examples in the source tree

- `third_party/bevy/examples/3d/meshlet.rs`

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
