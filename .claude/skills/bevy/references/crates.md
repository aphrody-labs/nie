# Bevy 0.20.1 workspace crates and Cargo features

Source: checkout of bevyengine/bevy at commit a0577b5 (tag v0.20.1). `ls crates` lists 62 directories (verified this session); descriptions are the `description` of each `crates/<name>/Cargo.toml` or the `//!` header of `src/lib.rs`. Workspace members are `crates/*` plus nested macro and compile-fail crates (`Cargo.toml:17-29`). Nested macro crates: `bevy_animation/macros`, `bevy_asset/macros`, `bevy_ecs/macros`, `bevy_ecs/macro_logic`, `bevy_extract/macros`, `bevy_gizmos/macros`, `bevy_material/macros`, `bevy_reflect/derive`, `bevy_render/macros`, `bevy_scene/macros`, `bevy_state/macros` (found with `find crates -mindepth 3 -maxdepth 3 -name Cargo.toml`). `bevy_mikktspace` is NOT in `crates/`: it is a crates.io dependency of `bevy_mesh` (`crates/bevy_mesh/Cargo.toml:23`). Crate-level feature doc: `docs/cargo_features.md`.

## App and ECS core
- `bevy_app`: `App`, `Plugin`, schedules runner (core App functionality).
- `bevy_ecs`: the entity component system (`Entity`, `World`, systems, messages, observers).
- `bevy_state`: finite state machines (`States`, `SubStates`).
- `bevy_tasks`: task executor and thread pools (async executor, `multi_threaded`).
- `bevy_time`: `Time`, timers, fixed timestep.
- `bevy_transform`: `Transform`, `GlobalTransform`, hierarchy propagation.
- `bevy_reflect`: runtime reflection (derive macros in `bevy_reflect/derive`).
- `bevy_derive`: assorted derives (`Deref`, `DerefMut`, app labels).
- `bevy_log`: tracing based logging.
- `bevy_platform`: platform abstraction (std/no_std, time, sync, collections).
- `bevy_ptr`: safe-ish untyped pointer helpers.
- `bevy_internal`: re-export hub that makes `dynamic_linking` optional; the `bevy` crate depends on it only.
- `bevy_dylib`: forces dynamic linking of Bevy for faster iteration (`dynamic_linking`, `Cargo.toml:284`).

## Math, color, utils
- `bevy_math`: vectors, matrices, rotations, primitives, curves' math, bounding volumes (glam based).
- `bevy_color`: color spaces and color types (`Srgba`, `LinearRgba`, `Oklcha`...).
- `bevy_curve`: curve and easing functionality (used by animation).
- `bevy_shape`: primitive shape data definitions.
- `bevy_utils`: small shared helpers.
- `bevy_macro_utils`: helpers for proc-macro crates.
- `bevy_encase_derive`: derive macro for `encase` (GPU buffer layout).

## Assets, scenes, serialization
- `bevy_asset`: `AssetServer`, `Assets<T>`, loaders, savers, `AssetProcessor` (feature `asset_processor`, `crates/bevy_internal/Cargo.toml:320`), watchers (`file_watcher`, `:323`). See `asset.md`.
- `bevy_scene`: Bevy Scene Notation (BSN) composable scene authoring (`crates/bevy_scene/src/lib.rs` header).
- `bevy_world_serialization`: dynamic world definition, instantiation, (de)serialization (the older DynamicScene role; `Cargo.toml:332`).
- `bevy_settings`: saving and loading user settings files.
- `bevy_remote`: Bevy Remote Protocol (BRP) for external tooling.

## Rendering core
- `bevy_render`: renderer on wgpu 30 (render world, extraction, render graph/schedules, pipelines, `GpuImage`, screenshots; `crates/bevy_render/Cargo.toml:90`). See `render.md`.
- `bevy_extract`: extraction of component data between ECS worlds (main -> render).
- `bevy_core_pipeline`: core_2d and core_3d pipelines, prepass, deferred, tonemapping, mip_generation, oit, skybox, upscaling, blit, fullscreen_material (`crates/bevy_core_pipeline/src/`).
- `bevy_post_process`: post-processing: auto_exposure, bloom, dof, motion_blur, gaussian blur, effect_stack, MSAA writeback (`crates/bevy_post_process/src/`).
- `bevy_anti_alias`: FXAA, SMAA, TAA, contrast adaptive sharpening, DLSS (`crates/bevy_anti_alias/src/`).
- `bevy_camera`: camera abstraction (`Camera`, projections, visibility, render targets).
- `bevy_image`: `Image`, `ImageLoader`, KTX2/DDS/Basis loaders, texture atlases, sampler types. See `image-ktx2.md`.
- `bevy_mesh`: `Mesh` types, attributes, primitives to mesh, morph targets, skinning data.
- `bevy_shader`: `Shader` assets and import resolution (`#import`, shader defs). See `shaders-materials.md`.
- `bevy_material`: material abstraction (`AlphaMode`, material keys; shared by 2D, 3D and UI).
- `bevy_gizmos` / `bevy_gizmos_render`: immediate-mode debug drawing and its renderer.
- `bevy_dev_tools`: developer tools: fps_overlay, frame_time_graph, easy_screenshot, infinite_grid, picking_debug, render_debug, ci_testing (`crates/bevy_dev_tools/src/`).
- `bevy_camera_controller`: premade camera controllers (`free_camera`, `pan_camera`, `pan_orbit_camera` features, `Cargo.toml:395-404`).

