# bevy_animation, animation graph, glTF clips (Bevy 0.20.1)

Pin: bevyengine/bevy commit `a0577b5` (Release 0.20.1). Citations `path:line (a0577b5)` are relative to the repo root
(`https://github.com/bevyengine/bevy/blob/a0577b5/<path>#L<line>`). Snippets are "not compiled" (the lead compiles recipes on fr1).
Web: examples index https://bevy.org/examples/ (Animation category, e.g. https://bevy.org/examples/animation/animated-mesh-control/),
migration guide https://bevy.org/learn/migration-guides/0-19-to-0-20/ (no animation item found on the fetched page).

## Features and plugin
- glTF clips need cargo feature `gltf_animation` (`crates/bevy_internal/Cargo.toml:226`); morph weight curves need `morph_animation` (`:232`). Both are in the root `3d_api` collection (`Cargo.toml:237,254`), which `default = ["2d", "3d", "ui", "audio"]` pulls in (`Cargo.toml:136`); `bevy_animation` itself is in `common_api` (`Cargo.toml:203`). With default-features off (as nie does) enable them explicitly.
- `AnimationPlugin` registers assets `AnimationClip`, `AnimationGraph` and the `.animgraph.ron` loader, then in `PostUpdate`, chained, before transform propagation: `advance_transitions`, `advance_animations`, `animate_targets`, `trigger_untargeted_animation_events`, `expire_completed_transitions` (`crates/bevy_animation/src/lib.rs:1312-1342`).

## Model
- `AnimationClip` (asset): `HashMap<AnimationTargetId, Vec<VariableCurve>>` + events + duration (`lib.rs:105-111`, curves type `:160`). Clips address bones by UUID, not entity, so any clip plays on any rig whose bone name paths match (`lib.rs:163-176`).
- `AnimationTargetId(Uuid)`: blake3 hash of the name path from the animation root to the bone, each name length-prefixed (`lib.rs:187`, `:1345-1376`). Build with `AnimationTargetId::from_names(names.iter())` or `from_name(&Name)`.
- `AnimatedBy(Entity)`: links an animated entity to the `AnimationPlayer` entity (`lib.rs:215`). In 0.20 `AnimationTarget{id, player}` no longer exists: the loader inserts the pair `(AnimationTargetId, AnimatedBy(root))` (`crates/bevy_gltf/src/loader/mod.rs:1569-1570`). Each entity is animated by one player.
- `AnimationPlayer`: map node index -> `ActiveAnimation` (`lib.rs:748`). Methods: `start(node)` restarts, `play(node)` keeps if already playing (`:875-886`), `stop`, `stop_all`, `playing_animations[_mut]` (`:901,909`), `animation(_mut)(node)` (`:991,999`), `all_finished`, `pause_all/resume_all/rewind_all`, `adjust_speeds(factor)` multiplies every speed (`:965`), `seek_all_by(delta)` (`:979`).
- `ActiveAnimation` (returned by `play`/`start`, chainable): `repeat()` = Forever (`:657`), `set_repeat(RepeatAnimation::{Never,Count(n),Forever})` (`:484-492,651`), `set_speed` (`:682`, negative reverses), `seek_to`/`set_seek_time` (`:713,724`), `set_weight` (`:625`), `pause/resume`, `replay`, `rewind`, `is_finished` (`:566`: Never => completions >= 1), `just_completed`, `completions`, `elapsed`, `seek_time`.

## AnimationGraph (`crates/bevy_animation/src/graph.rs`)
- `AnimationGraph{graph: DiGraph<AnimationGraphNode>, root, mask_groups}` asset (`:115`). Attach with component `AnimationGraphHandle(Handle<AnimationGraph>)` on the same entity as the `AnimationPlayer` (`:139`). Without it the player animates nothing.
- Node types (`:214-243`): `Clip(Handle<AnimationClip>)` (leaf), `Blend` (children weights normalized to 1), `Add` (additive, weights not normalized). Node weight is not inherited downward; it scales the node's accumulated result (`:176-206`).
- Build: `new()` (`:494`), `from_clip(handle) -> (graph, node)` (`:509`), `from_clips(iter) -> (graph, Vec<node>)` (`:522`), `add_clip(h, weight, parent)` (`:537`), `add_clips`, `add_blend(weight, parent)` (`:601`), `add_additive_blend` (`:641`), `*_with_mask` variants (`:556,619,663`), `add_edge/remove_edge` (`:683,691`), `save` to RON (`:722`). Index type `AnimationNodeIndex = NodeIndex<u32>` is what `AnimationPlayer` takes.
- Masks: `AnimationMask = u64`, at most 64 mask groups (`:490`). `graph.add_target_to_mask_group(target_id, group)` puts a bone in a group (`:736`); a node's `mask` bit N = this node and descendants must NOT animate group N (`:181-190`); `node.add_mask_group(n)`/`remove_mask_group(n)` (`:767,776`). Worked example: `examples/animation/animation_masks.rs:241-309` (additive blend root, clips with mask `0x3f`, per-bone groups, then removing `AnimationTargetId`+`AnimatedBy` from bones in no group).
- Weighted blend example: `examples/animation/animation_graph.rs:175-190` (`add_blend(0.5, root)` + `add_clip`).

