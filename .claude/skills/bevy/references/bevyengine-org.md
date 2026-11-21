# bevyengine.org (now bevy.org), release notes and migration guides

All URLs below were fetched live on 2026-11-21 with WebFetch (a small model summarizes each page; anchors come from those summaries and from the local drafts, so open a URL before relying on an exact anchor). Local source of truth for the pinned release: bevyengine/bevy tag v0.20.1, commit a0577b5 ("Release 0.20.1", Wed Oct 28 2026), `_release-content/`.

## Site map

- `https://bevyengine.org/*` answers 301 to `https://bevy.org/*` (WebFetch redirect report for `/learn/`, `/examples/`, `/assets/`). Use bevy.org.
- Learn hub `https://bevy.org/learn/`: Quick Start book `/learn/quick-start/introduction` (getting started `/learn/quick-start/getting-started/`), Migration Guides `/learn/migration-guides/introduction`, API docs `https://docs.rs/bevy`, Contributing `/learn/contribute/introduction`, Bevy Errors `/learn/errors/introduction`, FAQ `/faq`, official examples `https://github.com/bevyengine/bevy/tree/latest/examples#examples`, web examples `/examples`, assets `/assets`. The page does not list individual book chapters.
- News index `https://bevy.org/news/`: 0.20 (2026-10-08) `/news/bevy-0-20/`, 0.19 (2026-06-19) `/news/bevy-0-19/`, 0.18 (2026-01-13) `/news/bevy-0-18/`, 0.17 (2025-09-30) `/news/bevy-0-17/`. No 0.20.x post is listed. Atom feed `/atom.xml`.
- API docs for the pin: `https://docs.rs/bevy/0.20.1/bevy/` (HTTP 200 via curl this session; `.../bevy/render/texture/index.html` also 200). Rustdoc is built with `all-features = true` (`crates/bevy_image/Cargo.toml` `[package.metadata.docs.rs]`).
- Migration guide URLs follow `/learn/migration-guides/0-19-to-0-20/`, `0-18-to-0-19/`, `0-17-to-0-18/`. The checkout holds the 0.20 drafts as 75 files in `_release-content/migration-guides/` (75 sections on the site, same count) and release-note drafts in `_release-content/release-notes/`.

## Examples index (`https://bevy.org/examples/`)

- WebGL2 build of the examples run in the browser; a WebGPU variant is linked. 27 categories: 2D Rendering, 3D Rendering, Animation, Application, Assets, Async Tasks, Audio, Camera, Dev tools, Diagnostics, ECS, Games, Gizmos, Math, Movement, Picking, Scene, Shaders Advanced, Shaders, Stress Tests, Tools, Transforms, UI, Usage, Window, glTF. Page URL pattern `/examples/<category>/<example>/` (e.g. `/examples/animation/animated-mesh/`). The page names no Bevy version.
- Source of truth is the repo `examples/` directory (checkout top-level dirs: `2d 3d animation app asset async_tasks audio camera dev_tools diagnostics ecs gizmos gltf helpers input large_scenes math mobile movement no_std picking reflection remote scene shader shader_advanced showcase state stress_tests testbed time tools transforms ui usage wasm window`, verified with `ls examples`). Prefer the checkout over the web build: the web index lags the tag.
- Directly useful in the checkout: `examples/gltf/` (load_gltf, gltf_skinned_mesh, load_gltf_extras, query_gltf_primitives, update_gltf_scene, edit_material_on_gltf, custom_gltf_vertex_attribute, gltf_extension_animation_graph, gltf_extension_mesh_2d), `examples/animation/` (animated_mesh, animated_mesh_control, animated_mesh_events, animation_graph, animation_masks, animation_events, morph_targets, custom_skinned_mesh), `examples/shader/` (extended_material, extended_material_bindless, array_texture, shader_defs, gpu_readback, ...), `examples/app/headless_renderer.rs` and `externally_driven_headless_renderer.rs`, `examples/asset/` (compressed_image_saver, asset_settings, hot_asset_reloading, processing/).
- Shaders in `assets/shaders/` are `.wesl` (44 files with that extension in the checkout; `ls assets/shaders | grep -c wesl`). See the WESL row below before writing any shader.

