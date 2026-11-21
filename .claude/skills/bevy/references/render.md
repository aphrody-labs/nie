# Bevy render (bevy_render on wgpu), headless, screenshots

Pin: bevyengine/bevy `a0577b5` (Release 0.20.1, 2026-10-28; same commit as aphrody-labs/bevy checkout `/c/src/gpu-bench/vendor/bevy`).
Citation form `path:line` = file at that commit. URL form: `https://github.com/bevyengine/bevy/blob/a0577b5c6a7b208719dd761c5c6a7a539e472a97/<path>#L<line>`.
Snippets are "not compiled" unless marked otherwise (the lead compiles them on fr1).

## Stack and versions
- wgpu 30: `wgpu = { version = "30", ... }` crates/bevy_render/Cargo.toml:90; nie pins `wgpu-types` 30.0.2 (/c/src/nie/Cargo.toml:102). Backend features: `webgl` `webgpu` `gles` `vulkan-portability` crates/bevy_render/Cargo.toml:31-34.
- `RenderPlugin { render_creation, synchronous_pipeline_compilation, debug_flags }` crates/bevy_render/src/lib.rs:160-167. `synchronous_pipeline_compilation` has no effect on macOS, Wasm, iOS or without `multi_threaded` (lib.rs:163-164).
- `WgpuSettings` crates/bevy_render/src/settings.rs:39; `RenderCreation` settings.rs:222; `backends: None` makes a renderer-less app (examples/app/no_renderer.rs:17).
- Env vars (crates/bevy_render/src/lib.rs:3-12, settings.rs:84-89, renderer/mod.rs:261): `WGPU_BACKEND` and `WGPU_POWER_PREF` (read by wgpu-types, /c/src/gpu-bench/vendor/wgpu wgpu-types/src/backend.rs:157, adapter.rs:91; Bevy calls `from_env` at settings.rs:84,87), `WGPU_DEBUG=1`, `WGPU_VALIDATION=0`, `WGPU_FORCE_FALLBACK_ADAPTER=1` (software rendering, "typically matches CI"), `WGPU_ADAPTER_NAME`, `WGPU_SETTINGS_PRIO=webgl2|webgpu`, `VERBOSE_SHADER_ERROR=1`. Migration 0.18->0.19: `WgpuSettingsPriority::Compatibility` renamed `WebGPU`, env value `compatibility` -> `webgpu` (https://bevy.org/learn/migration-guides/0-18-to-0-19/).
- CI software adapter: upstream installs `mesa-vulkan-drivers` (lavapipe) and `libgl1-mesa-dri` (llvmpipe) .github/workflows/update-caches.yml:198-204. Our fleet recipe: `apt install mesa-vulkan-drivers libgl1-mesa-dri`, then `WGPU_BACKEND=vulkan WGPU_FORCE_FALLBACK_ADAPTER=1 ./app`.

## Two worlds, schedules, sets
- `RenderApp` sub-app label crates/bevy_render/src/lib.rs:380; main app extracts into it each frame; optional pipelined rendering via `PipelinedRenderingPlugin` (lib.rs:156-158, bevy_internal/src/default_plugins.rs:57).
- `RenderScheduleOrder` default `[First, Render]` crates/bevy_render/src/lib.rs:282-300; `RenderStartup` runs on every device (re)acquire, lib.rs:246; use `init_gpu_resource::<R>` / `SubApp::init_gpu_resource` (lib.rs:248-269).
- `Render` schedule lib.rs:321; sets `RenderSystems` lib.rs:186-239: ExtractCommands, PrepareAssets, PrepareMeshes, CreateViews, Specialize, PrepareViews, Queue(+QueueMeshes, QueueSweep), PhaseSort, Prepare(+PrepareResources, PrepareResourcesBatchPhases, ...WritePhaseBuffers, ...CollectPhaseBuffers, ...Flush, PrepareBindGroups), Render, Cleanup, PostCleanup. `ManageViews` was split into `CreateViews`/`Specialize`/`PrepareViews` in 0.19.
- Render graph is now schedules, not a graph object: `RenderGraph` schedule crates/bevy_render/src/renderer/mod.rs:48 with sets `RenderGraphSystems::{Begin, Render, Submit, Finish}` (mod.rs:55-58, 68-77); `render_system` runs `world.run_schedule(RenderGraph)` then screenshots/readbacks/present (mod.rs:82-94). Hierarchy: `Render > RenderSystems::Render > render_system > RenderGraph > camera_driver > Core2d | Core3d | custom` (mod.rs:29-45). Custom passes = systems taking `RenderContext` in a `RenderGraphSystems::Render` set (examples/app/headless_renderer.rs:224 orders after `Submit`). 0.19 "Render Graph as Systems" and 0.20 weak ordering (`chain_weak`) https://bevy.org/learn/migration-guides/0-19-to-0-20/#several-built-in-schedules-now-order-their-system-sets-weakly.
- Extraction moved to the new crate `bevy_extract` in 0.20 (generic over world/AppLabel): `ExtractSchedule` crates/bevy_extract/src/extract_plugin.rs:108, `MainWorld` :127, `Extract<P>` extract_param.rs:51, `ExtractComponent<L: AppLabel,F>` extract_component.rs:28, `#[extract_app(RenderApp)]` (crates/bevy_render/src/gpu_readback.rs:84). Migration: https://bevy.org/learn/migration-guides/0-19-to-0-20/#extraction-is-now-generic-over-worlds. `ExtractResourcePlugin`/`ExtractComponentPlugin` alias bevy_extract (crates/bevy_render/src/lib.rs:45-60).
- Pipelines: `PipelineCache` crates/bevy_render/src/render_resource/pipeline_cache.rs:219 (async creation deferred to `RenderSystems::Render`, no dedup, :208-217); `queue_render_pipeline` :432, `get_render_pipeline` :374. Specialization: `SpecializedRenderPipeline`(+`...Pipelines`) pipeline_specializer.rs:30,51; `SpecializedMeshPipeline` :135; newer `Specializer<T>` trait render_resource/specializer.rs:176. Material users never touch these (see shaders-materials.md).
- Camera: `Camera3d` bevy_camera/src/components.rs:25; `Camera { clear_color: ClearColorConfig, .. }` camera.rs:385,402; `ClearColorConfig::{Default, Custom(Color), None}` bevy_camera/src/clear_color.rs:13-23; `RenderTarget` is a component (spawn it in the camera bundle): `enum RenderTarget { Window, Image(ImageRenderTarget), TextureView, None{size} }` camera.rs:893-910 (examples/3d/render_to_texture.rs:78). Render-side `ViewTarget` crates/bevy_render/src/view/mod.rs:785. 0.20: `ViewTarget::compositing_space` -> `ResolvedCompositingSpace`; `WgpuWrapper` removed; `Tonemapping::None` is passthrough, `Tonemapping::Linear` added (0.19->0.20 guide, Rendering).

## Render to texture and GPU readback
- Target image: `Image::new_target_texture(w, h, TextureFormat, Option<view_format>)` crates/bevy_image/src/image.rs:1253 (usage TEXTURE_BINDING|COPY_DST|RENDER_ATTACHMENT, :1265-1267). Camera with `order: -1` + `RenderTarget::Image(handle.into())` examples/3d/render_to_texture.rs:72-78; isolate with `RenderLayers`.
- `Readback::texture(handle)` / `Readback::buffer(..)` / `ReadbackOnce` components + `ReadbackComplete` entity event crates/bevy_render/src/gpu_readback.rs:85,125,157; example examples/shader/gpu_readback.rs:74,146. Buffer type is `ShaderBuffer` (renamed from `ShaderStorageBuffer` in 0.19).
- Manual copy path with channels (`ImageCopier`, `RenderGraphSystems::Submit`, `PollType::wait_indefinitely`, row padding via `RenderDevice::align_copy_bytes_per_row`): examples/app/headless_renderer.rs:224,432. The file's own doc says use `Screenshot` for single captures (headless_renderer.rs:11-13).

## Headless and screenshots
- Headless app = `DefaultPlugins` with `WindowPlugin { primary_window: None, exit_condition: ExitCondition::DontExit, .. }` + `.disable::<WinitPlugin>()` + `ScheduleRunnerPlugin::run_loop(Duration)` (or `run_once()`): examples/app/headless_renderer.rs:88-107; bevy_app/src/schedule_runner.rs:57,64. WinitPlugin panics without a display server (headless_renderer.rs:96-97). Imports: `bevy::{window::ExitCondition, winit::WinitPlugin, app::{AppExit, ScheduleRunnerPlugin}}`.
- Externally pumped loop: build with `.disable::<WinitPlugin>()`, `app.finish(); app.cleanup();`, take `SubApps`, call `sub_apps.update()` then `RenderDevice::wgpu_device().poll(PollType::Wait{submission_index: None, timeout: None})`: examples/app/externally_driven_headless_renderer.rs:42-68,92-110. Uses `RenderPlugin { synchronous_pipeline_compilation: true }` (:42) so shaders exist on the first frame.
- `Screenshot(pub RenderTarget)` component crates/bevy_render/src/view/window/screenshot.rs:84; constructors `window`, `primary_window`, `image(handle)`, `texture_view` :95-113; result is the `ScreenshotCaptured { entity, image: Image }` entity event (:53) triggered from `trigger_screenshots` (:259-267) a frame or more after spawn; the screenshot entity is despawned afterwards (:68, :310). Markers `Capturing`/`Captured` :73-79.
- `save_to_disk(path)` :145 is an observer; for 8-bit targets it does `dyn_img.to_rgb8()` and drops alpha (:197-201), saved with `img.save_with_format` (:204). Use your own observer (below) for transparent PNG. Float targets keep range only for `.exr`/`.hdr` (:136-143).
- Example: examples/window/screenshot.rs:27 (`.spawn(Screenshot::primary_window()).observe(save_to_disk(path))`); image-target use examples/app/externally_driven_headless_renderer.rs:116-117.

## Render recovery (device lost, errors)
- `RenderErrorHandler(fn(&RenderError, &mut World /*main*/, &mut World /*render*/) -> RenderErrorPolicy)` crates/bevy_render/src/error_handler.rs:47-49; `RenderErrorPolicy::{Ignore, StopRendering, Recover(RenderCreation)}` :20-35; states `RenderState::{Initializing, Ready, Errored, Reinitializing}` :102. Example sets the handler as a resource: examples/app/render_recovery.rs:127-130. Register GPU resources with `init_gpu_resource` so they are rebuilt (lib.rs:248-269).

## Recipes for our GLB pipeline

### Offscreen render of a GLB from 8 compass directions to RGBA PNG (not compiled)
Design: one camera -> one target image (Rgba8UnormSrgb, transparent clear); rotate the model root by `i * 45deg` so light stays camera-fixed and every sprite is lit the same; one `Screenshot::image` per direction; next direction starts when `ScreenshotCaptured` fires; exit with `AppExit` after 8. Rotating on frame k and shooting on frame k+1 guarantees the transform is propagated and extracted (inference; verify). Needs features `bevy_gltf`, `bevy_pbr`, `bevy_window`, `png`; run with a Vulkan/GL adapter (lavapipe on fr1). Asset path is relative to `assets/` (assets.md).
```rust
use bevy::{
    app::{AppExit, ScheduleRunnerPlugin},
    camera::{RenderTarget, ScalingMode},
    prelude::*,
    render::{
        render_resource::TextureFormat,
        view::screenshot::{Screenshot, ScreenshotCaptured},
        RenderPlugin,
    },
    window::ExitCondition,
    winit::WinitPlugin,
    world_serialization::WorldInstanceReady,
};
use std::{f32::consts::TAU, time::Duration};

const SIZE: u32 = 512;
const NAMES: [&str; 8] = ["s", "sw", "w", "nw", "n", "ne", "e", "se"]; // model faces +Z at yaw 0 (check GltfConvertCoordinates)
const WARMUP_FRAMES: u32 = 10;

#[derive(Resource, Default)]
struct Capture { ready: bool, warmup: u32, index: usize, stage: Stage }
#[derive(Default, PartialEq)]
enum Stage { #[default] Rotate, Shoot, Wait }
#[derive(Resource)]
struct Target(Handle<Image>);
#[derive(Component)]
struct Model;

fn main() {
    App::new()
        .init_resource::<Capture>()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin { primary_window: None, exit_condition: ExitCondition::DontExit, ..default() })
                .set(RenderPlugin { synchronous_pipeline_compilation: true, ..default() })
                .disable::<WinitPlugin>(),
        )
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_millis(16)))
        .add_systems(Startup, setup)
        .add_systems(Update, drive)
        .run();
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, assets: Res<AssetServer>) {
    let target = images.add(Image::new_target_texture(SIZE, SIZE, TextureFormat::Rgba8UnormSrgb, None));
    commands.insert_resource(Target(target.clone()));
    commands
        .spawn((Model, WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset("models/hero.glb")))))
        .observe(|_: On<WorldInstanceReady>, mut capture: ResMut<Capture>| capture.ready = true);
    commands.spawn((
        Camera3d::default(),
        Camera { clear_color: Color::NONE.into(), ..default() },
        RenderTarget::Image(target.into()),
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical { viewport_height: 2.5 },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(0.0, 3.0, 5.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
    ));
    commands.spawn((DirectionalLight { shadow_maps_enabled: false, ..default() }, Transform::from_xyz(2.0, 4.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y)));
    commands.insert_resource(GlobalAmbientLight { brightness: 300.0, ..default() });
}

fn drive(mut commands: Commands, mut capture: ResMut<Capture>, target: Res<Target>, mut model: Query<&mut Transform, With<Model>>) {
    if !capture.ready { return; }
    if capture.warmup < WARMUP_FRAMES { capture.warmup += 1; return; }
    match capture.stage {
        Stage::Rotate => {
            for mut transform in &mut model {
                transform.rotation = Quat::from_rotation_y(capture.index as f32 * TAU / 8.0);
            }
            capture.stage = Stage::Shoot;
        }
        Stage::Shoot => {
            let path = format!("out/{}.png", NAMES[capture.index]);
            commands.spawn(Screenshot::image(target.0.clone())).observe(
                move |shot: On<ScreenshotCaptured>, mut capture: ResMut<Capture>, mut exit: MessageWriter<AppExit>| {
                    match shot.image.clone().try_into_dynamic() {
                        Ok(image) => if let Err(error) = image.to_rgba8().save(&path) { error!("save {path}: {error}"); },
                        Err(error) => error!("convert {path}: {error:?}"),
                    }
                    capture.index += 1;
                    if capture.index == NAMES.len() { exit.write(AppExit::Success); } else { capture.stage = Stage::Rotate; }
                },
            );
            capture.stage = Stage::Wait;
        }
        Stage::Wait => {}
    }
}
```
- API sources: `MessageWriter<AppExit>` + `.write(AppExit::Success)` examples/app/headless_renderer.rs:519; `Camera { clear_color: Color::WHITE.into() }` examples/3d/render_to_texture.rs:72-76; orthographic `FixedVertical` examples/3d/orthographic.rs:17-21; `WorldAssetRoot`/`GltfAssetLabel::Scene(0).from_asset(..)` examples/gltf/load_gltf.rs and examples/asset/hot_asset_reloading.rs:13; `WorldInstanceReady` examples/gltf/edit_material_on_gltf.rs:5,56; `GlobalAmbientLight` crates/bevy_light/src/ambient_light.rs:62 (default brightness 80, :86); `try_into_dynamic()` examples/app/headless_renderer.rs (update fn) and screenshot.rs:151.
- Uncertain, compile-check: `WorldAssetRoot`/`GltfAssetLabel` in prelude (used with `prelude::*` in examples/gltf/load_gltf.rs); `try_into_dynamic` error type; observer closures with `MessageWriter` params and `move` captures; `mkdir out` must exist (the saver does not create dirs).
- Waiting rules: `WorldInstanceReady` means entities spawned, not GPU-ready; the warm-up count plus `synchronous_pipeline_compilation: true` covers pipelines/textures (examples/app/headless_renderer.rs:122-131 uses 40 pre-roll frames and notes the first frames are transparent/black). Fit the camera from `Aabb` (bevy_camera/src/primitives.rs:62) after PostUpdate when models vary in size.
- Perspective instead: drop `Projection`, keep `Camera3d` default (`Projection::Perspective`), place at `Vec3::new(0, h, r)`.

### Run on CI/fleet without a GPU
`WGPU_BACKEND=vulkan WGPU_FORCE_FALLBACK_ADAPTER=1 cargo run --release` with lavapipe installed (see above); unset `DISPLAY` is fine because `WinitPlugin` is disabled.
