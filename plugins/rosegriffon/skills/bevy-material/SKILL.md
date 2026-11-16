---
name: bevy-material
description: "Use when working with the `bevy_material` crate of bevy: Provides a material abstraction for Bevy Engine."
---

# bevy_material

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_material/` (manifest `third_party/bevy/crates/bevy_material/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_material
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides a material abstraction for Bevy Engine

## README (`third_party/bevy/crates/bevy_material/README.md`)

# Bevy Material

## Depends on (same ecosystem)

`bevy_asset`, `bevy_derive`, `bevy_ecs`, `bevy_material_macros`, `bevy_mesh`, `bevy_platform`, `bevy_reflect`, `bevy_shader`, `bevy_utils`

## Public items by file

- `third_party/bevy/crates/bevy_material/src/alpha.rs`: enum AlphaMode
- `third_party/bevy/crates/bevy_material/src/bind_group_layout_entries.rs`: struct BindGroupLayoutEntryBuilder, struct BindGroupLayoutEntries, trait IntoBindGroupLayoutEntryBuilder, trait IntoBindGroupLayoutEntryBuilderArray, trait IntoIndexedBindGroupLayoutEntryBuilderArray, struct DynamicBindGroupLayoutEntries, mod binding_types
- `third_party/bevy/crates/bevy_material/src/descriptor.rs`: struct BindGroupLayoutDescriptor, struct RenderPipelineDescriptor, struct NoFragmentStateError, struct VertexState, struct FragmentState, struct ComputePipelineDescriptor, enum PipelineDescriptor, struct MeshPipelineDescriptor, struct TaskState, struct MeshState, struct CachedRenderPipelineId, struct CachedComputePipelineId
- `third_party/bevy/crates/bevy_material/src/key.rs`: struct ErasedMeshPipelineKey, struct ErasedMaterialPipelineKey, struct ErasedMaterialKey, struct ErasedMaterialKeyVTable
- `third_party/bevy/crates/bevy_material/src/labels.rs`: type InternedShaderLabel, type InternedDrawFunctionLabel, struct DrawFunctionId
- `third_party/bevy/crates/bevy_material/src/lib.rs`: mod bind_group_layout_entries, mod descriptor, mod key, mod labels, mod specialize, mod prelude, struct MaterialProperties
- `third_party/bevy/crates/bevy_material/src/opaque.rs`: enum OpaqueRendererMethod
- `third_party/bevy/crates/bevy_material/src/phase.rs`: enum RenderPhaseType
- `third_party/bevy/crates/bevy_material/src/specialize.rs`: type BaseSpecializeFn, type PrepassSpecializeFn, type UserSpecializeFn, enum SpecializedMeshPipelineError

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