## Bevy Assets (`https://bevy.org/assets/`)

Community directory of third-party plugins (page text is 150k characters; the first 100k were read, later categories Testing, UI, Writing, Apps were not). Items relevant to our pipeline, as listed on the page:
- 3D: `bevy_gltf_draco` Draco decoder for Bevy's glTF loader https://github.com/jiangheng90-opensource/bevy_gltf_draco ; `bevy_toon_shader` https://github.com/tbillington/bevy_toon_shader ; `bevy_wind_waker_shader` https://github.com/janhohenheim/bevy_wind_waker_shader ; `bevy_obj`, `bevy_stl`, `bevy_vox`, `bevy_svg` loaders.
- Animation: `bevy_gltf_animation` (identifies and plays glTF animations) https://github.com/dsgallups/bevy_gltf_animation ; `bevy_spritesheet_animation` https://github.com/merwaaan/bevy_spritesheet_animation ; `bevy_open_vat` (vertex animation textures) https://github.com/hk416/bevy_open_vat ; `bevy_tweening`, `bevy_tween`, `bevy_easings`, `bevy_spine`, `bevy_aseprite_ultra`.
- Asset loading: `bevy_asset_loader` (collections during a state) https://crates.io/crates/bevy_asset_loader ; `bevy_common_assets` (JSON/YAML/RON/TOML/MessagePack) https://github.com/NiklasEi/bevy_common_assets ; `bevy_embedded_assets` https://crates.io/crates/bevy_embedded_assets ; `bevy_web_asset` ; `bevy_titan` and a heterogeneous texture atlas loader (RON manifests).
- Dev tools: `skein` and `bevy_gltf_components` / `blender_bevy_components_workflow` (components in glTF extras), `shadplay`, `bevy_shadertoy_wgsl`.
- No KTX2 item was seen on the page. No meshopt item was seen. `bevy_kira` did not appear in the part read.

## Release 0.20 headlines relevant to us (`https://bevy.org/news/bevy-0-20/`, 2026-10-08)

| Feature | Note |
|---|---|
| Better Texture Compression `#better-texture-compression` | `CompressedImageSaver` new `ctt` backend outputs BCn or ASTC KTX2 with mipmaps; old Basis path is feature `compressed_image_saver_universal` (code: `crates/bevy_image/src/compressed_image_saver/mod.rs`) |
| WESL Shaders `#wesl-shaders` | shaders are WESL (WGSL superset with modules, imports, conditionals); naga_oil dialect removed |
| 2D Extended Materials `#2d-extended-materials`, Sprite Materials `#sprite-materials` | `MaterialExtension2d`; sprites render through Mesh2d plus a sprite material (`#sprite-render-backend-unification`) |
| Mesh Shaders `#mesh-shaders` | base support, not on web |
| `chain_weak` `#weak-system-ordering-with-chain-weak` | built-in render schedules order sets weakly |
| BSN scenes `#bsn-syntax-improvements`, `#ready-event` | `bevy_scene` is BSN; old scenes are `bevy_world_serialization` |
| UI/text `#val-em-and-val-rem`, `#inlinebox-and-inlineimage`, `#fixednode`, `#elliptical-border-radius` | em/rem units, inline boxes, fixed nodes, per-axis radii |

The post says nothing on glTF, KTX2 loader changes, animation, extract crate or screenshots (checked in the fetched text); those facts come from the code and the migration guide.

## Migration 0.19 to 0.20 (`https://bevy.org/learn/migration-guides/0-19-to-0-20/`)

