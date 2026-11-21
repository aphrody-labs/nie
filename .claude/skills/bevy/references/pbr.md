# bevy_pbr and lighting (Bevy 0.20.1)

Pin: `bevyengine/bevy` @ `a0577b5` (Release 0.20.1). Cites `path:line` are in that tree (`https://github.com/bevyengine/bevy/blob/a0577b5/<path>#L<line>`); a bare URL is a live web page.
Not compiled: every snippet below is checked by reading only; the lead compiles the recipes on fr1.

## What changed in 0.20 for this area
- Shaders are WESL, naga_oil is gone; module paths follow file paths (`bevy_pbr::render::pbr_functions`): `_release-content/migration-guides/shaders_use_wesl.md`, https://bevy.org/learn/migration-guides/0-19-to-0-20/ (heading "WESL shaders").
- `Tonemapping`, `DebandDither` moved to `bevy_render::view` (`crates/bevy_render/src/view/mod.rs:283`); `Tonemapping::None` is a full passthrough, use `Tonemapping::Linear` for "no curve" (`migration-guides/tonemapping_none_passthrough.md`).
- `ScreenSpaceTransmission` is no longer required by `Camera3d`; add it for specular transmission (`migration-guides/transmission_opt_in.md`, struct at `crates/bevy_pbr/src/transmission/mod.rs:63`).
- Atmosphere supports multiple cameras (`AtmosphereBuffer` is a per-camera component) (`migration-guides/atmosphere_multicam.md`).
- OIT is opt-out per material through `Material::enable_oit` / `MaterialExtension::enable_oit`, shader def `MATERIAL_OIT_ENABLED` (`migration-guides/material_oit_changes.md`, `crates/bevy_pbr/src/extended_material.rs:66`).
- `UnpreparedBindGroup` is `BindGroupBuilder`, `unprepared_bind_group` is `build_bind_group` (`migration-guides/bind_group_builder.md`, `crates/bevy_pbr/src/extended_material.rs:193`).
- Carried from 0.19/0.18: lights live in `bevy_light` (atmosphere moved there), `Hdr` is a component in `bevy_camera`, `AmbientLight` is a camera component plus a `GlobalAmbientLight` resource, bloom in `bevy_post_process` (https://bevy.org/learn/migration-guides/0-18-to-0-19/ headings "Atmosphere has been moved to bevy_light", "Hdr moved to bevy_camera", "Post Processing Split"; https://bevy.org/learn/migration-guides/0-17-to-0-18/ heading "AmbientLight split into a component and a resource").
- `SceneRoot` / `SceneInstanceReady` of older docs are `WorldAssetRoot` / `WorldInstanceReady` (`bevy::world_serialization`): `examples/gltf/edit_material_on_gltf.rs:5,55-56`.

## StandardMaterial (`crates/bevy_pbr/src/pbr_material.rs:27`)
Derives `Asset, AsBindGroup, Reflect`; uniform at binding 0 via `#[data(0, StandardMaterialUniform, binding_array(10))]`, bindless index table `range(0..31)` (:23-25).
- Colour: `base_color: Color` (:35, default white), `emissive: LinearRgba` (:93), `emissive_exposure_weight` (:100).
- Surface: `perceptual_roughness` (:134, default 0.5, `:871`), `metallic` (:145, default 0.0), `reflectance` (:187, default 0.5), `specular_tint` (:200), `ior` (:345, default 1.5), `clearcoat*` (:505-571), `anisotropy_*` (:590-642).
- Transmission: `diffuse_transmission`, `specular_transmission`, `thickness`, `attenuation_distance`, `attenuation_color` (:222-370).
- Flags: `double_sided` (:650), `cull_mode: Option<Face>` default `Some(Face::Back)` (:671), `unlit` (:677), `fog_enabled` (:680), `alpha_mode` (:685), `depth_bias` (:697), `opaque_render_method` (:782), `uv_transform: Affine2` applied to UV0 (:790; this is where a KHR_texture_transform lands, see gltf.md).
- Textures (binding of texture, sampler = +1): `base_color_texture` 1/2 (:55), `emissive_texture` 3 (:115), `metallic_roughness_texture` 5 (:171, glTF G=roughness, B=metallic), `occlusion_texture` 7 (:443), `normal_map_texture` 9 (:414), `depth_map` 11 (:743). Each has a `*_channel: UvChannel` (`Uv0`/`Uv1`), e.g. :40.
- Texture fields behind cargo features: `pbr_transmission_textures`, `pbr_multi_layer_material_textures` (clearcoat), `pbr_anisotropy_texture`, `pbr_specular_textures` (`crates/bevy_pbr/Cargo.toml:14-19`, forwarded to bevy_gltf with `?`). Without them the glTF loader cannot fill those fields.
- Normal maps need vertex tangents, UVs and normals; generate with `Mesh::with_generated_tangents` (:380-400). Normal map must be linear (`is_srgb = false`): the glTF loader computes the linear set per texture (`crates/bevy_gltf/src/loader/mod.rs:280,1210`).
- KTX2: textures are plain `Handle<Image>`; the KTX2 loader honours `is_srgb` when choosing the format (`crates/bevy_image/src/ktx2.rs:107,165,229`). Features `ktx2`, `basis-universal`, `zstd_rust` / `zstd_c`: `Cargo.toml:236-241,456,491,521-524`; details in image-ktx2.md.
- Setting a texture with non-default settings uses `asset_server.load_builder().with_settings(|s: &mut ImageLoaderSettings| s.is_srgb = false).load(path)` (:403-412).

## Materials, not lights, decide the look: ExtendedMaterial
`ExtendedMaterial<B: Material, E: MaterialExtension> { base, extension }` (`crates/bevy_pbr/src/extended_material.rs:151`); `MaterialExtension` trait at :31 with `vertex_shader`, `fragment_shader`, `alpha_mode() -> Option`, `enable_prepass`, `enable_shadows`, `enable_oit`, prepass/deferred shader hooks, `specialize` (:34-123). Extension bindings must start at 100 (0..99 belong to the base): `examples/shader/extended_material.rs:68-70`. Register with `MaterialPlugin::<ExtendedMaterial<StandardMaterial, E>>::default()` (`examples/shader/extended_material.rs:17-20`). Full recipe: shaders-materials.md.

## Plugin wiring
`PbrPlugin` (`crates/bevy_pbr/src/lib.rs:126`) already adds `MaterialPlugin::<StandardMaterial>` (:217), `ScreenSpaceAmbientOcclusionPlugin` (:221), `VolumetricFogPlugin` (:230), `ScreenSpaceReflectionsPlugin` (:231), `ScreenSpaceTransmissionPlugin` (:232), `AtmospherePlugin` (:246). No extra plugin is needed for those effects; they are per-camera components. Meshlet is behind features `meshlet`, `meshlet_processor` (`Cargo.toml:691-694`), Solari behind `bevy_solari` (`Cargo.toml:338`).

## Lights (crate `bevy_light`, `crates/bevy_light/src/`)
| Type | Cite | Notes |
|---|---|---|
| `DirectionalLight` | `directional_light.rs:73` | `illuminance` lux default `AMBIENT_DAYLIGHT` 10000 (:86,:151); `shadow_maps_enabled` default false (:93,:152); `contact_shadows_enabled` (:96); `soft_shadow_size` (:116) only exists with feature `experimental_pbr_pcss` |
| `DirectionalLightShadowMap` | `directional_light.rs:193` | resource, `size` default 2048, must be power of two (:202-204) |
| `CascadeShadowConfigBuilder` | `cascade.rs:60` | builds `CascadeShadowConfig` (:26); used in `examples/3d/anti_aliasing.rs:452` |
| `PointLight` | `point_light.rs:49` | `intensity` lumens default 1_000_000 (:133), `range` 20 (:134), `radius`, `shadow_maps_enabled` false; `PointLightShadowMap` :180 |
| `SpotLight` | `spot_light.rs:34` | same fields as point plus `outer_angle`, `inner_angle` (:129,:135) |
| `RectLight` | `rect_light.rs:21` | `width`, `height`, `intensity`, `range` (:25-39) |
| `AmbientLight` | `ambient_light.rs:12` | component on a camera, overrides the resource (`#[require(Camera)]`) |
| `GlobalAmbientLight` | `ambient_light.rs:62` | resource, `brightness` default 80 cd/m2 (:86), `GlobalAmbientLight::NONE` (:94) |
| `EnvironmentMapLight` | `probe.rs:107` | camera component: `diffuse_map`, `specular_map` (KTX2 cubemaps), `intensity` cd/m2 (`examples/3d/pbr.rs:118-124`) |
| `GeneratedEnvironmentMapLight` | `probe.rs:269` | filters a source cubemap on GPU, power-of-two size |
| `Skybox` | `probe.rs:235` | `image`, `brightness` |
| `LightProbe`, `IrradianceVolume` | `probe.rs:75,338` | baked/placed probes |
| `NotShadowCaster`, `NotShadowReceiver` | `lib.rs:264,272` | marker components |
| `ShadowFilteringMethod` | `lib.rs:291` | camera component: `Hardware2x2`, `Gaussian` (default), `Temporal` |
Lux presets: `light_consts::lux::{OVERCAST_DAY 1000, AMBIENT_DAYLIGHT 10000, FULL_DAYLIGHT 20000, DIRECT_SUNLIGHT 100000}` (`crates/bevy_light/src/lib.rs:148-155`).

## Camera-side effects
- HDR and display: `Hdr` marker (`crates/bevy_camera/src/components.rs:89`); `Msaa` component default `Sample4` (`crates/bevy_render/src/view/mod.rs:248-254`); `Exposure { ev100 }` (`crates/bevy_camera/src/camera.rs:233`, `Exposure::BLENDER`, `INDOOR`, `OVERCAST`, `SUNLIGHT`).
- `Tonemapping` variants: None, Linear, Reinhard, ReinhardLuminance, AcesFitted, AgX, SomewhatBoringDisplayTransform, TonyMcMapface (default), BlenderFilmic (`view/mod.rs:283-292`); example `examples/3d/tonemapping.rs`.
- `Bloom` (`crates/bevy_post_process/src/bloom/settings.rs:33`, requires `Hdr`; presets `NATURAL`, `ANAMORPHIC`, `OLD_SCHOOL` at :132,147,155); `examples/3d/bloom_3d.rs:33-35`.
- SSAO: `ScreenSpaceAmbientOcclusion { quality_level, radius, constant_object_thickness }` (`crates/bevy_pbr/src/ssao/mod.rs:117`); requires `DepthPrepass` + `NormalPrepass` (:113); docs recommend TAA, not supported on WebGL2/WebGPU (:109-110); example uses `Hdr` + `Msaa::Off` (`examples/3d/ssao.rs:33-34`).
- SSR: `ScreenSpaceReflections` (`crates/bevy_pbr/src/ssr/mod.rs:82`) requires `DepthPrepass` + `DeferredPrepass`; deferred rendering must be enabled by the user; camera setup `Hdr, Msaa::Off, TemporalAntiAliasing, ScreenSpaceReflections` (`examples/3d/ssr.rs:437-447`).
- Atmosphere: entity `Atmosphere::earth(medium)` (`crates/bevy_light/src/atmosphere.rs:37,74`), medium `ScatteringMedium::earth(256, 256)` (:197) in `Assets<ScatteringMedium>`; camera gets `AtmosphereSettings` (`crates/bevy_pbr/src/atmosphere/mod.rs:297`, `AtmosphereMode` :423) and optional `AtmosphereEnvironmentMapLight` (`probe.rs:306`); light side `SunDisk` (`directional_light.rs:268`). Full setup: `examples/3d/atmosphere.rs:136-182`.
- Volumetric fog: camera `VolumetricFog { ambient_color, ambient_intensity (0.1), jitter, step_count (64) }` (`crates/bevy_light/src/volumetric.rs:25,96-100`), lights opt in with `VolumetricLight` (:16), volumes `FogVolume` (:78); `examples/3d/volumetric_fog.rs:75-96`. Set `ambient_intensity: 0.0` with no environment map.
- Screen-space transmission: add `ScreenSpaceTransmission` to the camera (see 0.20 changes above).
- Others present as examples: depth of field (`crates/bevy_post_process/src/dof/mod.rs:78`), auto exposure (`.../auto_exposure/settings.rs:31`), `ColorGrading` (`view/mod.rs:490`), motion blur, contact shadows, decals, lightmaps, `examples/3d/*.rs`.

## Recipes for our GLB pipeline
Turntable / asset-preview lighting (neutral, deterministic, no screen-space effects, LDR render target, works offscreen):
```rust
// not compiled; types and fields verified against the cites above
use bevy::{light::light_consts::lux, prelude::*, render::view::Tonemapping};

fn preview_stage(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(GlobalAmbientLight { brightness: 0.0, ..default() });
    commands.spawn((
        Camera3d::default(),
        Tonemapping::TonyMcMapface,
        EnvironmentMapLight {
            diffuse_map: asset_server.load("environment_maps/pisa_diffuse_rgb9e5_zstd.ktx2"),
            specular_map: asset_server.load("environment_maps/pisa_specular_rgb9e5_zstd.ktx2"),
            intensity: 900.0, // examples/3d/pbr.rs:121-123
            ..default()
        },
        Transform::from_xyz(0.0, 1.2, 3.0).looking_at(Vec3::new(0.0, 0.9, 0.0), Vec3::Y),
    ));
    // key
    commands.spawn((
        DirectionalLight { illuminance: lux::AMBIENT_DAYLIGHT, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(2.0, 4.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    // fill, no shadows
    commands.spawn((
        DirectionalLight { illuminance: lux::OVERCAST_DAY, ..default() },
        Transform::from_xyz(-3.0, 2.0, -1.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
```
- The two environment KTX2 files exist in the pin under `assets/environment_maps/` (`pisa_diffuse_rgb9e5_zstd.ktx2`, `pisa_specular_rgb9e5_zstd.ktx2`); ship our own copies under the app `assets/`. They are RGB9E5 + zstd, so the `ktx2` and `zstd_rust` features must be on.
- For a pure albedo/flat check set `StandardMaterial::unlit = true` (:677) instead of removing lights.
- Hold the lights fixed and rotate the model root (or move the camera on a circle) when rendering the 8 directions; see render.md for the screenshot loop.
- glTF materials arrive as `MeshMaterial3d<StandardMaterial>` on mesh entities of the `WorldAssetRoot`; to swap them for `ExtendedMaterial` use the `WorldInstanceReady` observer in shaders-materials.md.
