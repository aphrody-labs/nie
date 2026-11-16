---
name: bevy-shape
description: "Use when working with the `bevy_shape` crate of bevy: Provides primitive shape data definitions for Bevy Engine."
---

# bevy_shape

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_shape/` (manifest `third_party/bevy/crates/bevy_shape/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_shape
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides primitive shape data definitions for Bevy Engine

## Crate docs (`third_party/bevy/crates/bevy_shape/src/lib.rs`)

This module defines primitive shapes.
The origin is (0, 0) for 2D primitives and (0, 0, 0) for 3D primitives,
unless stated otherwise.

## Cargo features (from the manifest)

- `alloc` (enables 2)
- `default` (enables 2)
- `std` (enables 7)
- `serialize` (enables 2)
- `approx` (enables 2): Enable approx for glam types to approximate floating point equality comparisons and assertions
- `bevy_reflect` (enables 3): Enable bevy_reflect (requires alloc)
- `rand` (enables 3): Enable the rand dependency for shape_sampling

## Depends on (same ecosystem)

`bevy_math`, `bevy_reflect`

## Public items by file

- `third_party/bevy/crates/bevy_shape/src/bounding/bounded2d/mod.rs`: trait Bounded2d, struct Aabb2d, struct BoundingCircle
- `third_party/bevy/crates/bevy_shape/src/bounding/bounded3d/extrusion.rs`: trait BoundedExtrusion
- `third_party/bevy/crates/bevy_shape/src/bounding/bounded3d/mod.rs`: trait Bounded3d, struct Aabb3d, struct BoundingSphere
- `third_party/bevy/crates/bevy_shape/src/bounding/mod.rs`: trait BoundingVolume, trait IntersectsVolume
- `third_party/bevy/crates/bevy_shape/src/bounding/raycast2d.rs`: struct RayCast2d, struct AabbCast2d, struct BoundingCircleCast
- `third_party/bevy/crates/bevy_shape/src/bounding/raycast3d.rs`: struct RayCast3d, struct AabbCast3d, struct BoundingSphereCast
- `third_party/bevy/crates/bevy_shape/src/dim2.rs`: struct Circle, struct Arc2d, struct CircularSector, struct CircularSegment, struct Ellipse, struct Annulus, struct Rhombus, struct Plane2d, struct Line2d, struct Segment2d, struct Polyline2d, struct Triangle2d, struct Rectangle, struct Polygon, struct ConvexPolygon, enum ConvexPolygonError, struct RegularPolygon, struct Capsule2d, struct Ring, trait ToRing
- `third_party/bevy/crates/bevy_shape/src/dim3.rs`: struct Sphere, struct Plane3d, struct InfinitePlane3d, struct Line3d, struct Segment3d, struct Polyline3d, struct Cuboid, struct Cylinder, struct Capsule3d, struct Cone, struct ConicalFrustum, enum TorusKind, struct Torus, struct Triangle3d, struct Tetrahedron, struct Extrusion
- `third_party/bevy/crates/bevy_shape/src/half_space.rs`: struct HalfSpace
- `third_party/bevy/crates/bevy_shape/src/inset.rs`: trait Inset
- `third_party/bevy/crates/bevy_shape/src/lib.rs`: mod sampling, trait Primitive2d, trait Primitive3d, enum WindingOrder, trait Measured2d, trait Measured3d, mod prelude
- `third_party/bevy/crates/bevy_shape/src/polygon.rs`: fn is_polygon_simple
- `third_party/bevy/crates/bevy_shape/src/ray.rs`: struct Ray2d, struct Ray3d
- `third_party/bevy/crates/bevy_shape/src/sampling/mesh_sampling.rs`: struct UniformMeshSampler
- `third_party/bevy/crates/bevy_shape/src/sampling/mod.rs`: mod mesh_sampling, mod shape_sampling
- `third_party/bevy/crates/bevy_shape/src/sampling/shape_sampling.rs`: trait ShapeSample, struct InteriorOf, struct BoundaryOf
- `third_party/bevy/crates/bevy_shape/src/view_frustum.rs`: struct ViewFrustum

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
