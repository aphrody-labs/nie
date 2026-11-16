---
name: bevy-light
description: "Use when working with the `bevy_light` crate of bevy: Keeps the lights on at Bevy Engine."
---

# bevy_light

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_light/` (manifest `third_party/bevy/crates/bevy_light/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_light
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Keeps the lights on at Bevy Engine

## Crate docs (`third_party/bevy/crates/bevy_light/src/lib.rs`)

Provides component types for lighting a bevy scene. This includes the usual
directional, point, and spot lights, as well as light probes, atmosphere,
other volumetrics, and shadow configuration.

## Cargo features (from the manifest)

- `default`
- `experimental_pbr_pcss`
- `webgl`
- `webgpu`

## Depends on (same ecosystem)

`bevy_app`, `bevy_asset`, `bevy_camera`, `bevy_color`, `bevy_curve`, `bevy_ecs`, `bevy_gizmos`, `bevy_image`, `bevy_log`, `bevy_math`, `bevy_mesh`, `bevy_platform`, `bevy_reflect`, `bevy_shape`, `bevy_transform`, `bevy_utils`

## Public items by file

- `third_party/bevy/crates/bevy_light/src/ambient_light.rs`: struct AmbientLight, struct GlobalAmbientLight
- `third_party/bevy/crates/bevy_light/src/atmosphere.rs`: struct Atmosphere, struct ScatteringMedium, struct ScatteringTerm, enum Falloff, enum PhaseFunction, fn extract_chromatic_phase_textures
- `third_party/bevy/crates/bevy_light/src/cascade.rs`: struct CascadeShadowConfig, struct CascadeShadowConfigBuilder, struct Cascades, struct Cascade, fn build_directional_light_cascades
- `third_party/bevy/crates/bevy_light/src/cluster/assign.rs`: enum ClusterableObjectType, fn calculate_cluster_factors
- `third_party/bevy/crates/bevy_light/src/cluster/mod.rs`: mod assign, struct GlobalClusterSettings, struct GlobalClusterGpuSettings, enum ClusterFarZMode, struct ClusterZConfig, enum ClusterConfig, struct Clusters, enum ClusterableObjects, struct ClusterVisibilityClass, struct ObjectsInClusterCpu, struct ClusterableObjectCounts, struct ClusteredDecal, fn add_light_probe_and_decal_aabbs
- `third_party/bevy/crates/bevy_light/src/directional_light.rs`: struct DirectionalLight, struct DirectionalLightTexture, struct DirectionalLightShadowMap, fn validate_shadow_map_size, fn update_directional_light_frusta, struct SunDisk
- `third_party/bevy/crates/bevy_light/src/gizmos.rs`: struct LightGizmoPlugin, enum LightGizmoColor, struct LightGizmoConfigGroup, struct ShowLightGizmo
- `third_party/bevy/crates/bevy_light/src/lib.rs`: mod cluster, mod atmosphere, mod cascade, mod gizmos, mod prelude, mod light_consts, struct LightPlugin, type WithLight, struct NotShadowCaster, struct NotShadowReceiver, struct TransmittedShadowReceiver, enum ShadowFilteringMethod, enum SimulationLightSystems, fn check_dir_light_mesh_visibility, fn check_point_light_mesh_visibility, fn get_shadow_lod_origin
- `third_party/bevy/crates/bevy_light/src/point_light.rs`: struct PointLight, struct PointLightTexture, struct PointLightShadowMap, fn update_point_light_bounding_spheres, fn update_point_light_frusta
- `third_party/bevy/crates/bevy_light/src/probe.rs`: struct LightProbe, struct EnvironmentMapLight, struct Skybox, struct GeneratedEnvironmentMapLight, struct AtmosphereEnvironmentMapLight, struct IrradianceVolume, enum ParallaxCorrection, fn automatically_add_parallax_correction_components
- `third_party/bevy/crates/bevy_light/src/rect_light.rs`: struct RectLight
- `third_party/bevy/crates/bevy_light/src/spot_light.rs`: struct SpotLight, fn orthonormalize, fn spot_light_world_from_view, fn spot_light_clip_from_view, struct SpotLightTexture, fn update_spot_light_bounding_spheres, fn update_spot_light_frusta
- `third_party/bevy/crates/bevy_light/src/volumetric.rs`: struct VolumetricLight, struct VolumetricFog, struct FogVolume

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
