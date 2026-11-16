---
name: bevy-ui-render
description: "Use when working with the `bevy_ui_render` crate of bevy: Provides rendering functionality for Bevy UI."
---

# bevy_ui_render

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_ui_render/` (manifest `third_party/bevy/crates/bevy_ui_render/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_ui_render
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides rendering functionality for Bevy UI

## README (`third_party/bevy/crates/bevy_ui_render/README.md`)

# Bevy Render UI

## Cargo features (from the manifest)

- `default` (enables 1)
- `serialize` (enables 2)
- `bevy_ui_debug`

## Depends on (same ecosystem)

`bevy_a11y`, `bevy_app`, `bevy_asset`, `bevy_camera`, `bevy_color`, `bevy_core_pipeline`, `bevy_derive`, `bevy_ecs`, `bevy_extract`, `bevy_image`, `bevy_input_focus`, `bevy_math`, `bevy_mesh`, `bevy_platform`, `bevy_reflect`, `bevy_render`, `bevy_shader`, `bevy_sprite`, `bevy_sprite_render`, `bevy_text`, `bevy_transform`, `bevy_ui`, `bevy_utils`

## Public items by file

- `third_party/bevy/crates/bevy_ui_render/src/box_shadow.rs`: struct BoxShadowPlugin, struct UiShadowsBatch, struct BoxShadowMeta, struct BoxShadowPipeline, fn init_box_shadow_pipeline, struct BoxShadowPipelineKey, struct ExtractedBoxShadow, struct ExtractedBoxShadows, fn extract_shadows, fn queue_shadows, fn prepare_shadows, type DrawBoxShadows, struct SetBoxShadowViewBindGroup, struct DrawBoxShadow
- `third_party/bevy/crates/bevy_ui_render/src/clipping.rs`: fn clip_polygon
- `third_party/bevy/crates/bevy_ui_render/src/debug_overlay.rs`: struct UiDebugOptions, struct GlobalUiDebugOptions, fn extract_debug_overlay
- `third_party/bevy/crates/bevy_ui_render/src/gradient.rs`: struct GradientPlugin, struct GradientBatch, struct GradientMeta, struct GradientPipeline, fn init_gradient_pipeline, fn compute_gradient_line_length, struct UiGradientPipelineKey, enum ResolvedGradient, struct ExtractedGradient, struct ExtractedGradients, fn extract_gradients, fn queue_gradient, fn prepare_gradient, type DrawGradientFns, struct SetGradientViewBindGroup, struct DrawGradient
- `third_party/bevy/crates/bevy_ui_render/src/image.rs`: struct ImageNodeAssetChangedSystems
- `third_party/bevy/crates/bevy_ui_render/src/lib.rs`: mod box_shadow, mod clipping, mod render_pass, mod ui_material, mod ui_texture_slice_pipeline, mod prelude, mod stack_z_offsets, enum RenderUiSystems, enum UiAntiAlias, struct BoxShadowSamples, struct UiRenderPlugin, struct UiCameraMap, struct UiCameraMapper, struct ExtractedUiNode, enum NodeType, enum ExtractedUiItem, struct ExtractedGlyph, struct ExtractedUiNodes, fn extract_uinode_changes, fn extract_uinode_background_colors, fn extract_inline_images, fn extract_uinode_images, enum Bevel, fn bevel_colors, ...
- `third_party/bevy/crates/bevy_ui_render/src/pipeline.rs`: struct UiPipeline, fn init_ui_pipeline, struct UiPipelineKey
- `third_party/bevy/crates/bevy_ui_render/src/render_pass.rs`: fn ui_pass, struct TransparentUi, type DrawUi, struct SetUiViewBindGroup, struct SetUiTextureBindGroup, struct DrawUiNode
- `third_party/bevy/crates/bevy_ui_render/src/text.rs`: fn extract_text_cursor, fn extract_preedit_underlines, fn extract_text_placeholder
- `third_party/bevy/crates/bevy_ui_render/src/ui_material.rs`: trait UiMaterial, struct UiMaterialKey, struct MaterialNode
- `third_party/bevy/crates/bevy_ui_render/src/ui_material_pipeline.rs`: struct UiMaterialPlugin, struct UiMaterialMeta, struct UiMaterialVertex, struct UiMaterialBatch, struct UiMaterialPipeline, fn init_ui_material_pipeline, type DrawUiMaterial, struct SetMatUiViewBindGroup, struct SetUiMaterialBindGroup, struct DrawUiMaterialNode, struct ExtractedUiMaterialNode, struct ExtractedUiMaterialNodes, fn extract_ui_material_nodes, fn prepare_uimaterial_nodes, struct PreparedUiMaterial, fn queue_ui_material_nodes
- `third_party/bevy/crates/bevy_ui_render/src/ui_texture_slice_pipeline.rs`: struct UiTextureSlicerPlugin, struct UiTextureSlicerBatch, struct UiTextureSliceMeta, struct UiTextureSliceImageBindGroups, struct UiTextureSlicePipeline, fn init_ui_texture_slice_pipeline, struct UiTextureSlicePipelineKey, struct ExtractedUiTextureSlice, struct ExtractedUiTextureSlices, fn extract_ui_texture_slices, fn queue_ui_slices, fn prepare_ui_slices, type DrawUiTextureSlices, struct SetSlicerViewBindGroup, struct SetSlicerTextureBindGroup, struct DrawSlicer

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