| Change | Migration note | Anchor |
|---|---|---|
| WESL shaders | naga_oil removed; rename `.wgsl` to `.wesl`; `import a::b::C;` first in file, `@if(FLAG)`, `constants::NAME`; plain WGSL with no preprocessor keeps working; `#define_import_path` gone; modules follow crate file paths (`bevy_pbr::render::mesh_view_bindings`); GLSL removed; `shader_format_wesl` feature gone (local draft `_release-content/migration-guides/shaders_use_wesl.md`) | `#wesl-shaders` |
| CompressedImageSaver improvements | Basis UASTC -> `compressed_image_saver_universal`; `compressed_image_saver` is ctt BCn/ASTC; jpeg joins the default processor; `ImagePlugin::default_compressed_image_processor_extensions`; new `CompressionFailed` error (draft `compressed_image_saver.md`) | `#compressedimagesaver-improvements` |
| UnpreparedBindGroup -> BindGroupBuilder | manual `AsBindGroup` impls rename the method to `build_bind_group`, write to `output` | `#unpreparedbindgroup-is-now-bindgroupbuilder` |
| Extraction generic over worlds | `#[extract_app(RenderApp)]`, `SyncComponent<RenderApp>`, `extract()` moved to `bevy_extract` | `#extraction-is-now-generic-over-worlds` |
| ShaderBuffer reworked | `data` is `ShaderBufferData`; `new` takes `Vec<T>`; `set_data`, `resize_in_place` removed; `with_size(u64)` | `#shaderbuffer-stores-typed-data-in-an-alignedvec-with-an-explicit-buffer-size` |
| WgpuWrapper removed | `RenderQueue`, `RenderAdapter` etc. use `new`/`into_inner` | `#wgpuwrapper-has-been-removed` |
| Window data in ECS (render world) | `ExtractedWindows`, `ExtractedWindowSurfaces` replaced by queries; `RenderAppChannels::new` takes a `MainThreadExecutor` | `#use-ecs-for-render-world-window-data`, `#renderappchannels-new-requires-a-mainthreadexecutor` |
| Tonemapping / DebandDither moved | now `bevy_render::view`; `Tonemapping::None` is full passthrough, use `Tonemapping::Linear` for the old look; deband on by default for `Camera3d` (changes PNG output) | `#tonemapping-and-debanddither-moved-to-bevy-render-view`, `#tonemapping-none-is-now-a-full-passthrough` |
| Opt-in ScreenSpaceTransmission | add `ScreenSpaceTransmission` to `Camera3d` for translucent scenes | `#opt-in-screenspacetransmission` |
| Depth/stencil attachment changes | `DepthAttachment` -> `DepthStencilViewAttachment`, `ViewDepthTexture` -> `ViewDepthStencilTexture` | `#changes-have-been-made-to-depthattachment-viewdepthtexture-and-viewprepasstextures-depth-to-accommodate-stencil-support` |
| SortedCamera::hdr removed | camera indices count per render target; upper camera blit auto-detects alpha blending | `#sortedcamera-hdr-removed-camera-indices-count-per-render-target` |
| OIT changes | gate `oit_draw` on `MATERIAL_OIT_ENABLED` | `#order-independent-transparency-changes-to-support-premultiplied-alpha-modes-and-per-material-opt-out` |
| Sprite on Mesh2d + SpriteMaterial | `Sprite.alpha_mode` default `Blend`; draw order at equal Z may change; `SpriteMaterial` -> `SpriteMeshMaterial` | `#migrate-sprite-to-use-mesh2d-spritematerial`, `#spritematerial-and-spritematerialplugin-rename` |
| Built-in schedules order sets weakly | add explicit `.after()` for dependencies not tracked by ECS access | `#several-built-in-schedules-now-order-their-system-sets-weakly` |
| Asset API | boxed error variants (`AssetLoadError::RequestedHandleTypeMismatch`), `AssetId::invalid()` deprecated, `UntypedHandle::meta_transform` deleted | `#some-large-error-variants-are-boxed`, `#assetid-invalid-and-assetid-invalid-uuid-have-been-deprecated`, `#meta-transform-accessor-on-untypedhandle-has-been-deleted` |
| UI | retained UI rendering (`#ui-rendering-is-now-retained`); `ui_widgets::Button` replaces `ui::widgets::Button`; `Val::Em/Rem`; `BorderRadius` 2D; `TextFont` default size `FontSize::Rem(1.)`; `EditableText` + `TextInput` split; `Font::from_bytes` drops the family name | anchors in the page |