## AnimationTransitions (`crates/bevy_animation/src/transition.rs`)
- Component beside player + graph handle. `transitions.play(&mut player, node, Duration) -> &mut ActiveAnimation` fades the previous main clip out over the duration and starts the new one (`:78-101`); `Duration::ZERO` switches instantly (`1/0.0` decline clamps the old weight to 0 on the next tick, `:90-111`). Must be the only way clips are started, else the "main" clip is misidentified (`:20-31`).
- Weights use a greedy layer scheme, newest layer first, remainder to the main clip (`:111-143`); finished fade-outs are `stop`ped (`:147-159`).
- `get_main_animation()` (`:104`).

## Events (`animation_event.rs`, `lib.rs:326-350`)
- `#[derive(AnimationEvent, Reflect, Clone)] struct Step;` then `clip.add_event(time_s, Step)` (target = the player entity) or `clip.add_event_to_target(target_id, time_s, Step)` (target = the bone entity); handle with an observer `On<Step>` and `step.trigger().target` (`examples/animation/animated_mesh_events.rs:42-52,165-173`). Mutate the clip via `Assets<AnimationClip>::get_mut`.
- Custom curves/properties: `AnimatableCurve::new(animated_field!(Transform::translation), curve)` (`crates/bevy_animation/src/animation_curves.rs:36`), trait `AnimatableProperty` (`:190`).

## glTF -> AnimationClip (`crates/bevy_gltf/src/loader/mod.rs`)
- `GltfLoaderSettings.load_animations` (default true, `:201-204,233`). The loader walks every glTF animation (`:316`), reads sampler inputs (sparse input accessors are skipped with a warning, `:326`), and builds curves for translation (`:344`), rotation (`:400`), scale (`:457`) and morph weights (`:508`), each with Linear, Step and CubicSpline interpolation. Animations lacking sampler input/output are an error (`:331,552`).
- Curve target = `AnimationTargetId::from_names(path)` where `path` is the node-name path from the scene's top-level node (`:566-569`, `collect_path` in `crates/bevy_gltf/src/loader/gltf_ext/scene.rs:71-87`). Unnamed nodes are named `GltfNode<index>` (`gltf_ext/scene.rs:15-21`); the path root is the scene root node, so renaming or reparenting nodes between exports changes target ids.
- Each clip is a labeled sub-asset `Animation{i}` = `GltfAssetLabel::Animation(i)` (`crates/bevy_gltf/src/label.rs:60,90`; `loader/mod.rs:584-587`), `i` = glTF animation index.
- `Gltf.animations: Vec<Handle<AnimationClip>>` and `Gltf.named_animations: HashMap<Box<str>, Handle<AnimationClip>>` keyed by the glTF animation `name` (`crates/bevy_gltf/src/assets.rs:41-46`; filled at `loader/mod.rs:588-592`). Unnamed animations are only in `animations`; duplicate names overwrite.
- When the scene is instantiated, `AnimationPlayer::default()` is inserted on each top-level scene node that is an animation root (`loader/mod.rs:1105-1110`) and `(AnimationTargetId, AnimatedBy(root))` on every node below it (`:1559-1571`). So the player is on the scene's root NODE entity (a descendant of your `WorldAssetRoot` entity), not on the `WorldAssetRoot` entity. A file with several animated top-level nodes yields several players.
- Skins: `SkinnedMesh{inverse_bindposes, joints}` (`crates/bevy_mesh/src/skinning.rs:16`); morph targets: `MorphWeights` (`crates/bevy_mesh/src/morph.rs:81`), animated by `WeightsCurve` (`crates/bevy_animation/src/morph.rs:21`); the loader sets morph targets and names on the mesh (`loader/mod.rs:800-817`). Skinned bounds policy `GltfSkinnedMeshBoundsPolicy` (`loader/mod.rs:162-164,219`). Examples: `examples/gltf/gltf_skinned_mesh.rs`, `examples/animation/morph_targets.rs`, `custom_skinned_mesh.rs`.
- Extension hooks `on_animation`, `on_animations_collected`, `on_scene_completed`, `on_gltf_node` on `GltfExtensionHandler` (`crates/bevy_gltf/src/loader/extensions/mod.rs:62-227`), used to build a graph at load time: `examples/gltf/gltf_extension_animation_graph.rs:156-285`.

