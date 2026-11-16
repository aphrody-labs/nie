---
name: bevy-render
description: "Use when working with the `bevy_render` crate of bevy: Provides rendering functionality for Bevy Engine."
---

# bevy_render

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_render/` (manifest `third_party/bevy/crates/bevy_render/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_render
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides rendering functionality for Bevy Engine

## README (`third_party/bevy/crates/bevy_render/README.md`)

# Bevy Render

## Cargo features (from the manifest)

- `multi_threaded` (enables 2)
- `morph` (enables 1)
- `shader_format_spirv` (enables 1)
- `spirv_shader_passthrough` (enables 1): Enable SPIR-V shader passthrough
- `statically-linked-dxc` (enables 1): Statically linked DXC shader compiler for DirectX 12 TODO: When wgpu switches to DirectX 12 instead of Vulkan by default on windows, make this a default feature
- `raw_vulkan_init` (enables 1): Forces the wgpu instance to be initialized using the raw Vulkan HAL, enabling additional configuration
- `trace` (enables 1)
- `tracing-tracy` (enables 1)
- `ci_limits`
- `webgl` (enables 1)
- `webgpu` (enables 1)
- `vulkan-portability` (enables 1)
- `gles` (enables 1)
- `detailed_trace`
- `type_label_buffers`: Pre-populate buffer labels with buffer types for debugging.
- `debug` (enables 2): Enables collecting extra information for debugging.
- `serialize` (enables 1): Adds serialization support through `serde`.
- `reflect_auto_register` (enables 1)
- `reflect_functions` (enables 1)
- `framepace_debug`

## Depends on (same ecosystem)

`bevy_app`, `bevy_asset`, `bevy_camera`, `bevy_color`, `bevy_derive`, `bevy_diagnostic`, `bevy_ecs`, `bevy_encase_derive`, `bevy_extract`, `bevy_extract_macros`, `bevy_image`, `bevy_log`, `bevy_material`, `bevy_material_macros`, `bevy_math`, `bevy_mesh`, `bevy_platform`, `bevy_reflect`, `bevy_render_macros`, `bevy_shader`, `bevy_shape`, `bevy_tasks`, `bevy_time`, `bevy_transform`, `bevy_utils`, `bevy_window`

## Public items by file

