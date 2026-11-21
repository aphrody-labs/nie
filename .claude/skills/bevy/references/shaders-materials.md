# Shaders and materials (Bevy 0.20.1)

Pin: `bevyengine/bevy` @ `a0577b5` (Release 0.20.1). Cites `path:line` are in that tree (`https://github.com/bevyengine/bevy/blob/a0577b5/<path>#L<line>`); a bare URL is a live web page.
Not compiled: snippets are written against the checkout's examples and read-verified only; the lead compiles the recipe on fr1.

## Shader language in 0.20: WESL, not naga_oil WGSL
- All Bevy shaders are WESL (`.wesl`); the naga_oil preprocessor (`#import`, `#ifdef`, `#{DEF}`, `#define_import_path`) is gone, GLSL support is gone, SPIR-V passthrough stays. Plain WGSL with no preprocessor directives still loads: `_release-content/migration-guides/shaders_use_wesl.md`; https://bevy.org/learn/migration-guides/0-19-to-0-20/ ("WESL shaders").
- Loader accepts `spv`, `wgsl`, `wesl` (`crates/bevy_shader/src/shader.rs:341-343`). Any shader that imports `bevy_pbr::...` must be `.wesl` and use WESL syntax:
  - `import a::b::{c, d};` with trailing `;`, first in the file (guide above).
  - `@if(FLAG)` / `@elif` / `@else` attach to declarations, struct members, parameters, imports, statements; boolean shader defs are flags, `Int`/`UInt` defs are `constants::NAME` (guide above; use in `assets/shaders/extended_material.wesl:6-15,17-25,50`).
  - Material bind group is `@group(constants::MATERIAL_BIND_GROUP)`; the def is pushed as `ShaderDefVal::UInt` with value 3 (`crates/bevy_pbr/src/material.rs:75,520-528`; `assets/shaders/custom_material.wesl:5-7`). Doc comments at `material.rs:143` and `crates/bevy_render/src/render_resource/bind_group.rs:107-112` still show the old `#{MATERIAL_BIND_GROUP}` form: stale, ignore.
  - Pipeline-overridable constants use WGSL `override` (`assets/shaders/pipeline_constants.wesl:6`, `examples/shader/pipeline_constants.rs`).
- Module path = file path inside its crate: `bevy_pbr::render::mesh_view_bindings`, `bevy_pbr::prepass::utils` (guide). Assets outside crates import by relative path: `import super::custom_material_import::COLOR_MULTIPLIER;` (`assets/shaders/custom_material.wesl:3`); `embedded://bevy_foo/bar.wesl` imports as `bevy_foo::bar` (guide; `crates/bevy_shader/src/shader.rs:176-200`).

## Real import names in the pin (`crates/bevy_pbr/src/render/*.wesl` unless noted)
- `bevy_pbr::render::forward_io::{VertexOutput, FragmentOutput}` (`forward_io.wesl:98,118`; `VertexOutput` has `position, world_position, world_normal, uv, uv_b, world_tangent, color, instance_index` under `@if`s).
- `bevy_pbr::render::pbr_fragment::{pbr_input_from_standard_material, pbr_input_from_vertex_output}` (`pbr_fragment.wesl:71,34`).
- `bevy_pbr::render::pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing, apply_fog, premultiply_alpha, calculate_view, apply_normal_mapping}` (`pbr_functions.wesl:98,326,978,866,926,251,175`).
- `bevy_pbr::render::pbr_types::{PbrInput, StandardMaterial}` (`pbr_types.wesl:96`; `PbrInput` has `material, N, V, world_normal, world_position, frag_coord, flags, ...`).
- `bevy_pbr::render::{mesh_functions, view_transformations::position_world_to_clip, mesh_view_bindings, mesh_bindings, skinning, morph, shadows, fog, utils, clustered_forward, pbr_lighting, pbr_ambient, parallax_mapping}`; `mesh_functions::{get_world_from_local, mesh_position_local_to_world, mesh_normal_local_to_world, get_tag}` (`mesh_functions.wesl:15,52,65,155`).
- `bevy_pbr::prepass::io::{VertexOutput, FragmentOutput}` (`prepass/io.wesl:107,140`), `bevy_pbr::prepass::{utils, bindings}`, `bevy_pbr::deferred::functions::{deferred_output, deferred_gbuffer_from_pbr_input, pbr_input_from_deferred_gbuffer}` (`deferred/functions.wesl:139,21,87`).
- `bevy_render::{globals::Globals, utils::octahedral_*, view, maths, color_operations, bindless}` (`crates/bevy_render/src/*.wesl`; octahedral move: `migration-guides/shader_octahedral_moved.md`). 2D: `bevy_sprite_render::mesh2d::vertex_output::VertexOutput` (`assets/shaders/custom_material_2d.wesl:1`).
- `assets/shaders/*.wesl` is the working reference set (extended_material, custom_material, storage_buffer, shader_defs, bindless_material, water_material, game_of_life, gpu_readback, ...).

