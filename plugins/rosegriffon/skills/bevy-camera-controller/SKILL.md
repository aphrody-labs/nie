---
name: bevy-camera-controller
description: "Use when working with the `bevy_camera_controller` crate of bevy: Premade camera controllers for Bevy."
---

# bevy_camera_controller

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_camera_controller/` (manifest `third_party/bevy/crates/bevy_camera_controller/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_camera_controller
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Premade camera controllers for Bevy

## Crate docs (`third_party/bevy/crates/bevy_camera_controller/src/lib.rs`)

A home for first-party camera controllers for Bevy,
used for moving the camera around your scene.

This crate serves two key purposes:

1. It provides functional camera controllers to help users quickly get started.
2. It holds the camera controllers used by Bevy's own examples and tooling.
 
While these camera controllers are customizable,
there is a limit to the customization options available.
If you find your project requires different behavior,
do not hesitate to copy-paste the camera controller code
into your own project and modify it as needed.

Each of the provided controllers is gated behind a feature flag,
so you don't have to pay for unused camera controllers.
These features are all off by default; to enable them,
you need to specify the desired features in your Cargo.toml file.

For example, to enable the `free_camera` camera controller,
you would add the following to your Cargo.toml:

```toml
[dependencies]
bevy = { version = "0.X", features = ["free_camera"] }
```

Once the correct feature is enabled,
add the camera controller plugin to your Bevy app.
If your camera is for debugging and development purposes,
consider adding a feature flag (e.g. `dev-mode`) or a run condition on
the systems in the plugin, which can check a configuration resource.

For a full overview of the available camera controllers,
please check out the modules of this crate.
Each camera controller is stored in its own module,
and gated behind a feature flag of the same name.

## Cargo features (from the manifest)

- `default` (enables 1)
- `bevy_reflect` (enables 1)
- `libm` (enables 1)
- `free_camera`: Camera controllers
- `pan_camera`
- `pan_orbit_camera`
- `extension_anchor_indicator` (enables 2)
- `extension_independent_skybox` (enables 3)

## Depends on (same ecosystem)

`bevy_app`, `bevy_asset`, `bevy_camera`, `bevy_color`, `bevy_core_pipeline`, `bevy_curve`, `bevy_derive`, `bevy_ecs`, `bevy_gizmos`, `bevy_image`, `bevy_input`, `bevy_log`, `bevy_math`, `bevy_picking`, `bevy_platform`, `bevy_reflect`, `bevy_render`, `bevy_time`, `bevy_transform`, `bevy_window`

## Public items by file

- `third_party/bevy/crates/bevy_camera_controller/src/free_camera.rs`: struct FreeCameraPlugin, struct FreeCamera, enum VerticalMovementAxis, struct FreeCameraState, fn run_freecamera_controller, fn rotate_freecam_to
- `third_party/bevy/crates/bevy_camera_controller/src/lib.rs`: mod free_camera, mod pan_camera, mod pan_orbit_camera
- `third_party/bevy/crates/bevy_camera_controller/src/pan_camera.rs`: struct PanCameraPlugin, struct PanCamera, struct MousePanSettings, enum ZoomTarget, struct ZoomEvent
- `third_party/bevy/crates/bevy_camera_controller/src/pan_orbit_camera/controller/component.rs`: struct PanOrbitCamera, fn rotate_around, fn look_to, fn cam_forward, fn cam_left, fn cam_up, enum OrbitConstraint, struct Sensitivity, struct EnabledMotion
- `third_party/bevy/crates/bevy_camera_controller/src/pan_orbit_camera/controller/inputs.rs`: enum MotionInputs
- `third_party/bevy/crates/bevy_camera_controller/src/pan_orbit_camera/controller/mod.rs`: mod component, mod inputs, mod momentum, mod motion, mod projections, mod smoothing, mod transform_utils, mod zoom, struct MinimalPanOrbitCameraPlugin
- `third_party/bevy/crates/bevy_camera_controller/src/pan_orbit_camera/controller/momentum.rs`: struct Momentum, enum Velocity
- `third_party/bevy/crates/bevy_camera_controller/src/pan_orbit_camera/controller/motion.rs`: enum CurrentMotion
- `third_party/bevy/crates/bevy_camera_controller/src/pan_orbit_camera/controller/projections.rs`: struct PerspectiveSettings, fn update_perspective, struct OrthographicSettings, fn update_orthographic
- `third_party/bevy/crates/bevy_camera_controller/src/pan_orbit_camera/controller/smoothing.rs`: struct Smoothing, struct InputQueue, struct InputStreamEntry
- `third_party/bevy/crates/bevy_camera_controller/src/pan_orbit_camera/controller/transform_utils.rs`: fn read_transform, fn apply_delta
- `third_party/bevy/crates/bevy_camera_controller/src/pan_orbit_camera/controller/zoom.rs`: struct ZoomLimits, fn length_per_pixel_at_view_space_pos
- `third_party/bevy/crates/bevy_camera_controller/src/pan_orbit_camera/extensions/anchor_indicator.rs`: struct AnchorIndicatorPlugin, struct AnchorIndicator, fn draw_anchor
- `third_party/bevy/crates/bevy_camera_controller/src/pan_orbit_camera/extensions/dolly_zoom.rs`: struct DollyZoomPlugin, struct DollyZoomTrigger, struct DollyZoom
- `third_party/bevy/crates/bevy_camera_controller/src/pan_orbit_camera/extensions/independent_skybox.rs`: struct IndependentSkyboxPlugin, struct IndependentSkybox, enum SkyboxFov, struct IndependentSkyboxCamera
- `third_party/bevy/crates/bevy_camera_controller/src/pan_orbit_camera/extensions/look_to.rs`: struct LookToPlugin, struct LookToTrigger, struct LookTo
- `third_party/bevy/crates/bevy_camera_controller/src/pan_orbit_camera/extensions/mod.rs`: mod dolly_zoom, mod look_to, mod anchor_indicator, mod independent_skybox
- `third_party/bevy/crates/bevy_camera_controller/src/pan_orbit_camera/mod.rs`: mod controller, mod extensions, mod prelude, struct DefaultPanOrbitCameraPlugins, struct SyncCameraPosition

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