Not covered by the guide text read (sections 1-75 all read): glTF and animation have no 0.20 migration entries. Local draft check: `grep -il "gltf\|animation\|screenshot\|AssetProcessor\|ExtendedMaterial" _release-content/migration-guides/*.md` found only bind_group_builder, downsample_shader_specialization, shaders_use_wesl (animation/gltf: none).

## Migration 0.18 to 0.19 (`https://bevy.org/learn/migration-guides/0-18-to-0-19/`, 103 sections; the last 33k characters were only partly readable)

| Change | Migration note | Anchor |
|---|---|---|
| Render graph as systems | `ViewNode` -> systems taking `ViewQuery` and `RenderContext`, ordered with `.before()/.after()`, added to the `Core3d` or `Core2d` schedule | `#render-graph-as-systems` |
| New crate `bevy_material` | `AlphaMode`, `SpecializedMeshPipelineError`, `OpaqueRendererMethod` imports move | `#new-crate-bevy-material` |
| `ShaderStorageBuffer` -> `ShaderBuffer` | rename, also `GpuShaderBuffer` | `#shaderstoragebuffer-renamed-to-shaderbuffer` |
| `PipelineCacheError` -> `ShaderCacheError` | `ProcessShaderError` variant boxed | `#pipelinecacheerror-renamed-to-shadercacheerror` |
| Camera format rework, `Hdr` moved to `bevy_camera` | read `ExtractedCamera::hdr`, `ExtractedView::target_format` | `#camera-textureformat-rework`, `#hdr-moved-to-bevy-camera` |
| bevy_gltf/bevy_pbr dependency inverted | material sub-assets load as `GltfMaterial`; add the `/std` label suffix for `StandardMaterial`; `GltfExtensionHandler::on_material` takes `&GltfMaterial` + `material_label`; `UvChannel` moves to `bevy_mesh`; `SceneRoot` -> `WorldAssetRoot` | `#invert-bevy-gltf-dependency-with-bevy-pbr`, `#the-old-bevy-scene-is-now-bevy-world-serialization` |
| Morph targets stored in meshes | `GltfAssetLabel::MorphTarget` -> `GltfAssetLabel::Primitive`, `Mesh::get_morph_targets` | `#morph-targets-are-now-stored-in-meshes` |
| `AnimationTargetId` algorithm changes | recompute serialized ids | `#animationtargetid-algorithm-changes` |
| AssetServer load builder | `load()` simple, `load_builder()` advanced; `LoadContext::loader` -> `load_builder`; `NestedLoader` -> `NestedLoadBuilder` | `#advanced-assetserver-load-variants-are-now-exposed-through-a-builder-pattern` |
| `AssetSaver::save` takes `AssetPath`, `SavedAsset<'_, '_, T>` | update signatures | `#savedasset-now-contains-two-lifetimes-and-assetsaver-now-takes-an-assetpath` |
| `Reader::seekable` required | `AsyncSeekForward` deleted | `#implementations-of-reader-now-must-implement-reader-seekable-and-asyncseekforward-is-deleted` |
| `Assets::get_mut` returns `AssetMut` | `AssetEvent::Modified` only on real mutation | `#avoiding-unnecessary-assetevent-modified-events-that-lead-to-rendering-performance-costs` |
| `AssetPath::resolve*` | take `&AssetPath`; `resolve_str`, `resolve_embed_str` for strings | `#assetpath-resolve-and-resolve-embed-now-take-assetpath` |
| bevy_image | `Image::pixel_bytes*` return `Result<_, TextureAccessError>`; `DataFormat` -> `TextureChannelLayout` | `#image-pixel-bytes-and-image-pixel-data-offset-now-return-result`, `#dataformat-renamed-to-texturechannellayout` |
| Immutable resources | generic code needs `Resource<Mutability = Mutable>` bound | `#immutable-resources` |
| Text | Cosmic Text -> Parley; `TextFont.font` is `FontSource`, `font_size` is `FontSize::Px(..)`; `Font::try_from_bytes` -> `from_bytes` | `#bevy-text-migration-from-cosmic-text-to-parley`, `#changes-to-textfont-s-font-size-and-font-fields` |
| Features | `ui` not implied by `2d`/`3d`; `experimental_ui_widgets` -> `bevy_ui_widgets`; `bevy_window`, `bevy_input_focus`, `custom_cursor` moved out of `default_app` | anchors in the page |