## `Material`, `MaterialPlugin`, `MeshMaterial3d`
- `trait Material: Asset + AsBindGroup + Clone + Sized` (`crates/bevy_pbr/src/material.rs:147`); defaults for all: `vertex_shader`, `fragment_shader` (:150,156), `alpha_mode(&self)` (:162), `opaque_render_method` (:170), `depth_bias` (:178), `reads_view_transmission_texture` (:187), `enable_prepass`, `enable_shadows`, `enable_oit` (:194,200,208), `prepass_*`, `deferred_*`, `meshlet_*` shaders (:217-271), `specialize` (:282).
- `MaterialPlugin::<M>::default()` (`material.rs:427-440`) registers asset `M`, `MeshMaterial3d<M>`, extraction, specialization; one plugin per material type. `PbrPlugin` adds the one for `StandardMaterial` (`lib.rs:217`).
- Spawn: `Mesh3d(meshes.add(..))` + `MeshMaterial3d(materials.add(M{..}))` (`crates/bevy_pbr/src/mesh_material.rs:41`; `examples/shader/shader_material.rs:25-34`).
- Minimal custom material (verbatim shape of `examples/shader/shader_material.rs:44-63`): `#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)] struct M { #[uniform(0)] color: LinearRgba, #[texture(1)] #[sampler(2)] tex: Option<Handle<Image>>, alpha_mode: AlphaMode }` and `impl Material for M { fn fragment_shader() -> ShaderRef {"shaders/custom_material.wesl".into()} fn alpha_mode(&self) -> AlphaMode {self.alpha_mode} }`; shader side `assets/shaders/custom_material.wesl`.
- Material trait imports: `use bevy::{shader::ShaderRef, render::render_resource::AsBindGroup, pbr::{ExtendedMaterial, MaterialExtension}, material::OpaqueRendererMethod}` (`examples/shader/extended_material.rs:3-9`).
- Deferred: `OpaqueRendererMethod::Deferred` or `DefaultOpaqueRendererMethod` resource plus `DeferredPrepass` on the camera (`examples/shader/extended_material.rs:38-44`). In deferred mode the fragment may only modify `PbrInput`, then `deferred_output(in, pbr_input)`.

