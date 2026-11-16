---
name: bevy-gizmos
description: "Use when working with the `bevy_gizmos` crate of bevy: Provides gizmos for Bevy Engine."
---

# bevy_gizmos

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_gizmos/` (manifest `third_party/bevy/crates/bevy_gizmos/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_gizmos
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides gizmos for Bevy Engine

## README (`third_party/bevy/crates/bevy_gizmos/README.md`)

# Bevy Gizmos

## Depends on (same ecosystem)

`bevy_app`, `bevy_asset`, `bevy_camera`, `bevy_color`, `bevy_curve`, `bevy_ecs`, `bevy_gizmos_macros`, `bevy_input`, `bevy_log`, `bevy_math`, `bevy_mesh`, `bevy_reflect`, `bevy_shape`, `bevy_time`, `bevy_transform`, `bevy_utils`, `bevy_window`

## Public items by file

- `third_party/bevy/crates/bevy_gizmos/src/aabb.rs`: struct AabbGizmoPlugin, struct AabbGizmoConfigGroup, struct ShowAabbGizmo
- `third_party/bevy/crates/bevy_gizmos/src/arcs.rs`: struct Arc2dBuilder, struct Arc3dBuilder
- `third_party/bevy/crates/bevy_gizmos/src/arrows.rs`: struct ArrowBuilder
- `third_party/bevy/crates/bevy_gizmos/src/circles.rs`: struct EllipseBuilder, struct Ellipse2dBuilder, struct SphereBuilder
- `third_party/bevy/crates/bevy_gizmos/src/config.rs`: enum GizmoLineJoint, enum GizmoLineStyle, trait GizmoConfigGroup, struct DefaultGizmoConfigGroup, struct ErasedGizmoConfigGroup, struct GizmoConfigStore, struct GizmoConfig, struct GizmoLineConfig, struct GizmoMeshConfig
- `third_party/bevy/crates/bevy_gizmos/src/frustum.rs`: struct FrustumGizmoPlugin, struct FrustumGizmoSystems, struct FrustumGizmoConfigGroup, struct ShowFrustumGizmo
- `third_party/bevy/crates/bevy_gizmos/src/gizmos.rs`: struct GizmoStorage, struct Swap, struct Gizmos, struct GizmosFetchState, struct GizmoBuffer, struct GizmoBufferView
- `third_party/bevy/crates/bevy_gizmos/src/global.rs`: struct GlobalGizmosPlugin, fn gizmo
- `third_party/bevy/crates/bevy_gizmos/src/grid.rs`: struct GridBuilder3d, struct GridBuilder2d
- `third_party/bevy/crates/bevy_gizmos/src/lib.rs`: mod aabb, mod arcs, mod arrows, mod circles, mod config, mod cross, mod curves, mod frustum, mod gizmos, mod grid, mod primitives, mod retained, mod rounded_box, mod stroke_text, mod transform_gizmo, mod skinned_mesh_bounds, mod prelude, struct GizmoPlugin, trait AppGizmoBuilder, struct GizmoHandles, fn start_gizmo_context, fn end_gizmo_context, fn collect_requested_gizmos, fn clear_gizmo_context, ...
- `third_party/bevy/crates/bevy_gizmos/src/primitives/dim2.rs`: trait GizmoPrimitive2d, struct Annulus2dBuilder, struct Line2dBuilder, struct Segment2dBuilder
- `third_party/bevy/crates/bevy_gizmos/src/primitives/dim3.rs`: trait GizmoPrimitive3d, struct Plane3dBuilder, struct Cylinder3dBuilder, struct Capsule3dBuilder, struct Cone3dBuilder, struct ConicalFrustum3dBuilder, struct Torus3dBuilder
- `third_party/bevy/crates/bevy_gizmos/src/primitives/mod.rs`: mod dim2, mod dim3
- `third_party/bevy/crates/bevy_gizmos/src/retained.rs`: struct Gizmo
- `third_party/bevy/crates/bevy_gizmos/src/rounded_box.rs`: struct RoundedRectBuilder, struct RoundedCuboidBuilder
- `third_party/bevy/crates/bevy_gizmos/src/skinned_mesh_bounds.rs`: struct SkinnedMeshBoundsGizmoPlugin, struct SkinnedMeshBoundsGizmoConfigGroup, struct ShowSkinnedMeshBoundsGizmo
- `third_party/bevy/crates/bevy_gizmos/src/stroke_text.rs`: struct StrokeFont, struct StrokeTextLayout
- `third_party/bevy/crates/bevy_gizmos/src/transform_gizmo.rs`: struct TransformGizmoFocus, struct TransformGizmoCamera, enum TransformGizmoMode, enum TransformGizmoSpace, enum TransformGizmoAxis, struct TransformGizmoSettings, struct TransformGizmoState, struct TransformGizmoSystems, struct TransformGizmoRoot, struct TransformGizmoMeshMarker, struct TransformGizmoPlugin, fn axis_direction, fn translation_plane_normal, fn intersect_plane, fn point_to_segment_dist, fn point_to_ring_screen_dist, fn effective_space, fn gizmo_rotation, macro resolve_gizmo_camera!

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