## Migration 0.17 to 0.18 (`https://bevy.org/learn/migration-guides/0-17-to-0-18/`, 64 sections, page not truncated)

| Change | Migration note | Anchor |
|---|---|---|
| `RenderTarget` is a component | spawn `RenderTarget::..` next to the camera instead of setting `camera.target` (affects offscreen capture) | `#rendertarget-is-now-a-component` |
| `ImageRenderTarget.scale_factor` is `f32` | drop `FloatOrd` | `#imagerendertargets-scale-factor-field-is-now-an-f32` |
| Image loader array layout | use `ImageLoaderSettings::array_layout` | `#image-loader-array-layout` |
| `Image::reinterpret_*` return `Result` | handle errors | `#image-reinterpret-size-and-image-reinterpret-stacked-2d-as-array-now-return-a-result` |
| glTF coordinate conversion | `use_model_forward_direction` -> `convert_coordinates` (`rotate_scene_entity`, `rotate_meshes`) | `#gltf-coordinate-conversion` |
| Animation features and targets | feature `animation` -> `gltf_animation`; `AnimationTarget` -> `(AnimationTargetId, AnimatedBy(player))`; `AnimationEventTrigger::animation_player` -> `target` | `#feature-cleanup`, `#animationtarget-replaced-by-separate-components` |
| Asset plumbing | `LoadContext::path()` returns `AssetPath`; custom sources need a reader (`AssetSourceBuilder::new(reader)`); `AssetProcessor::new` returns `(processor, Arc<AssetSources>)`; `AssetPlugin.use_asset_processor_override`; loaders/savers/`Process` need `TypePath`; `ProcessContext` has no `asset_bytes` | `#loadcontext-path-now-returns-assetpath`, `#custom-asset-sources-now-require-a-reader`, `#changes-to-assetserver-and-assetprocessor-creation`, `#traits-assetloader-assettransformer-assetsaver-and-process-all-now-require-typepath` |
| Material methods | `enable_prepass`, `enable_shadows` are `Material` (and `MaterialExtension`) trait methods | `#enable-prepass-and-enable-shadows-are-now-material-methods` |
| BindGroupLayout | labels required (`AsBindGroup::label()`); pipeline descriptors hold `BindGroupLayoutDescriptor` | `#bindgrouplayout-labels-are-no-longer-optional`, `#renderpipelinedescriptor-and-computepipelinedescriptor-now-hold-a-bindgrouplayoutdescriptor` |
| Aabb auto-update | sprites and meshes; opt out with `NoAutoAabb` | `#automatic-aabb-updates-for-sprites-and-meshes` |
| `BorderRadius` | a `Node` field | `#borderradius-has-been-added-to-node-and-is-no-longer-a-component` |

## Consequences for our GLB pipeline

- Custom shaders must be WESL-compatible: start from the checkout's `assets/shaders/extended_material.wesl` and `examples/shader/extended_material.rs`; imports use `import bevy_pbr::...;` paths (module paths changed, see the WESL row).
- Offscreen capture on 0.20 spawns `RenderTarget` as a component (0.18) and the render world uses schedules, not a render graph (0.19); older tutorials with `camera.target` or `RenderGraph` nodes do not apply.
- Output PNGs depend on tonemapping and deband defaults changed in 0.20 (`Tonemapping::None` passthrough, `DebandDither` default on for `Camera3d`): pin them explicitly for deterministic test renders.
- Texture compression naming changed in 0.20: UASTC `.basis` output is `compressed_image_saver_universal`; BCn/ASTC KTX2 is `compressed_image_saver`.