## `ExtendedMaterial` + `MaterialExtension`
- `ExtendedMaterial<B, E> { base, extension }` (`crates/bevy_pbr/src/extended_material.rs:151`) implements `AsBindGroup` (data = `CombinedBindGroupData`, :173) and `Material` (:227). `MaterialExtension` (:31) hooks: `vertex_shader`, `fragment_shader` (if `ShaderRef::Default` the base shader runs), `alpha_mode() -> Option<AlphaMode>`, `enable_prepass/shadows/oit`, prepass and deferred shaders, `specialize` (:34-123).
- Extension bindings start at 100 (slots 0-99 belong to the base): `examples/shader/extended_material.rs:75-78`. Bindless variant: `examples/shader/extended_material_bindless.rs` with `#[bindless(index_table(range(..), binding(..)))]` (`bind_group.rs:397-411`).
- Extended fragment shader skeleton: `assets/shaders/extended_material.wesl:1-66`: `pbr_input_from_standard_material(in, is_front)` -> edit `pbr_input` -> `alpha_discard` -> `apply_pbr_lighting` -> edit -> `main_pass_post_lighting_processing` -> edit; the `@if(PREPASS_PIPELINE)` branch returns `deferred_output`.
- 2D analogue: `ExtendedMaterial2d<B: Material2d, E: MaterialExtension2d>` (`crates/bevy_sprite_render/src/mesh2d/extended_material.rs:77`, trait :21; release note `_release-content/release-notes/extended_material_2d.md`).

## `AsBindGroup` attributes (`crates/bevy_render/src/render_resource/bind_group.rs`)
- Field attrs: `uniform(N)` (:158; fields with equal `N` are merged into one struct, in field order), `texture(N, dimension="2d"|"2d_array"|"3d"|"cube"|"cube_array", sample_type, filterable, visibility(..))` (:164), `sampler(N, sampler_type="filtering"|"non_filtering"|"comparison")` (:194), `storage_texture(N, ..)` (:180), `storage(N, read_only, buffer, visibility(..))` (:207; field `Handle<ShaderBuffer>` or raw `Buffer`).
- Struct attrs: `uniform(N, ConvertedType)` with `From<&Self>` (:278), `data(N, Type, binding_array(M))` (:308), `bind_group_data(Key)` for pipeline-key data (:357), `bindless(..)` with `limit(N)` and `index_table(range(M..N), binding(B))` (:364-411).
- `Option<Handle<Image>>` binds `FallbackImage` when `None` (`bind_group.rs:169,251`; `examples/shader/fallback_image.rs`).
- `unprepared_bind_group` is now `build_bind_group` returning `BindGroupBuilder` (migration guide `bind_group_builder.md`); derive users need no change.
- Alignment: structs under WebGL2 need 16-byte alignment; the example pads under `#[cfg(feature = "webgl2")]` and `@if(SIXTEEN_BYTE_ALIGNMENT)` (`examples/shader/extended_material.rs:80-88`, `assets/shaders/extended_material.wesl:17-25`). Not needed for native wgpu.

## Shader assets, embedding, hot reload
- `Shader` asset (`crates/bevy_shader/src/shader.rs:113`): `path`, `source`, `import_path`, `imports`, `shader_defs`, `file_dependencies`, `validate_shader`; constructors `from_wgsl`, `from_wesl`, `from_spirv` (:135,176,162). `ShaderRef::{Default, Handle(Handle<Shader>), Path(AssetPath)}` (:357-365), `From<&'static str>` (:379).
- Shared WESL library modules are embedded with `load_shader_library!(app, "path.wesl")` (`crates/bevy_shader/src/lib.rs:28`; usage `crates/bevy_pbr/src/lib.rs:183-185`); it keeps a strong handle so imports resolve. For app shaders use `embedded_asset!(app, "file.wesl")` and `load_embedded_asset!` (`crates/bevy_asset/src/io/embedded/mod.rs:205,352`; `examples/asset/embedded_asset.rs`); path `embedded://<crate>/<file>`.
- Hot reload: cargo feature `file_watcher` (`Cargo.toml:667`, part of the `dev` collection :156-160) and `embedded_watcher` for embedded sources (:670); toggle at runtime with `AssetPlugin.watch_for_changes_override` (`crates/bevy_asset/src/lib.rs:246`); `examples/asset/hot_asset_reloading.rs:1-10`.
- Shader defs from Rust: push in `Material::specialize` (`examples/shader/shader_defs.rs:62-70`, `assets/shaders/shader_defs.wesl`).

