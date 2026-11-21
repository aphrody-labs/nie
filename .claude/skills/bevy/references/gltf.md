# bevy_gltf (Bevy 0.20.1)

Pin: `bevyengine/bevy` @ `a0577b5` (Release 0.20.1). Cites `path:line` are in that tree (`https://github.com/bevyengine/bevy/blob/a0577b5/<path>#L<line>`). Verdicts come from reading the code and the loader doc table; the loader was NOT run against test files (a fr1 probe build was started and abandoned) and no snippet here was compiled.

## Verdict for our export choices (exact)
| Feature | Bevy 0.20.1 | Evidence |
|---|---|---|
| KTX2 image decoding (`image/ktx2`, features `ktx2` + `zstd_rust`/`zstd_c`, optional `basis-universal`) | yes, as an image format | `crates/bevy_image/src/image.rs:398,452,489` |
| `KHR_texture_basisu` glTF extension | NO (extension syntax ignored; table row "no*") | `crates/bevy_gltf/src/lib.rs:122,129`; issue https://github.com/bevyengine/bevy/issues/19104 |
| `EXT_meshopt_compression` | NO | `crates/bevy_gltf/src/lib.rs:126` |
| `KHR_draco_mesh_compression` | NO | `crates/bevy_gltf/src/lib.rs:102` |
| `KHR_mesh_quantization` (gltfpack emits it by default) | NO | `crates/bevy_gltf/src/lib.rs:118` |
| `KHR_texture_transform` | base color only | `crates/bevy_gltf/src/lib.rs:123,131`; issue https://github.com/bevyengine/bevy/issues/15310 |
| Skeleton animation (translation, rotation, scale, morph weights; linear, step, cubic spline) | yes | `crates/bevy_gltf/src/loader/mod.rs:306-607` |
Practical rule: export plain glTF 2.0 GLB (embedded PNG/JPEG, or a GLB whose `images[].mimeType` is `image/ktx2` on the core `textures[].source`), uncompressed accessors, no required extensions. Meshopt/Draco/basisu-only files must be decoded back (`gltfpack -noq`, `gltf-transform`) or loaded with a custom `GltfExtensionHandler` plus own decoder (not shipped).

## Supported extensions (upstream table, `crates/bevy_gltf/src/lib.rs:93-133`)
Supported: `KHR_lights_punctual`, `KHR_materials_anisotropy` (feature `pbr_anisotropy_texture` for the texture), `KHR_materials_clearcoat` (`pbr_multi_layer_material_textures`), `KHR_materials_emissive_strength`, `KHR_materials_ior`, `KHR_materials_specular` (`pbr_specular_textures`), `KHR_materials_transmission` (`pbr_transmission_textures`), `KHR_materials_unlit`, `KHR_materials_volume`, `KHR_texture_transform` (base color only). Features are declared at `crates/bevy_gltf/Cargo.toml:13-16`, the `gltf` parser crate is pinned `1.4.0` with only `KHR_lights_punctual`, `KHR_materials_{transmission,ior,specular,volume,unlit,emissive_strength}`, `KHR_texture_transform`, `extras`, `extensions`, `names`, `utils` (`Cargo.toml:47-58`; resolved 1.4.1 on fr1 has no basisu/webp/meshopt/draco support in `gltf-json`).
Not supported (table rows): `KHR_animation_pointer`, `KHR_draco_mesh_compression`, `KHR_gaussian_splatting`, `KHR_interactivity`, `KHR_materials_dispersion`, `KHR_materials_iridescence`, `KHR_materials_sheen`, `KHR_materials_variants`, `KHR_mesh_quantization`, `KHR_node_hoverability`, `KHR_node_selectability`, `KHR_node_visibility`, `KHR_texture_basisu`, `KHR_xmp_json_ld`, `EXT_mesh_gpu_instancing`, `EXT_meshopt_compression`, `EXT_texture_webp` (`lib.rs:101-127`).
Parsing of the three material extensions: `crates/bevy_gltf/src/loader/extensions/khr_materials_{anisotropy,clearcoat,specular}.rs`, called at `loader/mod.rs:1418-1426`. Emissive strength `:1432`, IOR `:1416`, unlit `:1474`.

