---
name: bevy-animation
description: "Use when working with the `bevy_animation` crate of bevy: Provides animation functionality for Bevy Engine."
---

# bevy_animation

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_animation/` (manifest `third_party/bevy/crates/bevy_animation/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_animation
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides animation functionality for Bevy Engine

## README (`third_party/bevy/crates/bevy_animation/README.md`)

# Bevy Animation

## Depends on (same ecosystem)

`bevy_animation_macros`, `bevy_app`, `bevy_asset`, `bevy_color`, `bevy_curve`, `bevy_derive`, `bevy_ecs`, `bevy_math`, `bevy_mesh`, `bevy_platform`, `bevy_reflect`, `bevy_time`, `bevy_transform`, `bevy_utils`

## Public items by file

- `third_party/bevy/crates/bevy_animation/src/animation_curves.rs`: trait BlendableProperty, struct AnimatedField, trait AnimationCompatibleCurve, struct BlendableCurve, struct BlendableCurveEvaluator, trait AnimationCurve, enum EvaluatorId, trait AnimationCurveEvaluator, struct BlendableKeyframeCurve, macro animated_field!
- `third_party/bevy/crates/bevy_animation/src/animation_event.rs`: trait AnimationEvent, struct AnimationEventTrigger
- `third_party/bevy/crates/bevy_animation/src/blendable.rs`: struct BlendInput, trait Blendable, fn interpolate_with_cubic_bezier
- `third_party/bevy/crates/bevy_animation/src/gltf_curves.rs`: struct SteppedKeyframeCurve, struct CubicKeyframeCurve, struct CubicRotationCurve, struct WideLinearKeyframeCurve, struct WideSteppedKeyframeCurve, struct WideCubicKeyframeCurve, enum WideKeyframeCurveError
- `third_party/bevy/crates/bevy_animation/src/graph.rs`: struct BlendGraph, struct BlendGraphHandle, type BlendDiGraph, type BlendNodeIndex, struct BlendGraphNode, enum BlendNodeType, struct BlendGraphAssetLoader, enum BlendGraphSaveError, enum BlendGraphLoadError, struct ThreadedBlendGraphs, struct ThreadedBlendGraph, struct ThreadedBlendSubgraph, struct SerializedBlendGraph, struct SerializedBlendGraphNode, enum SerializedBlendNodeType, type AnimationMask, struct NonPathHandleError
- `third_party/bevy/crates/bevy_animation/src/lib.rs`: mod animation_curves, mod blendable, mod gltf_curves, mod graph, mod transition, mod prelude, struct VariableCurve, struct AnimationClip, type AnimationCurves, struct AnimationTargetId, struct AnimatedBy, enum RepeatAnimation, enum AnimationEvaluationError, struct ActiveAnimation, struct AnimationPlayer, struct AnimationEvaluationState, fn advance_animations, type AnimationEntityMut, fn animate_targets, struct AnimationPlugin
- `third_party/bevy/crates/bevy_animation/src/morph.rs`: struct WeightsCurve, struct WeightsCurveSample
- `third_party/bevy/crates/bevy_animation/src/transition.rs`: struct AnimationTransitions, struct AnimationTransition, fn advance_transitions, fn expire_completed_transitions

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
