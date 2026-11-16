---
name: bevy-ui
description: "Use when working with the `bevy_ui` crate of bevy: A custom ECS-driven UI framework built specifically for Bevy Engine."
---

# bevy_ui

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_ui/` (manifest `third_party/bevy/crates/bevy_ui/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_ui
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Orientation

Bevy UI nodes are laid out with Taffy (`taffy` dependency in `third_party/bevy/crates/bevy_ui/Cargo.toml`), the same Taffy checkout Blitz and Dioxus native use here (`third_party/taffy`), so one layout engine serves Bevy scenes and Dioxus views.

## Purpose

A custom ECS-driven UI framework built specifically for Bevy Engine

## README (`third_party/bevy/crates/bevy_ui/README.md`)

# Bevy UI

## Cargo features (from the manifest)

- `default`
- `serialize` (enables 4)
- `bevy_picking` (enables 2)

## Depends on (same ecosystem)

`bevy_a11y`, `bevy_app`, `bevy_asset`, `bevy_camera`, `bevy_color`, `bevy_derive`, `bevy_ecs`, `bevy_image`, `bevy_input`, `bevy_input_focus`, `bevy_log`, `bevy_math`, `bevy_picking`, `bevy_platform`, `bevy_reflect`, `bevy_sprite`, `bevy_text`, `bevy_time`, `bevy_transform`, `bevy_utils`, `bevy_window`, `taffy`

## Public items by file

- `third_party/bevy/crates/bevy_ui/src/accessibility.rs`: struct AccessibilityUiSystems, struct AccessibleLabel
- `third_party/bevy/crates/bevy_ui/src/auto_directional_navigation.rs`: struct AutoDirectionalNavigation, struct AutoDirectionalNavigator
- `third_party/bevy/crates/bevy_ui/src/geometry.rs`: struct UiRect, struct UiPosition, struct CornerRadius
- `third_party/bevy/crates/bevy_ui/src/gradients.rs`: struct ColorStop, struct AngularColorStop, struct LinearGradient, struct RadialGradient, struct ConicGradient, enum Gradient, struct BackgroundGradient, struct BorderGradient, enum RadialGradientShape, enum InterpolationColorSpace, trait InColorSpace
- `third_party/bevy/crates/bevy_ui/src/interaction_states.rs`: struct InteractionDisabled, struct Pressed, struct Checkable, struct Checked, struct Selectable, struct Selected, struct Expandable, struct Expanded
- `third_party/bevy/crates/bevy_ui/src/layout/clipping.rs`: fn update_clipping_system
- `third_party/bevy/crates/bevy_ui/src/layout/convert.rs`: fn update_taffy_style_from_node
- `third_party/bevy/crates/bevy_ui/src/layout/debug.rs`: fn print_ui_layout_tree
- `third_party/bevy/crates/bevy_ui/src/layout/layout_tree.rs`: struct TaffyStyle, struct ComputedLayout, fn compute_layout, fn get_text_buffer
- `third_party/bevy/crates/bevy_ui/src/layout/mod.rs`: mod clipping, mod debug, mod layout_tree, struct UiTreeDirty, struct UiRoots, enum LayoutError, fn clear_transient_dirty_flags, fn update_ui_roots, fn sync_font_size_to_em_size, fn sync_taffy_styles_with_nodes, fn mark_dirty_ui_trees, fn ui_layout_system, fn update_computed_nodes, fn update_border_radius
- `third_party/bevy/crates/bevy_ui/src/lib.rs`: mod auto_directional_navigation, mod interaction_states, mod measurement, mod update, mod widget, mod gradients, mod picking_backend, mod ui_transform, mod prelude, struct UiPlugin, enum UiSystems, struct UiScale
- `third_party/bevy/crates/bevy_ui/src/measurement.rs`: struct MeasureArgs, struct ResolvedAxis, trait Measure, enum NodeMeasure, struct FixedMeasure, struct ContentSize
- `third_party/bevy/crates/bevy_ui/src/picking_backend.rs`: struct UiPickingCamera, struct UiPickingSettings, struct UiPickingPlugin, struct NodeQuery, fn ui_picking
- `third_party/bevy/crates/bevy_ui/src/scroll_into_view.rs`: struct ScrollIntoView, enum ScrollIntoViewContainer
- `third_party/bevy/crates/bevy_ui/src/stack.rs`: struct ComputedStackIndex, struct UiStack, fn ui_stack_system
- `third_party/bevy/crates/bevy_ui/src/ui_node.rs`: struct ComputedNode, struct ScrollPosition, struct IgnoreScroll, struct Node, enum InlineDirection, enum AlignItems, enum JustifyItems, enum AlignSelf, enum JustifySelf, enum AlignContent, enum JustifyContent, enum Display, enum BoxSizing, enum FlexDirection, struct Overflow, enum OverflowAxis, struct OverflowClipMargin, enum VisualBox, enum PositionType, enum FlexWrap, enum GridAutoFlow, enum MinTrackSizingFunction, enum MaxTrackSizingFunction, struct GridTrack, ...
- `third_party/bevy/crates/bevy_ui/src/ui_transform.rs`: struct Val2, struct UiTransform, struct UiGlobalTransform
- `third_party/bevy/crates/bevy_ui/src/update.rs`: fn propagate_ui_target_cameras
- `third_party/bevy/crates/bevy_ui/src/val.rs`: struct ValContext, enum Val, enum ValParseError, enum ValArithmeticError, trait ValNum, fn auto, fn px, fn em, fn rem, fn percent, fn vw, fn vh, fn vmin, fn vmax
- `third_party/bevy/crates/bevy_ui/src/widget/image.rs`: struct ImageNode, enum NodeImageMode, struct ImageNodeSize, struct ImageMeasure, fn update_image_content_size_system
- `third_party/bevy/crates/bevy_ui/src/widget/inline_image.rs`: struct InlineImage, fn update_inline_image_boxes
- `third_party/bevy/crates/bevy_ui/src/widget/label.rs`: struct Label
- `third_party/bevy/crates/bevy_ui/src/widget/text.rs`: struct TextNodeFlags, struct Text, struct TextShadow, type TextUiReader, type TextUiWriter, struct TextMeasure, fn measure_text_system, fn text_system
- `third_party/bevy/crates/bevy_ui/src/widget/text_input_layout.rs`: struct EditableTextContentSizeState, fn update_editable_text_content_size, fn update_editable_text_styles, fn sync_editable_text_viewports, fn update_editable_text_layout
- `third_party/bevy/crates/bevy_ui/src/widget/viewport.rs`: struct ViewportNode, fn viewport_picking, fn update_viewport_render_target_size

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
