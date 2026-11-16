---
name: bevy-gltf
description: "Use when working with the `bevy_gltf` crate of bevy: Bevy Engine GLTF loading."
---

# bevy_gltf

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_gltf/` (manifest `third_party/bevy/crates/bevy_gltf/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_gltf
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Bevy Engine GLTF loading

## README (`third_party/bevy/crates/bevy_gltf/README.md`)

# Bevy glTF

## Cargo features (from the manifest)

- `pbr_transmission_textures`
- `pbr_multi_layer_material_textures`
- `pbr_anisotropy_texture`
- `pbr_specular_textures`
- `meshopt` (enables 1): Decodes KHR_meshopt_compression and EXT_meshopt_compression buffer views while loading

## Depends on (same ecosystem)

`bevy_animation`, `bevy_app`, `bevy_asset`, `bevy_camera`, `bevy_color`, `bevy_curve`, `bevy_ecs`, `bevy_image`, `bevy_light`, `bevy_material`, `bevy_math`, `bevy_mesh`, `bevy_platform`, `bevy_reflect`, `bevy_tasks`, `bevy_transform`, `bevy_world_serialization`

## Public items by file

- `third_party/bevy/crates/bevy_gltf/src/assets.rs`: struct Gltf, struct GltfMesh, struct GltfNode, struct GltfPrimitive, struct GltfSkin, struct GltfExtras, struct GltfSceneExtras, struct GltfSceneName, struct GltfMeshExtras, struct GltfMeshName, struct GltfMaterialExtras, struct GltfMaterialName
- `third_party/bevy/crates/bevy_gltf/src/convert_coordinates.rs`: struct GltfConvertCoordinates
- `third_party/bevy/crates/bevy_gltf/src/label.rs`: enum GltfAssetLabel
- `third_party/bevy/crates/bevy_gltf/src/lib.rs`: mod convert_coordinates, mod vertex_attributes, mod prelude, struct DefaultGltfImageSampler, enum GltfSkinnedMeshBoundsPolicy, struct GltfPlugin
- `third_party/bevy/crates/bevy_gltf/src/loader/extensions/khr_meshopt_compression.rs`: fn is_fallback_buffer, fn decode_buffer_views
- `third_party/bevy/crates/bevy_gltf/src/loader/extensions/mod.rs`: mod khr_meshopt_compression, struct GltfExtensionHandlers, trait GltfExtensionHandler, trait ErasedGltfExtensionHandler
- `third_party/bevy/crates/bevy_gltf/src/loader/gltf_ext/mesh.rs`: fn primitive_topology
- `third_party/bevy/crates/bevy_gltf/src/loader/gltf_ext/mod.rs`: mod material, mod mesh, mod scene, mod texture
- `third_party/bevy/crates/bevy_gltf/src/loader/mod.rs`: mod extensions, mod gltf_ext, enum GltfError, struct GltfLoader, struct GltfLoaderSettings, struct PrimitiveMorphAttributesIter, struct MorphTargetNames
- `third_party/bevy/crates/bevy_gltf/src/material.rs`: struct GltfMaterial
- `third_party/bevy/crates/bevy_gltf/src/vertex_attributes.rs`: enum AccessFailed, enum ConvertAttributeError, fn convert_attribute

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