## 0.20 specifics that bite
- `SceneRoot` is now `WorldAssetRoot(Handle<WorldAsset>)` and the ready event is `bevy::world_serialization::WorldInstanceReady` (an `EntityEvent` fired on the entity that holds the root; `crates/bevy_world_serialization/src/components.rs:12-23`, `world_asset_spawner.rs:25-37`). The doc comment says "recently renamed from `SceneRoot`"; the rename is not in the 0.19->0.20 guide text I could fetch (searched the fetched text of the guide; treat as unconfirmed). `Gltf.default_scene: Option<Handle<WorldAsset>>` (`assets.rs:39`). Load a scene as `GltfAssetLabel::Scene(0).from_asset(path)`.
- Observers use `On<E>` (not `Trigger<E>`) and `AnimationEvent` is a derive plus custom trigger (`animation_event.rs:16`).

## Recipes for our GLB pipeline (not compiled)
Select a clip by NAME, then crossfade. Names = the glTF `animations[i].name` written by the exporter; the exporter must name every clip.

```rust
use bevy::{prelude::*, world_serialization::WorldInstanceReady};
use std::{collections::HashMap, time::Duration};

#[derive(Resource)] struct ModelHandle(Handle<Gltf>);
#[derive(Resource)] struct Clips { graph: Handle<AnimationGraph>, nodes: HashMap<String, AnimationNodeIndex> }
#[derive(Resource, Default)] struct PlayRequest(Option<String>);

fn load(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(ModelHandle(assets.load("models/hero.glb")));
    commands.insert_resource(PlayRequest(Some("idle".into())));
}

// Update, run_if(not(resource_exists::<Clips>)): wait for the Gltf and its dependencies, build one graph with every named clip.
fn spawn_when_loaded(
    mut commands: Commands,
    model: Res<ModelHandle>,
    assets: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    if !assets.is_loaded_with_dependencies(&model.0) { return; }
    let Some(gltf) = gltfs.get(&model.0) else { return };
    let mut names: Vec<String> = gltf.named_animations.keys().map(|k| k.to_string()).collect();
    names.sort();
    let (graph, indices) = AnimationGraph::from_clips(
        names.iter().map(|n| gltf.named_animations[n.as_str()].clone()),
    );
    commands.insert_resource(Clips {
        graph: graphs.add(graph),
        nodes: names.into_iter().zip(indices).collect(),
    });
    let Some(scene) = gltf.default_scene.clone() else { return };
    commands.spawn(WorldAssetRoot(scene)).observe(attach_graph);
}

// The AnimationPlayer already exists on the scene root node when WorldInstanceReady fires.
fn attach_graph(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    clips: Res<Clips>,
    request: Res<PlayRequest>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
) {
    for entity in children.iter_descendants(ready.entity) {
        let Ok(mut player) = players.get_mut(entity) else { continue };
        let mut transitions = AnimationTransitions::new();
        if let Some(node) = request.0.as_ref().and_then(|n| clips.nodes.get(n)) {
            transitions.play(&mut player, *node, Duration::ZERO).repeat();
        }
        commands.entity(entity).insert((AnimationGraphHandle(clips.graph.clone()), transitions));
    }
}

// Crossfade: set PlayRequest(Some("run".into())) from any system.
fn crossfade(
    request: Res<PlayRequest>,
    clips: Option<Res<Clips>>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    let Some(clips) = clips else { return };
    if !request.is_changed() { return }
    let Some(node) = request.0.as_ref().and_then(|n| clips.nodes.get(n)) else { return };
    for (mut player, mut transitions) in &mut players {
        transitions.play(&mut player, *node, Duration::from_millis(250)).repeat();
    }
}
```
- One-shot clip then back to idle: `play(...)` without `.repeat()`; poll `player.animation(node).is_some_and(|a| a.is_finished())` or `just_completed()`, then `transitions.play(idle, 250ms)`.
- Alternative that needs no scene-ready observer: `Added<AnimationPlayer>` query in `Update` (`examples/animation/animation_masks.rs:241-249`).
- Same pattern in the checkout: `examples/animation/animated_mesh_control.rs:99-165` (named clips, `Single<(Entity,&mut AnimationPlayer)>`, `Duration::ZERO` first play, 250 ms crossfade at `:212-218`).
- Offscreen frame capture of an animation needs deterministic stepping: advance the clip with `ActiveAnimation::set_seek_time(t)` after `pause()`, or `TimeUpdateStrategy::ManualDuration`; see render.md for the screenshot loop. Uncertainty: `set_seek_time` takes effect at the next `animate_targets` run in `PostUpdate`; verify one-frame latency before relying on it.
