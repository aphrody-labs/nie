---
name: bevy-picking
description: "Use when working with the `bevy_picking` crate of bevy: Provides screen picking functionality for Bevy Engine."
---

# bevy_picking

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_picking/` (manifest `third_party/bevy/crates/bevy_picking/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_picking
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides screen picking functionality for Bevy Engine

## README (`third_party/bevy/crates/bevy_picking/README.md`)

# Bevy Picking

## Cargo features (from the manifest)

- `mesh_picking` (enables 2): Provides a mesh picking backend
- `custom_cursor` (enables 1)

## Depends on (same ecosystem)

`bevy_app`, `bevy_asset`, `bevy_camera`, `bevy_derive`, `bevy_ecs`, `bevy_input`, `bevy_math`, `bevy_mesh`, `bevy_platform`, `bevy_reflect`, `bevy_shape`, `bevy_time`, `bevy_transform`, `bevy_window`

## Public items by file

- `third_party/bevy/crates/bevy_picking/src/backend.rs`: mod prelude, trait HitDataExtra, struct PointerHits, struct HitData, mod ray
- `third_party/bevy/crates/bevy_picking/src/cursor.rs`: struct DefaultCursor, enum EntityCursor, struct OverrideCursor, struct CursorIconPlugin
- `third_party/bevy/crates/bevy_picking/src/events.rs`: struct Pointer, struct PointerTraversal, trait PointerEvent, struct PointerCancel, struct PointerOver, struct PointerEnter, struct PointerOut, struct PointerLeave, struct PointerPress, struct PointerRelease, struct PointerClick, struct PointerMove, struct PointerDragStart, struct PointerDrag, struct PointerDragEnd, struct PointerDragEnter, struct PointerDragOver, struct PointerDragLeave, struct PointerDragDrop, struct PointerScroll, struct DragEntry, struct PointerButtonState, struct HoveredEntityAncestors, struct PointerState, ...
- `third_party/bevy/crates/bevy_picking/src/hover.rs`: struct HoverMap, struct PreviousHoverMap, struct PointerCaptureMap, fn generate_hovermap, enum PickingInteraction, fn update_interactions, struct Hovered, struct DirectlyHovered, fn update_is_hovered, fn update_is_directly_hovered
- `third_party/bevy/crates/bevy_picking/src/input.rs`: mod prelude, struct PointerInputSettings, struct PointerInputPlugin, fn spawn_mouse_pointer, fn mouse_pick_events, fn touch_pick_events, fn deactivate_touch_pointers
- `third_party/bevy/crates/bevy_picking/src/lib.rs`: mod backend, mod cursor, mod events, mod hover, mod input, mod mesh_picking, mod pointer, mod window, mod prelude, struct Pickable, enum PickingSystems, struct DefaultPickingPlugins, struct PickingSettings, struct PickingPlugin, struct InteractionPlugin
- `third_party/bevy/crates/bevy_picking/src/mesh_picking/mod.rs`: mod ray_cast, struct MeshPickingCamera, struct MeshPickingSettings, struct MeshPickingPlugin, fn update_hits
- `third_party/bevy/crates/bevy_picking/src/mesh_picking/ray_cast/intersections.rs`: struct RayMeshHit, struct RayTriangleHit, fn ray_mesh_intersection, fn ray_aabb_intersection_3d
- `third_party/bevy/crates/bevy_picking/src/mesh_picking/ray_cast/mod.rs`: enum RayCastVisibility, struct MeshRayCastSettings, enum Backfaces, struct RayCastBackfaces, struct SimplifiedMesh, struct MeshRayCast
- `third_party/bevy/crates/bevy_picking/src/pointer.rs`: enum PointerId, struct PointerInteraction, struct PointerMap, struct PointerPressState, enum PressDirection, enum PointerButton, struct PointerLocation, struct Location, enum PointerAction, struct PointerInput
- `third_party/bevy/crates/bevy_picking/src/window.rs`: fn update_window_hits

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
