# Bevy assets (bevy_asset), processing, `.meta`

Pin: bevyengine/bevy `a0577b5` (0.20.1). `path:line` = file at that commit, URL form `https://github.com/bevyengine/bevy/blob/a0577b5c6a7b208719dd761c5c6a7a539e472a97/<path>#L<line>`. Snippets "not compiled" (the lead compiles on fr1). Unless noted, paths are under `crates/bevy_asset/`.

## AssetServer, handles, labels
- `AssetServer::load::<A>(path) -> Handle<A>` src/server/mod.rs:364 (non-blocking, returns the existing handle if already loaded, mod.rs:1784-1790). Keep the handle: dropping the last strong handle unloads the asset (`#[must_use]` mod.rs:363).
- 0.19 removed `load_with_settings` / `load_acquire` (no such symbols in the tree: `grep` finds only internal `load_with_settings_loader_and_reader`, server/mod.rs:690,1481). Use the builder: `asset_server.load_builder().with_settings(|s: &mut S| ..).override_unapproved().with_guard(g).load::<A>(path)`; `LoadBuilder` src/server/mod.rs:1705, `with_settings` :1737, `override_unapproved` :1757, `with_guard` :1770 (guard dropped when the load completes or fails; example examples/asset/multi_asset_sync.rs). Also `load_erased`, `load_untyped` (-> `Handle<LoadedUntypedAsset>`), `NestedLoadBuilder` for loaders (src/loader_builders.rs:32). Migration 0.18->0.19 "Advanced AssetServer load variants are now exposed through a builder pattern" (https://bevy.org/learn/migration-guides/0-18-to-0-19/).
- Other: `load_folder` -> `Handle<LoadedFolder>` :947 (src/folder.rs:12), `reload` :776, `add(asset)` :832, `add_async` :878, `get_handle` :1226.
- State: `get_load_state` :1075, `load_state` :1123, `recursive_dependency_load_state` :1140, `is_loaded` :1149, `is_loaded_with_dependencies` :1163; `enum LoadState { NotLoaded, Loading, Loaded, Failed(Arc<AssetLoadError>) }` :2114-2128. Events: `AssetEvent::{Added, Modified, Removed, Unused, LoadedWithDependencies}` src/event.rs:50-61. `Assets<A>` src/assets.rs:288 (`add` :401); `Handle<A>` enum src/handle.rs:115, `UntypedHandle` :456.
- Paths: `source://path/file.ext#Label`; label = named sub-asset, `#` delimiter, empty label is an error (src/path.rs:23-24,94-96,103-118). Misspelled glTF labels are silently ignored (crates/bevy_gltf/src/lib.rs:85-91).
- glTF labels (crates/bevy_gltf/src/label.rs:33-66, Display :68-93): `Scene{n}` -> `WorldAsset`, `Node{n}`, `Mesh{n}`, `Mesh{n}/Primitive{m}` -> `Mesh`, `Texture{n}` -> `Image`, `Material{n}` -> `GltfMaterial` (suffix `" (inverted)"` when scale inverted), `DefaultMaterial`, `Animation{n}` -> `AnimationClip`, `Skin{n}`, `Skin{n}/InverseBindMatrices`. Typed builder: `GltfAssetLabel::Scene(0).from_asset("models/x.glb")` (label.rs:112). A `StandardMaterial` for a glTF material is the sub-label `Material{n}/std` (crates/bevy_pbr/src/gltf.rs:132,149; default `DefaultMaterial/std` :116); migration: https://bevy.org/learn/migration-guides/0-18-to-0-19/ ("Invert bevy_gltf dependency with bevy_pbr"). Scenes: `bevy_scene` became `bevy_world_serialization`; spawn with `WorldAssetRoot(handle)` (crates/bevy_world_serialization/src/components.rs:23) and observe `WorldInstanceReady` (world_asset_spawner.rs:33).
- Wait for GLB: poll `is_loaded_with_dependencies(&handle)` or read `AssetEvent::LoadedWithDependencies`; for entities, `WorldInstanceReady`.

## AssetPlugin
Fields src/lib.rs:235-262: `file_path` (default `"assets"`, :346), `processed_file_path` (default `"imported_assets/Default"`, :349; the processor log is `imported_assets/log`, src/processor/log.rs:119), `watch_for_changes_override: Option<bool>`, `use_asset_processor_override: Option<bool>`, `mode: AssetMode`, `meta_check: AssetMetaCheck`, `unapproved_path_mode: UnapprovedPathMode`.
- `AssetMode::{Unprocessed, Processed}` :295-315. `Processed` reads from `imported_assets`; it assumes the processor already ran unless cargo feature `asset_processor` (Cargo.toml:20; root Cargo.toml:664) is on, in which case `AssetProcessor::start` runs at Startup and the server shares its loaders (lib.rs:384-400, processor/mod.rs:254). Without the feature the server just reads processed output (lib.rs:401-411). Processed mode hard-codes `AssetMetaCheck::Always` (lib.rs:395,405).
- Web warning: preprocessing in the browser is problematic (no filesystem), lib.rs:292-293 (bevy#10157). Ship pre-baked output or Unprocessed.
- `AssetMetaCheck::{Always (default), Paths(set), Never}` lib.rs:321-329: `Never` skips `.meta` lookups (avoids 404s on web); default meta used.
- `UnapprovedPathMode::{Allow, Deny, Forbid (default)}` lib.rs:277-286: approved = `file_path` and each AssetSource folder; `Deny` still allows `LoadBuilder::override_unapproved`. Loading `/abs/path.glb` needs `Allow`/`override_unapproved`.
- Watching/hot reload: cargo feature `file_watcher` (src Cargo.toml:14, root Cargo.toml:667) or `watch_for_changes_override`; embedded assets need `embedded_watcher` (embedded/mod.rs:345-350). With `asset_processor` + `file_watcher`, source changes re-process then reload (lib.rs:308-310). Example examples/asset/hot_asset_reloading.rs.

## Sources, readers, embedded
- `AssetSource` / `AssetSourceBuilder` src/io/source.rs:119,390; `with_reader/with_writer/with_watcher` (+`with_processed_*`) :229-277; `platform_default(path, processed_path)` :303. Register: `app.register_asset_source("name", AssetSourceBuilder::platform_default("dir", None))` BEFORE `DefaultPlugins` (examples/asset/extra_source.rs:19-21; trait fn lib.rs:580), load with `name://file.glb`. Custom reader: examples/asset/custom_asset_reader.rs:17. Web readers: `HttpWasmAssetReader` src/io/wasm.rs:32 (403/404 -> NotFound :115), `WebAssetPlugin` io/web.rs:62 (cargo `http`/`https`, Cargo.toml:17-18).
- Embedded: `embedded_asset!(app, "src_dir/", "file")` embeds bytes at compile time (src/io/embedded/mod.rs:352-363); `load_embedded_asset!`, `embedded_path!` :205,228; path `embedded://crate/file`. WESL/shader import-by-asset-path change in 0.20: https://bevy.org/learn/migration-guides/0-19-to-0-20/#wesl-shaders.
- Extension registration: `init_asset::<A>()`, `register_asset_loader(L)`, `preregister_asset_loader`, `register_asset_processor`, `set_default_asset_processor::<P>("ext")` (lib.rs:573-606).

## Loader, saver, transformer, processor
- `AssetLoader { type Asset; type Settings: Default+Serialize+Deserialize; type Error: Into<BevyError>; fn load(&self, reader: &mut dyn Reader, settings, &mut LoadContext) -> impl Future; fn extensions() }` src/loader.rs:32-52. Example (gzip decompression wrapping any loader via `LoadContext::loader()...`): examples/asset/asset_decompression.rs:43,85.
- `AssetSaver { Asset, Settings, OutputLoader, Error; fn save(&self, writer, SavedAsset, settings, asset_path: AssetPath) -> Result<OutputLoader::Settings> }` src/saver.rs:35-56 (0.19: `SavedAsset` has two lifetimes, `save` takes `AssetPath`). `AssetTransformer` src/transformer.rs:28.
- `Process` trait src/processor/process.rs:31 (low level); `LoadTransformAndSave<L, T, S>` :66 (high level; `From<S>` gives identity transform :76); `AssetProcessor` (Resource) src/processor/mod.rs:98, `register_processor` :747, `set_default_processor` :781, `wait_until_finished` :1387. A hash of source+meta decides re-processing; a write-ahead transaction log makes it crash safe (struct doc processor/mod.rs:84-97, log.rs:119).
- Default meta when none exists: default processor for the full extension if set, else the registered loader's default meta, else `AssetAction::Ignore` (processor/mod.rs:1075-1095). A `.glb` has the glTF loader (`extensions` `["gltf","glb"]`, crates/bevy_gltf/src/loader/mod.rs:1195-1197) and NO default processor, so processing just copies it to `imported_assets/Default`.
- Images: `ImagePlugin` registers `LoadTransformAndSave<ImageLoader, IdentityAssetTransformer<Image>, CompressedImageSaver>` and makes it the default for `default_compressed_image_processor_extensions` = `["png","jpeg","jpg"]` when feature `compressed_image_saver` or `compressed_image_saver_universal` is on and the processor exists (crates/bevy_image/src/image.rs:175-190,238-258). Not for `.ktx2`.
- Processor examples: examples/asset/processing/asset_processing.rs:31 (mode Processed), :62-65 (register + default per extension), `a.cool.ron.meta`; compressed textures: examples/asset/compressed_image_saver.rs:20 (required features `compressed_image_saver asset_processor jpeg`, Cargo.toml:2144).

## `.meta` format (RON)
- `AssetMeta { meta_format_version: "1.0" (src/meta.rs:31), asset: AssetAction, processed_info }`; `AssetAction::{Load{loader, settings}, Process{processor, settings}, Ignore}` meta.rs:70-87. File = `<asset path>.meta` next to the asset (examples/asset/files/bevy_pixel_dark_with_meta.png.meta).
- Loader meta (verbatim from the example): `loader: "bevy_image::image_loader::ImageLoader"`, settings `format: FromExtension, is_srgb: true, sampler: Descriptor (ImageSamplerDescriptor(...)), asset_usage: RenderAssetUsages("RENDER_WORLD | MAIN_WORLD")`. Process meta: `processor: "LoadTransformAndSave<CoolTextLoader, CoolTextTransformer, CoolTextSaver>"` with `settings: (loader_settings: (), transformer_settings: (..), saver_settings: ())` (examples/asset/processing/assets/a.cool.ron.meta). Processor names use short or long type paths (`MetaTypePathKind` src/processor/process.rs:252).
- The processor writes default `.meta` files into the source folder for every loadable asset (processor/mod.rs docs); edit those rather than hand-writing. Per-load override at runtime: `load_builder().with_settings`.

## 0.20 migration changes touching assets (https://bevy.org/learn/migration-guides/0-19-to-0-20/)
- `compressed_image_saver` now uses `ctt` (BCn/ASTC); Basis Universal moved to `compressed_image_saver_universal` (recommended for web); JPEG joins the default compressed processor; `ImagePlugin::default_compressed_image_processor_extensions` (checked above, image.rs:175).
- `AssetLoadError::RequestedHandleTypeMismatch` and `LoadDirectError::LoadError::error` boxed; `UntypedHandle::meta_transform` deleted; `AssetId::invalid()`/`INVALID_UUID` deprecated; `Font::from_bytes` drops family name; shaders: WESL import by path, `#define_import_path` removed.
- 0.19 (same-source guide): `Assets::get_mut` returns `AssetMut` (fires `Modified` only on real mutation); `Reader::seekable` required; `AssetPath::resolve` takes `&AssetPath`; `Image::pixel_bytes` returns `Result`.
- Local `_release-content/migration-guides/*.md` at the pin lists the guides merged for the release (compressed_image_saver.md, shader_buffer_changes.md, shaders_use_wesl.md, ...).

## Recipes for our GLB pipeline

### Load a GLB scene (not compiled)
```rust
fn load(mut commands: Commands, assets: Res<AssetServer>) {
    let scene = assets.load(GltfAssetLabel::Scene(0).from_asset("models/hero.glb"));
    commands.spawn(WorldAssetRoot(scene));
    let walk: Handle<AnimationClip> = assets.load(GltfAssetLabel::Animation(0).from_asset("models/hero.glb"));
}
```
Clip selection by name lives in animation.md (names come from the glTF `animations[].name`, not from labels).

### `.meta` for a `.glb` (not parsed; the checkout has no glTF meta sample)
`assets/models/hero.glb.meta`:
```ron
(
    meta_format_version: "1.0",
    asset: Load(
        loader: "bevy_gltf::loader::GltfLoader",
        settings: (
            load_cameras: false,
            load_lights: false,
            load_animations: true,
            include_source: false,
            validate: true,
        ),
    ),
)
```
Fields from `GltfLoaderSettings` (`#[serde(default)]`, so omitted fields default) crates/bevy_gltf/src/loader/mod.rs:186-224, defaults :226-243: `load_meshes`/`load_materials: RenderAssetUsages`, `load_cameras`, `load_lights`, `load_animations`, `include_source`, `default_sampler`, `override_sampler`, `validate`, `convert_coordinates`, `skinned_mesh_bounds_policy`, `mesh_compression: Option<MeshCompressionArgs>` (bevy_mesh/src/mesh.rs:274). The loader string is the `TypePath` of `GltfLoader` (derive at loader/mod.rs:143-144); unverified: confirm by letting the processor generate one (Processed + `asset_processor`) and copy it.

### `.meta` for a `.ktx2`
`assets/textures/hero_albedo.ktx2.meta` (modelled on the png example above; `Format(Ktx2)` unverified against a RON round trip):
```ron
(
    meta_format_version: "1.0",
    asset: Load(
        loader: "bevy_image::image_loader::ImageLoader",
        settings: (
            format: FromExtension,
            is_srgb: true,
            sampler: Default,
            asset_usage: RenderAssetUsages("RENDER_WORLD"),
        ),
    ),
)
```
`ImageLoaderSettings` fields crates/bevy_image/src/image_loader.rs:134-164 (`format: FromExtension|Format(ImageFormat)|Guess` :90-101, `texture_format`, `is_srgb`, `sampler: Default|Descriptor(..)` image.rs:697-703, `asset_usage`, `array_layout`, `expand_grayscale`). `asset_usage: RENDER_WORLD` frees the CPU copy after the first frame, `RENDER_WORLD | MAIN_WORLD` keeps it (crates/bevy_asset/src/render_asset.rs:14-17). Needs cargo feature `ktx2` (image.rs:293, root Cargo.toml:491; in the `3d_api` collection, root Cargo.toml:236) plus `zstd_rust`/`zstd_c` for supercompressed files; `basis-universal` for `.basis` (root Cargo.toml:456). See image-ktx2.md. glTF caveat: Bevy supports the ktx2 image format but not the `KHR_texture_basisu` extension syntax (crates/bevy_gltf/src/lib.rs:122,129); a GLB whose textures only reference `.ktx2` through that extension will not bind them. Keep a fallback `source` image or retarget materials to external `.ktx2` handles.

### Prepare assets for the web (no processor in the browser)
1. Bake in the pipeline, not in the browser: export GLB + KTX2 (assets-lab), commit `assets/`.
2. Either (a) `AssetPlugin { meta_check: AssetMetaCheck::Never, ..default() }` with `AssetMode::Unprocessed` (lib.rs:321-329, 292-293), or (b) on fr1 run once with `AssetMode::Processed` + features `asset_processor` (+ `compressed_image_saver_universal` for web-safe Basis/KTX2 output), then ship `imported_assets/Default` and run the web build with `AssetMode::Processed` and no `asset_processor` feature (lib.rs:401-411). Processed mode still fetches `.meta` (meta check forced `Always`, lib.rs:405).
3. Do not set `UnapprovedPathMode::Allow` for user-supplied paths (lib.rs:269-271).

### Custom loader: skip
A glTF-to-glTF processor needs a glTF saver (none in Bevy). Encode textures to KTX2 upstream and let `GltfLoader` load the plain result; do not feed it meshopt or Draco output, Bevy cannot read `EXT_meshopt_compression` or `KHR_draco_mesh_compression` (crates/bevy_gltf/src/lib.rs:102,126). Community crate `bevy_gltf_draco` adds Draco loading (https://bevy.org/assets/, unverified against 0.20.1).
