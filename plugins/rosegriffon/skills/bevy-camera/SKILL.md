---
name: bevy-camera
description: "Use when working with the `bevy_camera` crate of bevy: Provides a camera abstraction for Bevy Engine."
---

# bevy_camera

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_camera/` (manifest `third_party/bevy/crates/bevy_camera/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_camera
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides a camera abstraction for Bevy Engine

## Cargo features (from the manifest)

- `default`

## Depends on (same ecosystem)

`bevy_app`, `bevy_asset`, `bevy_color`, `bevy_derive`, `bevy_ecs`, `bevy_image`, `bevy_math`, `bevy_mesh`, `bevy_reflect`, `bevy_shape`, `bevy_transform`, `bevy_utils`, `bevy_window`

## Public items by file

- `third_party/bevy/crates/bevy_camera/src/camera.rs`: struct Viewport, struct MainPassResolutionOverride, struct SubCameraView, struct RenderTargetInfo, struct ComputedCameraValues, struct Exposure, struct PhysicalCameraParameters, enum ViewportConversionError, struct Camera, struct ShadowLodOrigin, enum CameraOutputMode, enum RenderTarget, enum NormalizedRenderTarget, struct ManualTextureViewHandle, struct ImageRenderTarget, struct CameraMainTextureUsages
- `third_party/bevy/crates/bevy_camera/src/clear_color.rs`: enum ClearColorConfig, enum MsaaWriteback, struct ClearColor
- `third_party/bevy/crates/bevy_camera/src/components.rs`: struct Camera2d, struct Camera3d, struct Camera3dDepthTextureUsage, enum Camera3dDepthLoadOp, struct Hdr, struct TonemappingPass, enum CompositingSpace
- `third_party/bevy/crates/bevy_camera/src/lib.rs`: mod primitives, mod visibility, struct CameraPlugin, mod prelude, struct CameraUpdateSystems
- `third_party/bevy/crates/bevy_camera/src/primitives.rs`: trait MeshAabb, struct Aabb, struct Sphere, struct Frustum, struct CubeMapFace, fn face_index_to_name, struct CubemapFrusta, enum CubemapLayout, struct CascadesFrusta
- `third_party/bevy/crates/bevy_camera/src/projection.rs`: struct CameraProjectionPlugin, trait CameraProjection, struct CustomProjection, enum Projection, struct PerspectiveProjection, enum ScalingMode, struct OrthographicProjection
- `third_party/bevy/crates/bevy_camera/src/visibility/mod.rs`: struct NoCpuCulling, enum Visibility, struct InheritedVisibility, struct VisibilityClass, struct ViewVisibility, trait SetViewVisibility, struct NoFrustumCulling, struct DynamicSkinnedMeshBounds, struct VisibleEntities, struct VisibleMeshEntities, struct CubemapVisibleEntities, struct CascadesVisibleEntities, enum VisibilitySystems, struct VisibilityPlugin, struct NoAutoAabb, fn calculate_bounds, fn update_frusta, fn check_visibility_cpu_culling, fn check_visibility_gpu_culling, fn add_visibility_class
- `third_party/bevy/crates/bevy_camera/src/visibility/range.rs`: struct VisibilityRangePlugin, struct VisibilityRange, struct VisibleEntityRanges, fn check_visibility_ranges
- `third_party/bevy/crates/bevy_camera/src/visibility/render_layers.rs`: type Layer, struct RenderLayers

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