## 3D, lighting, PBR
- `bevy_pbr`: PBR rendering: `StandardMaterial`, `ExtendedMaterial`, shadows, SSAO, SSR, atmosphere, volumetric fog, meshlets, light probes, decals. See `pbr.md`.
- `bevy_light`: light component types (point, spot, directional, rect, probes).
- `bevy_solari`: raytraced lighting (`bevy_solari` feature, `Cargo.toml:338`; examples `solari*.rs`).
- `bevy_gltf`: glTF 2.0 loader (features: `pbr_transmission_textures`, `pbr_multi_layer_material_textures`, `pbr_anisotropy_texture`, `pbr_specular_textures`, `crates/bevy_gltf/Cargo.toml` `[features]`; no feature exists for draco, meshopt or basisu). See `gltf.md`.
- `bevy_animation`: clips and curves (`animation_curves.rs`, `gltf_curves.rs`), `graph.rs`, `transition.rs`, morph targets, events (feature `gltf_animation` wires it to glTF, `crates/bevy_internal/Cargo.toml:226`). See `animation.md`.

## 2D
- `bevy_sprite`: `Sprite`, `TextureAtlas` usage, 9-slice, sprite picking.
- `bevy_sprite_render`: 2D sprite and `Mesh2d` rendering.

## Text and UI
- `bevy_text`: text layout and fonts (`default_font`, `system_font_discovery`, `complex_script_segmentation`, `Cargo.toml:623-629`).
- `bevy_ui`: ECS-driven UI framework (`Node`, flex/grid layout).
- `bevy_ui_render`: UI renderer.
- `bevy_ui_widgets`: unstyled standard widgets (button, checkbox, slider).
- `bevy_feathers`: styled and themed widgets for editors and tools.
- `bevy_input_focus`: keyboard focus management.
- `bevy_a11y`: accessibility support (AccessKit).
- `bevy_clipboard`: clipboard support (OS access behind `system_clipboard`, `Cargo.toml:467`).
- `bevy_picking`: pointer picking core with backends (`mesh_picking`, `sprite_picking`, `ui_picking`, `Cargo.toml:272-278`).
See `sprite-text-ui.md`.

## Input, window, platform
- `bevy_input`: keyboard, mouse, touch, gamepad input types.
- `bevy_gilrs`: gamepad backend on gilrs.
- `bevy_window`: window types and events.
- `bevy_winit`: winit window and input backend (absent when running headless, see `render.md`).
- `bevy_android`: Android-specific functionality.

## Audio and diagnostics
- `bevy_audio`: audio playback (rodio 0.22, `crates/bevy_audio/Cargo.toml:21`; formats `vorbis`, `wav`, `mp3`, `flac`, `aac`, `mp4`, `Cargo.toml:536-551`).
- `bevy_diagnostic`: diagnostics store with frame time, entity count, log and system-information plugins (`crates/bevy_diagnostic/src/`).

## Root Cargo features (`Cargo.toml`, 0.20.1)
- `default = ["2d","3d","ui","audio"]` (`:136`); profiles `2d` (`:139`), `3d` (`:142`), `ui` (`:145`), `dev` (`:156`), `audio` (`:164`), `scene` (`:170`), `picking` (`:173`).
- Collections: `default_app` (`:176`: async_executor, bevy_asset, bevy_log, bevy_state, reflect_auto_register), `default_platform` (`:185`: std, gilrs, winit, clipboard, default_font, multi_threaded, webgl2, x11, wayland, custom_cursor, sysinfo_plugin), `common_api` (`:202`: animation, camera, color, curve, gizmos, image, mesh, shader, material, text, window, hdr, png), `3d_api` (`:232`: light, mikktspace, ktx2, morph, smaa_luts, tonemapping_luts, zstd_rust), `3d_bevy_render` (`:245`: render, core_pipeline, anti_alias, gltf, pbr, post_process, gltf_animation).
- A windowless 3D build can start from `default-features = false` plus the `3d` members that do not need winit (`3d_bevy_render` at `:245` includes `bevy_render` and `bevy_pbr` but its parent `3d` pulls `default_platform` with `bevy_winit`, `:142,185`); the working feature set for headless capture is in `render.md`.
- Images: `basis-universal` (`:456`), `compressed_image_saver*` (`:459-464`), `bmp dds exr ff gif hdr ktx2 ico jpeg png pnm qoi tga tiff webp zlib` (`:473-518`), `zstd_rust` / `zstd_c` (`:521,524`). Note `jpeg` and `dds` are not default.
- Assets: `asset_processor` (`:664`), `file_watcher` (`:667`, hot reload), `embedded_watcher` (`:670`), `http` / `https` / `web_asset_cache` (`:673-679`).
- Rendering extras: `webgl2` (`:658`), `webgpu` (`:661`), `shader_format_spirv` (`:632`), `spirv_shader_passthrough` (`:432`), `meshlet` / `meshlet_processor` (`:691,694`), `dlss` (`:608`), `render_dev_tools` (`:578`), `morph` / `morph_animation` (`:584,587`), `pbr_*` texture features (`:635-655`), LUT features `tonemapping_luts smaa_luts bluenoise_texture dfg_lut area_light_luts` (`:593-605`).
- Tracing: `trace`, `trace_tracy`, `trace_chrome` (`:444-453`).
- Bevy asset hot reload during development: `dev` includes `file_watcher` (`:156-161`).
