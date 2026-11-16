---
name: bevy-mesh
description: "Use when working with the `bevy_mesh` crate of bevy: Provides mesh types for Bevy Engine."
---

# bevy_mesh

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_mesh/` (manifest `third_party/bevy/crates/bevy_mesh/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_mesh
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides mesh types for Bevy Engine

## Cargo features (from the manifest)

- `default`
- `serialize` (enables 3): Adds serialization support through `serde`.
- `morph` (enables 1)

## Depends on (same ecosystem)

`bevy_app`, `bevy_asset`, `bevy_derive`, `bevy_ecs`, `bevy_math`, `bevy_mikktspace`, `bevy_platform`, `bevy_reflect`, `bevy_shape`, `bevy_transform`

## Public items by file

- `third_party/bevy/crates/bevy_mesh/src/components.rs`: struct Mesh2d, struct Mesh3d, fn mark_2d_meshes_as_changed_if_their_assets_changed, fn mark_3d_meshes_as_changed_if_their_assets_changed, struct MeshTag
- `third_party/bevy/crates/bevy_mesh/src/conversions.rs`: struct FromVertexAttributeError
- `third_party/bevy/crates/bevy_mesh/src/index.rs`: enum MeshWindingInvertError, enum MeshTrianglesError, enum Indices
- `third_party/bevy/crates/bevy_mesh/src/lib.rs`: mod morph, mod primitives, mod skinning, mod prelude, struct MeshPlugin
- `third_party/bevy/crates/bevy_mesh/src/mesh.rs`: enum MeshAccessError, struct Mesh, struct MeshCompressionArgs, enum UvChannel, struct SerializedMesh, struct MeshDeserializer, enum MeshAttributeCompressionError, enum MeshMergeDuplicateVerticesError, enum MeshMergeError
- `third_party/bevy/crates/bevy_mesh/src/mikktspace.rs`: enum GenerateTangentsError
- `third_party/bevy/crates/bevy_mesh/src/morph.rs`: enum MorphBuildError, struct MorphWeights, enum MeshMorphWeights, struct MorphAttributes
- `third_party/bevy/crates/bevy_mesh/src/primitives/dim2.rs`: struct CircleMeshBuilder, enum CircularMeshUvMode, struct CircularSectorMeshBuilder, struct CircularSegmentMeshBuilder, struct ConvexPolygonMeshBuilder, struct RegularPolygonMeshBuilder, struct EllipseMeshBuilder, struct Segment2dMeshBuilder, struct Polyline2dMeshBuilder, struct AnnulusMeshBuilder, struct RhombusMeshBuilder, struct Triangle2dMeshBuilder, struct RectangleMeshBuilder, struct Capsule2dMeshBuilder, struct RingMeshBuilder
- `third_party/bevy/crates/bevy_mesh/src/primitives/dim3/capsule.rs`: enum CapsuleUvProfile, struct Capsule3dMeshBuilder
- `third_party/bevy/crates/bevy_mesh/src/primitives/dim3/cone.rs`: enum ConeAnchor, struct ConeMeshBuilder
- `third_party/bevy/crates/bevy_mesh/src/primitives/dim3/conical_frustum.rs`: struct ConicalFrustumMeshBuilder
- `third_party/bevy/crates/bevy_mesh/src/primitives/dim3/cuboid.rs`: struct CuboidMeshBuilder
- `third_party/bevy/crates/bevy_mesh/src/primitives/dim3/cylinder.rs`: enum CylinderAnchor, struct CylinderMeshBuilder
- `third_party/bevy/crates/bevy_mesh/src/primitives/dim3/plane.rs`: struct PlaneMeshBuilder
- `third_party/bevy/crates/bevy_mesh/src/primitives/dim3/polyline3d.rs`: struct Polyline3dMeshBuilder
- `third_party/bevy/crates/bevy_mesh/src/primitives/dim3/segment3d.rs`: struct Segment3dMeshBuilder
- `third_party/bevy/crates/bevy_mesh/src/primitives/dim3/sphere.rs`: enum IcosphereError, enum SphereKind, struct SphereMeshBuilder
- `third_party/bevy/crates/bevy_mesh/src/primitives/dim3/tetrahedron.rs`: struct TetrahedronMeshBuilder
- `third_party/bevy/crates/bevy_mesh/src/primitives/dim3/torus.rs`: struct TorusMeshBuilder
- `third_party/bevy/crates/bevy_mesh/src/primitives/dim3/triangle3d.rs`: struct Triangle3dMeshBuilder
- `third_party/bevy/crates/bevy_mesh/src/primitives/extrusion.rs`: enum PerimeterSegment, trait Extrudable, struct ExtrusionBuilder
- `third_party/bevy/crates/bevy_mesh/src/primitives/mod.rs`: trait Meshable, trait MeshBuilder
- `third_party/bevy/crates/bevy_mesh/src/skinning.rs`: struct SkinnedMesh, struct SkinnedMeshInverseBindposes, struct JointAabb, struct SkinnedMeshBounds, enum SkinnedMeshBoundsError, enum EntityAabbFromSkinnedMeshBoundsError, fn entity_aabb_from_skinned_mesh_bounds, struct JointIndex, struct Influence, struct InfluenceIterator, enum MeshAttributeError
- `third_party/bevy/crates/bevy_mesh/src/vertex.rs`: struct MeshVertexAttribute, struct MeshVertexAttributeId, struct MeshVertexBufferLayout, struct MissingVertexAttributeError, struct VertexAttributeDescriptor, fn triangle_area_normal, fn triangle_normal, enum AttributeQuantization, enum VertexAttributeValues, struct VertexBufferLayout, struct MeshVertexBufferLayoutRef, struct MeshVertexBufferLayouts, fn octahedral_encode_signed, fn octahedral_encode_tangent, fn octahedral_decode_signed, fn octahedral_decode_tangent

## Examples in the source tree

- `third_party/bevy/examples/3d/meshlet.rs`
- `third_party/bevy/examples/shader/gpu_component_array_buffer.rs`

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