- `third_party/bevy/crates/bevy_render/src/batching/gpu_preprocessing.rs`: struct BatchingPlugin, struct GpuPreprocessingSupport, enum GpuPreprocessingMode, struct BatchedInstanceBuffers, trait BufferDataInput, struct PhaseBatchedInstanceBuffers, struct UntypedPhaseBatchedInstanceBuffers, struct InstanceInputUniformBuffer, struct PreviousInstanceInputUniformBuffer, enum PreprocessWorkItemBuffers, struct GpuOcclusionCullingWorkItemBuffers, struct LatePreprocessWorkItemIndirectParameters, fn get_or_create_work_item_buffer, fn init_work_item_buffers, struct PreprocessWorkItem, struct IndirectParametersIndexed, struct IndirectParametersNonIndexed, struct IndirectParametersBuildJob, struct IndirectParametersMetadata, struct IndirectBatchSet, struct IndirectParametersBuffers, struct IndirectParametersBuffersSettings, struct IndirectParametersBuildJobs, struct GpuBinUnpackingMetadata, ...
- `third_party/bevy/crates/bevy_render/src/batching/mod.rs`: mod gpu_preprocessing, mod no_gpu_preprocessing, struct NoAutomaticBatching, trait GetBatchData, trait GetFullBatchData, fn sort_binned_render_phase
- `third_party/bevy/crates/bevy_render/src/batching/no_gpu_preprocessing.rs`: struct BatchedInstanceBuffer, fn clear_batched_cpu_instance_buffers, fn batch_and_prepare_sorted_render_phase, fn batch_and_prepare_binned_render_phase, fn write_batched_instance_buffer
- `third_party/bevy/crates/bevy_render/src/camera.rs`: struct CameraMainPassTextureFormats, struct CameraPlugin, struct CameraRenderGraph, trait NormalizedRenderTargetExt, enum MissingRenderTargetInfoError, fn camera_system, struct ExtractedCamera, fn extract_cameras, struct SortedCameras, struct SortedCamera, fn sort_cameras, struct TemporalJitter, struct MipBias, struct DirtySpecializations, struct DirtyWireframeSpecializations, fn clear_dirty_specializations, fn clear_dirty_wireframe_specializations, fn expire_specializations_for_views, fn expire_wireframe_specializations_for_views, enum DirtySpecializationSystems, struct PendingQueues, struct ViewPendingQueues
- `third_party/bevy/crates/bevy_render/src/combined_bind_group.rs`: fn bindless_slot_count, fn bind_group_data, fn build_bind_group, fn bind_group_layout_entries, fn combine_bind_group_layout_entries, fn bindless_descriptor, fn combine_bindless_descriptors, struct CombinedBindGroupData
- `third_party/bevy/crates/bevy_render/src/diagnostic/erased_render_asset_diagnostic_plugin.rs`: struct ErasedRenderAssetDiagnosticPlugin
- `third_party/bevy/crates/bevy_render/src/diagnostic/internal.rs`: struct DiagnosticsRecorder, struct RenderDiagnostics, struct RenderDiagnostic, struct RenderDiagnosticsMutex, fn sync_diagnostics, trait WriteTimestamp, trait WritePipelineStatistics, trait Pass, enum PassKind
- `third_party/bevy/crates/bevy_render/src/diagnostic/mesh_allocator_diagnostic_plugin.rs`: struct MeshAllocatorDiagnosticPlugin
- `third_party/bevy/crates/bevy_render/src/diagnostic/mod.rs`: struct RenderDiagnosticsPlugin, fn begin_diagnostics_frame, fn resolve_encoder, trait RecordDiagnostics, struct TimeSpanGuard, struct PassSpanGuard
- `third_party/bevy/crates/bevy_render/src/diagnostic/render_asset_diagnostic_plugin.rs`: struct RenderAssetDiagnosticPlugin
- `third_party/bevy/crates/bevy_render/src/diagnostic/tracy_gpu.rs`: fn new_tracy_gpu_context
- `third_party/bevy/crates/bevy_render/src/erased_render_asset.rs`: enum PrepareAssetError, struct AssetExtractionSystems, trait ErasedRenderAsset, struct ErasedRenderAssetPlugin, trait ErasedRenderAssetDependency, struct ExtractedAssets, struct ErasedRenderAssets, struct PrepareNextFrameAssets, fn prepare_erased_assets
- `third_party/bevy/crates/bevy_render/src/error_handler.rs`: enum RenderErrorPolicy, struct RenderErrorHandler, struct RenderError
- `third_party/bevy/crates/bevy_render/src/framepace/debug.rs`: struct FramepaceDiagnosticsPlugin
- `third_party/bevy/crates/bevy_render/src/framepace/mod.rs`: mod debug, struct FramepacePlugin, struct FramepaceSettings, enum Limiter, struct FrametimeLimit, struct FrameTimer, struct FramePaceStats
- `third_party/bevy/crates/bevy_render/src/globals.rs`: struct GlobalsPlugin, struct GlobalsUniform, struct GlobalsBuffer
- `third_party/bevy/crates/bevy_render/src/gpu_component_array_buffer.rs`: struct GpuComponentArrayBufferPlugin, trait GpuComponentArrayBuffer, struct GpuComponentArray
- `third_party/bevy/crates/bevy_render/src/gpu_readback.rs`: struct GpuReadbackPlugin, enum Readback, struct ReadbackOnce, struct ReadbackComplete
- `third_party/bevy/crates/bevy_render/src/lib.rs`: mod batching, mod camera, mod combined_bind_group, mod diagnostic, mod erased_render_asset, mod error_handler, mod extract_component, mod extract_plugin, mod extract_resource, mod framepace, mod globals, mod gpu_component_array_buffer, mod gpu_readback, mod material_bind_groups, mod mesh, mod occlusion_culling, mod pipelined_rendering, mod render_asset, mod render_phase, mod render_resource, mod renderer, mod settings, mod slab_allocator, mod storage, ...
- `third_party/bevy/crates/bevy_render/src/material_bind_groups.rs`: struct MaterialBindGroupPlugin, struct RenderMaterialBindings, struct MaterialBindGroupAllocators, enum MaterialBindGroupAllocator, struct MaterialBindGroupBindlessAllocator, struct MaterialBindlessSlab, struct MaterialBindGroupNonBindlessAllocator, struct FallbackBindlessResources, struct FallbackBuffer, struct MaterialBindingId, struct MaterialBindGroupIndex, struct MaterialBindGroupSlot, struct MaterialSlab, fn material_uses_bindless_resources, fn prepare_material_bind_groups, fn write_material_bind_group_buffers
- `third_party/bevy/crates/bevy_render/src/mesh/allocator.rs`: struct MeshAllocatorPlugin, struct MeshAllocator, struct MeshAllocatorSettings, type MeshSlabId, type MeshBufferSlice, struct MeshSlabItem, struct MeshSlabs, struct MeshAllocationKey, enum ElementClass, struct ElementLayout, fn allocate_and_free_meshes
- `third_party/bevy/crates/bevy_render/src/mesh/mod.rs`: mod allocator, mod morph, struct MeshRenderAssetPlugin, struct MeshMetadataFallbackMesh, struct MeshMetadataFallbackBuffer, fn prepare_mesh_metadata_fallback_buffer, struct MeshMetadata, struct RenderMesh, enum RenderMeshBufferInfo
- `third_party/bevy/crates/bevy_render/src/mesh/morph.rs`: struct MorphTargetImage, enum RenderMorphTargetAllocator, enum MorphTargetsResource
- `third_party/bevy/crates/bevy_render/src/occlusion_culling/mod.rs`: struct OcclusionCullingPlugin, struct OcclusionCulling, struct OcclusionCullingSubview, struct OcclusionCullingSubviewEntities
- `third_party/bevy/crates/bevy_render/src/pipelined_rendering.rs`: struct RenderExtractApp, struct RenderAppChannels, struct PipelinedRenderingPlugin
- `third_party/bevy/crates/bevy_render/src/render_asset.rs`: enum PrepareAssetError, struct AssetExtractionSystems, enum AssetExtractionError, trait RenderAsset, struct RenderAssetPlugin, trait RenderAssetDependency, struct ExtractedAssets, struct RenderAssets, struct PrepareNextFrameAssets, fn prepare_assets, fn reset_render_asset_bytes_per_frame, fn extract_render_asset_bytes_per_frame, struct RenderAssetBytesPerFrame, struct RenderAssetBytesPerFrameLimiter
- `third_party/bevy/crates/bevy_render/src/render_phase/draw.rs`: trait Draw, enum DrawError, struct DrawFunctionsInternal, struct DrawFunctions, trait RenderCommand, enum RenderCommandResult, struct RenderCommandState, trait AddRenderCommand
- `third_party/bevy/crates/bevy_render/src/render_phase/draw_state.rs`: struct TrackedRenderPass
- `third_party/bevy/crates/bevy_render/src/render_phase/mod.rs`: struct ViewBinnedRenderPhases, struct BinnedRenderPhase, struct RenderBin, struct RenderMultidrawableBin, struct RenderMultidrawableBatchSetGpuBuffers, struct RenderMultidrawableBatchSet, struct GpuRenderBinnedMeshInstance, struct CachedBinnedEntity, struct CachedBinKey, enum BinnedRenderPhaseBatchSets, struct BinnedRenderPhaseBatchSet, struct BinnedRenderPhaseBatch, struct UnbatchableBinnedEntities, struct NonMeshEntities, enum BinnedRenderPhaseType, struct InputUniformIndex, struct BinnedRenderPhasePlugin, struct ViewSortedRenderPhases, struct SortedRenderPhasePlugin, struct SortedRenderPhase, trait PhaseItem, enum PhaseItemExtraIndex, trait BinnedPhaseItem, trait PhaseItemBatchSetKey, ...
- `third_party/bevy/crates/bevy_render/src/render_phase/rangefinder.rs`: struct ViewRangefinder3d
- `third_party/bevy/crates/bevy_render/src/render_resource/atomic_pod.rs`: trait AtomicPod, trait AtomicPodBlob, macro impl_atomic_pod!
- `third_party/bevy/crates/bevy_render/src/render_resource/batched_uniform_buffer.rs`: struct BatchedUniformBuffer
- `third_party/bevy/crates/bevy_render/src/render_resource/bind_group.rs`: struct BindGroup, trait AsBindGroup, enum AsBindGroupError, struct PreparedBindGroup, struct BindGroupBuilder, struct BindingResources, struct UnpreparedBindingResources, enum OwnedBindingResource, enum UnpreparedBindingResource, struct OwnedData, trait AsBindGroupShaderType
- `third_party/bevy/crates/bevy_render/src/render_resource/bind_group_entries.rs`: struct BindGroupEntries, trait IntoBinding, trait IntoBindingArray, trait IntoIndexedBindingArray, struct DynamicBindGroupEntries
- `third_party/bevy/crates/bevy_render/src/render_resource/bind_group_layout.rs`: struct BindGroupLayout
- `third_party/bevy/crates/bevy_render/src/render_resource/bindless.rs`: enum BindlessSlabResourceLimit, struct BindlessDescriptor, enum BindlessResourceType, struct BindlessBufferDescriptor, struct BindlessIndexTableDescriptor, struct BindingNumber, struct BindlessIndex, fn create_bindless_bind_group_layout_entries
- `third_party/bevy/crates/bevy_render/src/render_resource/buffer.rs`: struct Buffer, struct BufferSlice
- `third_party/bevy/crates/bevy_render/src/render_resource/buffer_vec.rs`: struct RawBufferVec, struct AtomicRawBufferVec, struct BufferVec, struct UninitBufferVec, struct PartialBufferVec, enum WriteBufferRangeError
- `third_party/bevy/crates/bevy_render/src/render_resource/gpu_array_buffer.rs`: trait GpuArrayBufferable, enum GpuArrayBuffer, struct GpuArrayBufferIndex
- `third_party/bevy/crates/bevy_render/src/render_resource/mod.rs`: mod encase
- (30 more files)

## Examples in the source tree

- `third_party/bevy/examples/2d/mesh2d_manual.rs`
- `third_party/bevy/examples/3d/order_independent_transparency.rs`
- `third_party/bevy/examples/shader_advanced/custom_phase_item.rs`

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