## Loading model
- Loader: `GltfLoader` (`loader/mod.rs:144`), `impl AssetLoader` at `:1179` for `gltf`, `glb`. Plugin `GltfPlugin` (`lib.rs:224`) registers it in `finish` and reads `CompressedImageFormatSupport` to decide which compressed texture formats it may keep (`lib.rs:287-296`); without the resource (no `RenderPlugin`) it warns and uses `CompressedImageFormats::NONE`, which makes KTX2/Basis transcode to RGBA8.
- Output `Gltf` (`assets.rs:18`): `scenes`, `named_scenes`, `meshes`, `named_meshes`, `materials`, `named_materials`, `nodes`, `named_nodes`, `skins`, `named_skins`, `default_scene`, `animations`, `named_animations: HashMap<Box<str>, Handle<AnimationClip>>` (feature `bevy_animation`), `source` (only with `include_source`). Scenes are `Handle<WorldAsset>`; spawn with `WorldAssetRoot` (`lib.rs:70-79` doc example).
- Labels (`label.rs`): `GltfAssetLabel::{Scene(i), Node(i), Mesh(i), Primitive{mesh,primitive}, Texture(i), Material{index,is_scale_inverted}, DefaultMaterial, Animation(i), Skin(i), InverseBindMatrices(i)}` rendered as `Scene0`, `Animation0`, ...; misspelled labels are silently ignored (`lib.rs:84-90`). Use `GltfAssetLabel::Scene(0).from_asset("m.glb")`.
- `GltfLoaderSettings` (`loader/mod.rs:188`): `load_meshes`, `load_materials` (`RenderAssetUsages`; drop `MAIN_WORLD` copies to save RAM), `load_cameras`, `load_lights`, `load_animations`, `include_source`, `default_sampler`, `override_sampler`, `validate` (default true; false calls `Gltf::from_slice_without_validation`, `:253-257`), `convert_coordinates`, `skinned_mesh_bounds_policy`, `mesh_compression`. Set per load with `asset_server.load_builder().with_settings(|s: &mut GltfLoaderSettings| ...).load(path)` (`crates/bevy_asset/src/server/mod.rs:1737`); `load_with_settings` no longer exists in 0.20.
- Plugin-level: `default_sampler` (linear), `convert_coordinates`, `custom_vertex_attributes` (names must start with `_`, `lib.rs:259-269`), `skinned_mesh_bounds_policy`, `mesh_compression` (default `MeshCompressionArgs::none()`, `lib.rs:247-256`; this is Bevy's own vertex quantization in `crates/bevy_mesh/src/mesh.rs:274`, unrelated to meshopt).
- Coordinates: `GltfConvertCoordinates { rotate_scene_entity, rotate_meshes }` (`convert_coordinates.rs:64-83`). Default converts nothing: a glTF that faces +Z looks away from Bevy's `-Z` forward unless you set it.

## Meshes, skins, textures
- Vertex data: POSITION, NORMAL, TANGENT, TEXCOORD_0/1, COLOR_0, JOINTS_0, WEIGHTS_0 and `_CUSTOM` attributes (`vertex_attributes.rs:162-234,310-315`); normalized u8/u16 joint weights and colours are converted to float (`:195-234`). Missing tangents are generated with mikktspace when a normal map exists (`loader/mod.rs:843-858`).
- Morph targets become `MeshMorphWeights` (`loader/mod.rs:800-817,1880-1890`). Skins: max `MAX_JOINTS = 256` (`:81`, check `:967`).
- Sparse accessors: only rejected in animation sampler input (`:325-327`); other paths read via the `gltf` crate reader.
- Textures: `load_image` (`:1201`) accepts a bufferView image (GLB embedded) with its mime type via `Image::from_buffer`, a base64 data URI, or an external URI which is loaded as an asset (sub-asset path, so `.meta`/processed assets apply; `:1217-1260`). Linear vs sRGB is decided per texture from material usage (`get_linear_textures`, `:280`). Samplers map from the glTF sampler (`gltf_ext/texture.rs`), overridden with `override_sampler`.
- Cameras and lights spawn with the scene (`:1587-1620`, `:1773-1830`).

## Animation import
Clips are built at `loader/mod.rs:306-607`: one `AnimationClip` per glTF animation, `Name` path targets per node, `named_animations` filled at `:589` with the glTF `animations[i].name`; unnamed clips are only in `animations`. See `animation.md` for graph, player and selection by name.

## Extension handlers (user extensions)
`GltfExtensionHandler` (`loader/extensions/mod.rs:62`) hooks: `on_root`, `on_animation`, `on_animations_collected`, `on_texture`, `on_material`, `on_gltf_primitive`, `on_gltf_mesh`, `on_spawn_mesh_and_material`, `on_scene_completed`, `on_gltf_node`, `on_spawn_light_{directional,point,spot}` (`:72-270`). Register in `GltfExtensionHandlers` (`:34`). Examples: `examples/gltf/gltf_extension_animation_graph.rs`, `examples/gltf/gltf_extension_mesh_2d.rs`. Hooks receive parsed extension JSON; they cannot replace the buffer decoding that happens before them, so Draco/meshopt need a pre-decode step in the exporter instead.

## Other examples in the pin
`examples/gltf/`: `load_gltf.rs`, `edit_material_on_gltf.rs`, `gltf_skinned_mesh.rs`, `load_gltf_extras.rs` (extras JSON on nodes/meshes/materials), `query_gltf_primitives.rs`, `update_gltf_scene.rs`, `custom_gltf_vertex_attribute.rs`, `gltf_extension_*`.

## Recipes for our GLB pipeline
Load and spawn (not compiled):
```rust
use bevy::{gltf::GltfLoaderSettings, prelude::*};
fn load(mut commands: Commands, assets: Res<AssetServer>) {
    let scene = assets
        .load_builder()
        .with_settings(|s: &mut GltfLoaderSettings| s.load_cameras = false)
        .load(GltfAssetLabel::Scene(0).from_asset("models/hero.glb"));
    commands.spawn(WorldAssetRoot(scene));
}
```
Exporter checklist: name every node and animation; one scene; no `extensionsRequired`; textures PNG/JPEG or `image/ktx2` core source; joints per skin <= 256; tangents baked if normal maps are used; keep `KHR_texture_transform` to base color; no `KHR_mesh_quantization` (if gltfpack is in the chain, pass `-noq`, no `-cc`, no `-tc`).