## Buffers, compute, readback
- `ShaderBuffer` (renamed from `ShaderStorageBuffer` in 0.19, https://bevy.org/learn/migration-guides/0-18-to-0-19/ heading "ShaderStorageBuffer renamed to ShaderBuffer"): `crates/bevy_render/src/storage.rs:60`; `new(Vec<T: NoUninit>, usage)` (:118), `with_size` (:129), `extend` (:166), `cast_slice` (:190). 0.20: `set_data` removed, data is `ShaderBufferData` (`migration-guides/shader_buffer_changes.md`).
- Bind as `#[storage(0, read_only)] colors: Handle<ShaderBuffer>`; create with `buffers.add(ShaderBuffer::from(vec))` (`examples/shader/storage_buffer.rs:36,96-97`; WESL `var<storage, read> colors: array<vec4<f32>, 5>` at `assets/shaders/storage_buffer.wesl:6`).
- Compute: `examples/shader/compute_shader_game_of_life.rs`, `examples/shader_advanced/compute_mesh.rs`; GPU to CPU: `Readback`, `ReadbackOnce`, `ReadbackComplete` from `bevy::render::gpu_readback` (`examples/shader/gpu_readback.rs:8-13`). Image capture is covered in render.md.
- Texture binding arrays, custom phases, post processing, fullscreen materials, mesh shaders (not on web): `examples/shader_advanced/*`, release note `_release-content/release-notes/mesh_shaders.md`.

## 2D materials and where they live
`Material2d` (`crates/bevy_sprite_render/src/mesh2d/material.rs:148`), `Material2dPlugin` (:326), `MeshMaterial2d` (:217), `Mesh2d` in `bevy_mesh` (`crates/bevy_mesh/src/components.rs:46`). Sprites in 0.20 render through the 3D mesh infrastructure and accept custom shaders via `MaterialExtension2d` + `SpriteMaterial` + `SpriteMaterialPlugin` (`crates/bevy_sprite_render/src/sprite_mesh/sprite_extended_material.rs:43,218`; `_release-content/release-notes/sprite_materials.md`; the old type was renamed `SpriteMeshMaterial` in `migration-guides/sprite_material_rename.md`). Examples: `examples/shader/shader_material_2d.rs`, `sprite_material.rs`. WESL helpers for sprites: `bevy_sprite_render::sprite_mesh::functions::{sample_final_color, sample_sprite_texture, get_final_color}` (release note above).

## Recipes for our GLB pipeline
Goal: toon / Material-3-flavoured shading of glTF assets: banded lighting plus a tonal rim. Built on `examples/shader/extended_material.rs` and `assets/shaders/extended_material.wesl`. Not compiled.

`src/toon.rs`
```rust
use bevy::{
    color::LinearRgba,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    world_serialization::WorldInstanceReady,
};

const TOON_SHADER: &str = "shaders/toon.wesl";
pub type ToonMaterial = ExtendedMaterial<StandardMaterial, ToonExtension>;

// Same-index field uniforms merge into one struct in field order (`bind_group.rs:254`);
// layout: u32 @0, f32 @4, f32 @8, vec4 @16, mirrored by ToonExtension in toon.wesl.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct ToonExtension {
    #[uniform(100)]
    pub bands: u32,
    #[uniform(100)]
    pub rim_power: f32,
    #[uniform(100)]
    pub rim_strength: f32,
    #[uniform(100)]
    pub rim_color: LinearRgba,
}

impl MaterialExtension for ToonExtension {
    fn fragment_shader() -> ShaderRef { TOON_SHADER.into() }
    fn deferred_fragment_shader() -> ShaderRef { TOON_SHADER.into() }
}

/// Put next to a `WorldAssetRoot` to restyle every glTF material under it.
#[derive(Component, Clone)]
pub struct Toonify(pub ToonExtension);

pub struct ToonPlugin;
impl Plugin for ToonPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<ToonMaterial>::default())
            .add_observer(toonify_gltf_materials);
    }
}

fn toonify_gltf_materials(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    toonify: Query<&Toonify>,
    standard: Query<&MeshMaterial3d<StandardMaterial>>,
    standard_assets: Res<Assets<StandardMaterial>>,
    mut toon_assets: ResMut<Assets<ToonMaterial>>,
) {
    let Ok(Toonify(extension)) = toonify.get(ready.entity) else { return };
    for entity in children.iter_descendants(ready.entity) {
        let Ok(handle) = standard.get(entity) else { continue };
        let Some(base) = standard_assets.get(&handle.0) else { continue };
        commands
            .entity(entity)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert(MeshMaterial3d(toon_assets.add(ToonMaterial {
                base: base.clone(),
                extension: extension.clone(),
            })));
    }
}
```
Observer shape and `WorldInstanceReady` / `ready.entity` follow `examples/gltf/edit_material_on_gltf.rs:5,55-56`. The base material clone keeps every glTF texture handle (KTX2 included). Cache by source `StandardMaterial` id if a model has many primitives sharing a material (comment at `edit_material_on_gltf.rs:87-90`).

Spawn: `commands.spawn((WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/hero.glb"))), Toonify(ToonExtension { bands: 4, rim_power: 3.0, rim_strength: 0.35, rim_color: LinearRgba::rgb(0.55, 0.62, 1.0) })));` (`WorldAssetRoot` + `GltfAssetLabel::Scene(0)`: `examples/gltf/edit_material_on_gltf.rs:35-47`).

`assets/shaders/toon.wesl` (WESL, not naga_oil WGSL; imports and `@if` as in `assets/shaders/extended_material.wesl:1-15,50-66`)
```wgsl
import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
};

@if(PREPASS_PIPELINE)
import bevy_pbr::{
    prepass::io::{VertexOutput, FragmentOutput},
    deferred::functions::deferred_output,
};
@else
import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};

struct ToonExtension {
    bands: u32,
    rim_power: f32,
    rim_strength: f32,
    rim_color: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> toon: ToonExtension;

// Quantize luminance in a compressed [0,1) domain so HDR highlights still land in the top band.
fn toon_band(color: vec3<f32>, bands: f32) -> vec3<f32> {
    let luma = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    let compressed = luma / (1.0 + luma);
    let banded = min(floor(compressed * bands + 0.5) / bands, 0.999);
    let target_luma = banded / (1.0 - banded);
    return color * (target_luma / max(luma, 0.0001));
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color
    );

    var out: FragmentOutput;
@if(PREPASS_PIPELINE) {
    out = deferred_output(in, pbr_input);
} @else {
    out.color = apply_pbr_lighting(pbr_input);

    let banded = toon_band(out.color.rgb, f32(max(toon.bands, 1u)));
    let facing = saturate(dot(pbr_input.N, pbr_input.V));
    let rim = pow(1.0 - facing, toon.rim_power) * toon.rim_strength;
    out.color = vec4<f32>(banded + toon.rim_color.rgb * rim, out.color.a);

    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
}
    return out;
}
```
Checks against the pin: `PbrInput.N` and `.V` exist (`pbr_types.wesl:108,111`); `apply_pbr_lighting(in: PbrInput) -> vec4<f32>` (`pbr_functions.wesl:326`); `main_pass_post_lighting_processing(pbr_input, color)` (:978); `saturate`, `floor`, `pow` are WGSL builtins. Likely compile pitfalls to check on fr1: (a) field-level `#[uniform(100)]` merging order versus the WESL struct, (b) whether `ExtendedMaterial<_, E>` needs `E: Reflect` fields only (it does here: `u32`, `f32`, `LinearRgba` are `Reflect`), (c) `@if(...) { } @else { }` statement form (copied from the example).
For M3 look, feed `rim_color` from the brand seed tonal role (`fw-m3`) and keep `bands` 3 to 5; shadows stay on because `enable_shadows` defaults true (`extended_material.rs:58`).
