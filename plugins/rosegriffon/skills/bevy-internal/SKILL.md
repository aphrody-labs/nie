---
name: bevy-internal
description: "Use when working with the `bevy_internal` crate of bevy: An internal Bevy crate used to facilitate optional dynamic linking via the 'dynamic_linking' feature."
---

# bevy_internal

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_internal/` (manifest `third_party/bevy/crates/bevy_internal/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_internal
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

An internal Bevy crate used to facilitate optional dynamic linking via the 'dynamic_linking' feature

## README (`third_party/bevy/crates/bevy_internal/README.md`)

# Bevy Internal

## Cargo features (from the manifest)

- `trace` (enables 12)
- `trace_chrome` (enables 1)
- `trace_tracy` (enables 2)
- `trace_tracy_memory` (enables 3)
- `detailed_trace` (enables 2)
- `sysinfo_plugin` (enables 1)
- `compressed_image_saver_universal` (enables 1): Texture compression asset processor (cross-platform, transcodes to any GPU format at load time)
- `compressed_image_saver` (enables 1): Texture compression asset processor (BCn for desktop, ASTC for mobile via env var)
- `equirectangular_cubemap` (enables 1): Equirectangular (lat-long) panorama to cubemap conversion
- `zlib` (enables 1): For ktx2 supercompression
- `zstd` (enables 1)
- `zstd_rust` (enables 1)
- `zstd_c` (enables 1)
- `basis-universal` (enables 1): Image format support (HDR and PNG enabled by default)
- `bmp` (enables 1)
- `ff` (enables 1)
- `gif` (enables 1)
- `ico` (enables 1)
- `jpeg` (enables 1)
- `png` (enables 1)
- `pnm` (enables 1)
- `qoi` (enables 1)
- `tga` (enables 1)
- `tiff` (enables 1)
- `webp` (enables 1)
- `dds` (enables 1)
- `exr` (enables 1)
- `hdr` (enables 1)
- `ktx2` (enables 1)
- `spirv_shader_passthrough` (enables 1): Enable SPIR-V passthrough
- `statically-linked-dxc` (enables 1): Statically linked DXC shader compiler for DirectX 12 TODO: When wgpu switches to DirectX 12 instead of Vulkan by default on windows, make this a default feature
- `raw_vulkan_init` (enables 1): Forces the wgpu instance to be initialized using the raw Vulkan HAL, enabling additional configuration
- `type_label_buffers` (enables 1): Pre-populate buffer labels with buffer types for debugging.
- `tonemapping_luts` (enables 3): Include tonemapping LUT KTX2 files.
- `smaa_luts` (enables 3): Include SMAA LUT KTX2 Files
- `bluenoise_texture` (enables 3): Include Bluenoise texture for environment map generation.
- `dfg_lut` (enables 3): Include a preintegrated BRDF Look Up Table for more accurate specular shading.
- `area_light_luts` (enables 3): Include Look Up Tables that are required for area lights.
- `dlss` (enables 2): NVIDIA Deep Learning Super Sampling
- `force_disable_dlss` (enables 2): Forcibly disable DLSS so that `cargo build --all-features` works without the DLSS SDK being installed. Not meant for users.
- `flac` (enables 1): Audio format support (vorbis is enabled by default)
- `mp3` (enables 1)
- `mp4` (enables 1)
- `vorbis` (enables 1)
- `wav` (enables 1)
- `aac` (enables 1)
- `symphonia-flac` (enables 1)
- `symphonia-vorbis` (enables 1)
- `symphonia-wav` (enables 1)
- `shader_format_spirv` (enables 1): Shader formats
- `serialize` (enables 19)
- `multi_threaded` (enables 6)
- `async-io` (enables 2)
- `wayland` (enables 2): Display server protocol support (X11 is enabled by default)
- `x11` (enables 1)
- `android-native-activity` (enables 1): Android activity support (choose one)
- `android-game-activity` (enables 1)
- `pbr_transmission_textures` (enables 2): Transmission textures in `StandardMaterial`:
- `pbr_clustered_decals` (enables 1): Clustered Decal support
- `pbr_light_textures` (enables 2): Light Texture support

## Depends on (same ecosystem)

`bevy-settings`, `bevy_a11y`, `bevy_animation`, `bevy_annotations`, `bevy_anti_alias`, `bevy_app`, `bevy_asset`, `bevy_audio`, `bevy_bsn`, `bevy_camera`, `bevy_camera_controller`, `bevy_clipboard`, `bevy_color`, `bevy_core_pipeline`, `bevy_curve`, `bevy_derive`, `bevy_dev_tools`, `bevy_diagnostic`, `bevy_ecs`, `bevy_extract`, `bevy_feathers`, `bevy_fuzzy`, `bevy_gilrs`, `bevy_gizmos`, `bevy_gizmos_render`, `bevy_gltf`, `bevy_image`, `bevy_input`, `bevy_input_focus`, `bevy_inspector`, `bevy_light`, `bevy_log`, `bevy_material`, `bevy_math`, `bevy_mesh`, `bevy_pbr`, `bevy_picking`, `bevy_platform`, `bevy_post_process`, `bevy_ptr`, `bevy_reflect`, `bevy_remote`, `bevy_render`, `bevy_scene`, `bevy_shader`, `bevy_shape`, `bevy_solari`, `bevy_sprite`, `bevy_sprite_render`, `bevy_state`, `bevy_tasks`, `bevy_text`, `bevy_time`, `bevy_transform`, `bevy_ui`, `bevy_ui_render`, `bevy_ui_widgets`, `bevy_utils`, `bevy_window`, `bevy_winit`, `bevy_world_serialization`

## Public items by file

- `third_party/bevy/crates/bevy_internal/src/lib.rs`: mod prelude

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
