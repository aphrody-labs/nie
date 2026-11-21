---
name: bevy
description: Bevy 0.20.1 (wgpu) reference: glTF/GLB, KTX2, animation graph, PBR, WESL shaders, assets, offscreen capture. Use when writing or debugging Bevy code or loading assets.
---

# bevy

Pinned: `bevyengine/bevy` v0.20.1, commit `a0577b5` (Release 0.20.1, 2026-10-28), fork `aphrody-labs/bevy`, wgpu 30 (`crates/bevy_render/Cargo.toml:90`). nie depends on Bevy crates 0.20.1 (`Cargo.toml`). Local checkout used for the citations: `/c/src/gpu-bench/vendor/bevy` (read only; owned by gpu-bench). Every reference cites `path:line` at that commit or a live URL (bevy.org, docs.rs/bevy/0.20.1).

Rule: Bevy breaks API every release. Never write Bevy code from memory: open the matching `examples/` file of the pin first, and prefer a `cargo check` on `fr1` (never build Bevy on the PC).

## Pick the reference
| Task | File |
|---|---|
| What glTF extensions load (KTX2, Draco, meshopt, basisu), loader settings, labels, extension handlers | `references/gltf.md` |
| Recipes (not compiled): load `gltf.md`, named clip `animation.md`, 8-direction PNG `render.md`, KTX2 `image-ktx2.md`, toon `shaders-materials.md` | see files |
| Animation graph, player, transitions, masks, clip by name | `references/animation.md` |
| StandardMaterial, lights, shadows, SSAO, atmosphere, volumetric fog, tonemapping | `references/pbr.md` |
| `Material`, `ExtendedMaterial`, `AsBindGroup`, WESL shaders, `ShaderBuffer`, hot reload | `references/shaders-materials.md` |
| Render schedule, extraction, pipelines, headless and offscreen screenshots, lavapipe | `references/render.md` |
| AssetServer, labels, `.meta`, AssetProcessor, processed assets, custom loaders | `references/asset.md` |
| Image features, KTX2/Basis transcoding, mipmaps, samplers, toktx flags | `references/image-ktx2.md` |
| Sprites, atlases, sprite sheets, picking, text, UI | `references/sprite-text-ui.md` |
| All 62 workspace crates and cargo features | `references/crates.md` |
| bevy.org map, 0.20 release notes, migration guide tables, Bevy Assets plugins | `references/bevyengine-org.md` |

## glTF verdict for the asset lab (Bevy 0.20.1)
- Loads: GLB/glTF 2.0 with embedded or external PNG/JPEG, KTX2 as an image format (features `ktx2` + `zstd_rust`; UASTC needs `basis-universal`; BasisLZ/ETC1S is rejected, `crates/bevy_image/src/ktx2.rs:96-100`), skins up to 256 joints, morph targets, node animation (linear, step, cubic spline), `KHR_lights_punctual`, `KHR_materials_{unlit,ior,emissive_strength,transmission,volume,specular,clearcoat,anisotropy}`, `KHR_texture_transform` (base color only).
- Does NOT load: `KHR_texture_basisu`, `EXT_meshopt_compression`, `KHR_draco_mesh_compression`, `KHR_mesh_quantization` (`crates/bevy_gltf/src/lib.rs:93-133`). Details in `references/gltf.md` (code-read verdict; the loader was not run on test files).
- So the exporter emits plain accessors, no required extensions, names on nodes and animations; gltfpack only with `-noq` and without `-cc`/`-tc`.

## 0.20 traps (verified in the pin)
- Shaders are WESL (`.wesl`), naga_oil is gone: `import bevy_pbr::render::pbr_functions::apply_pbr_lighting;`, `@if(FLAG)`, `constants::NAME`; plain WGSL without directives still loads (`_release-content/migration-guides/shaders_use_wesl.md`).
- `SceneRoot` is `WorldAssetRoot`, `SceneInstanceReady` is `WorldInstanceReady` (`bevy::world_serialization`); glTF scenes are `Handle<WorldAsset>`.
- `asset_server.load_with_settings` and `load_acquire` are gone: `asset_server.load_builder().with_settings(|s: &mut S| ..).load(path)` (`crates/bevy_asset/src/server/mod.rs:371,1737`).
- `ShaderStorageBuffer` is `ShaderBuffer` (`crates/bevy_render/src/storage.rs:60`); lights are in `bevy_light`; `Tonemapping` is in `bevy_render::view`; `Screenshot`/`ScreenshotCaptured` are in `bevy::render::view::screenshot`.
- `Image::try_into_dynamic` supports only sRGB RGBA8 among 8-bit RGBA: render offscreen to `Rgba8UnormSrgb` (`crates/bevy_image/src/image_texture_conversion.rs:228`).
- glTF faces +Z; Bevy forward is -Z: set `GltfConvertCoordinates` or rotate the root (`crates/bevy_gltf/src/convert_coordinates.rs:64`).
- `KHR_texture_transform` only reaches `base_color_texture`; `StandardMaterial.uv_transform` is one transform for all maps.

## Workflow
1. Identify the crate and feature in `references/crates.md`; enable only the features needed (`default-features = false`).
2. Copy the closest example of the pin (`examples/<area>/`), then adapt; the recipes in the references are NOT compiled yet: `cargo check` them on fr1 before relying on them.
3. Heavy builds: `ssh fr1`, `CARGO_TARGET_DIR` on a volume with space (fr1 root disk is nearly full; `/tmp` is tmpfs), `-j` modest, delete the target afterward.
4. Headless GPU on fr1: `WGPU_BACKEND=vulkan WGPU_FORCE_FALLBACK_ADAPTER=1` (lavapipe ICD present at `/usr/share/vulkan/icd.d/lvp_icd.json`), `WinitPlugin` disabled, see `references/render.md`.
5. Verify output by inspecting the PNGs (screenshots), not by exit code.

## Related
- Benchmarks and Bevy scenes for GPU comparison: `/c/src/gpu-bench` (another worker; `vendor/bevy` checkout, WGSL kernels in `wgsl/`).
- Asset pipeline and exporters: private lab `aphrody-labs/assets-lab` (`docs/bevy.md` there summarises this skill).
- wgpu/WGSL/Vulkan generic skills: `wgpu`, `webgpu`, `vulkan` in the unified aphrody plugin.
