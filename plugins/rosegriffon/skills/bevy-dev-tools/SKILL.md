---
name: bevy-dev-tools
description: "Use when working with the `bevy_dev_tools` crate of bevy: Collection of developer tools for the Bevy Engine."
---

# bevy_dev_tools

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_dev_tools/` (manifest `third_party/bevy/crates/bevy_dev_tools/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_dev_tools
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Collection of developer tools for the Bevy Engine

## README (`third_party/bevy/crates/bevy_dev_tools/README.md`)

# Bevy Developer Tools

## Cargo features (from the manifest)

- `bevy_audio` (enables 1)
- `bevy_ci_testing` (enables 3)
- `screenrecording` (enables 2)
- `render` (enables 4)
- `webgl` (enables 2)
- `webgpu` (enables 2)
- `schedule_data` (enables 3)
- `serialize` (enables 5)

## Depends on (same ecosystem)

`bevy_animation`, `bevy_app`, `bevy_asset`, `bevy_audio`, `bevy_camera`, `bevy_color`, `bevy_core_pipeline`, `bevy_diagnostic`, `bevy_ecs`, `bevy_extract`, `bevy_image`, `bevy_input`, `bevy_light`, `bevy_log`, `bevy_math`, `bevy_mesh`, `bevy_pbr`, `bevy_picking`, `bevy_platform`, `bevy_reflect`, `bevy_render`, `bevy_shader`, `bevy_sprite`, `bevy_state`, `bevy_text`, `bevy_time`, `bevy_transform`, `bevy_ui`, `bevy_ui_render`, `bevy_ui_widgets`, `bevy_utils`, `bevy_window`, `bevy_world_serialization`

## Public items by file

- `third_party/bevy/crates/bevy_dev_tools/src/ci_testing/config.rs`: struct CiTestingConfig, struct CiTestingSetup, struct CiTestingEventOnFrame, enum CiTestingEvent, struct CiTestingCustomEvent
- `third_party/bevy/crates/bevy_dev_tools/src/ci_testing/mod.rs`: struct CiTestingPlugin
- `third_party/bevy/crates/bevy_dev_tools/src/diagnostics_overlay.rs`: struct DiagnosticsOverlay, struct DiagnosticsOverlayStyle, struct DiagnosticsOverlayPlane, struct DiagnosticsOverlayItem, enum DiagnosticsOverlayStatistic, enum DiagnosticsOverlaySystems, struct DiagnosticsOverlayPlugin
- `third_party/bevy/crates/bevy_dev_tools/src/easy_camera.rs`: struct EasyCameraMovementPlugin, struct CameraMovement
- `third_party/bevy/crates/bevy_dev_tools/src/easy_screenshot.rs`: enum ScreenshotFormat, struct EasyScreenshotPlugin, enum Preset, enum Tune, struct EasyScreenRecordPlugin, enum RecordScreen, struct EasyScreenRecordSystems
- `third_party/bevy/crates/bevy_dev_tools/src/fps_overlay.rs`: struct FpsOverlayPlugin, enum FpsOverlaySystems, struct FpsOverlayConfig, struct FrameTimeGraphConfig
- `third_party/bevy/crates/bevy_dev_tools/src/frame_time_graph/mod.rs`: struct FrameTimeGraphPlugin, struct FrameTimeGraphConfigUniform, struct FrametimeGraphMaterial
- `third_party/bevy/crates/bevy_dev_tools/src/infinite_grid.rs`: struct InfiniteGridPlugin, struct InfiniteGrid, struct InfiniteGridSettings
- `third_party/bevy/crates/bevy_dev_tools/src/inspection/component_inspection.rs`: struct ComponentInspection, struct ComponentTypeMetadata, struct ComponentTypeInspection, struct ComponentMetadataMap, enum ComponentInspectionError, struct ComponentInspectionSettings, enum ComponentDetailLevel
- `third_party/bevy/crates/bevy_dev_tools/src/inspection/entity_grouping.rs`: struct EntityGrouping, enum GroupingStrategy, struct NameKey, struct NameEntityKey
- `third_party/bevy/crates/bevy_dev_tools/src/inspection/entity_inspection.rs`: struct EntityInspection, enum EntityInspectionError, struct EntityInspectionSettings
- `third_party/bevy/crates/bevy_dev_tools/src/inspection/extension_methods.rs`: trait WorldInspectionExtensionTrait, trait EntityCommandsInspectionExtensionTrait, trait CommandsInspectionExtensionTrait
- `third_party/bevy/crates/bevy_dev_tools/src/inspection/fuzzy_name_matching.rs`: fn fuzzy_match_component_name, fn fuzzy_match_resource_name, fn fuzzy_match_entity_label
- `third_party/bevy/crates/bevy_dev_tools/src/inspection/label_resolution.rs`: struct LabelDefinitionPriority, struct ComponentLabelData, struct EntityLabel, enum LabelOrigin, fn resolve_label, struct LabelResolutionRegistry, struct LabelResolutionPlugin
- `third_party/bevy/crates/bevy_dev_tools/src/inspection/mod.rs`: mod component_inspection, mod entity_grouping, mod entity_inspection, mod extension_methods, mod fuzzy_name_matching, mod label_resolution, mod reflection_tools, mod resource_inspection, mod serde_conversions, mod world_summary
- `third_party/bevy/crates/bevy_dev_tools/src/inspection/reflection_tools.rs`: fn clone_incomplete, fn component_value_to_string
- `third_party/bevy/crates/bevy_dev_tools/src/inspection/resource_inspection.rs`: struct ResourceInspection, enum ResourceInspectionError, struct ResourceInspectionSettings
- `third_party/bevy/crates/bevy_dev_tools/src/inspection/serde_conversions.rs`: mod component_id, mod archetype_id, mod slice_component_id, mod debug_name, mod option_vec_debug_name, mod storage_type, mod hash_map_component_id_component_type_metadata, fn serialize_spawn_details, fn serialize_option_spawn_details, fn reflect_to_json_value
- `third_party/bevy/crates/bevy_dev_tools/src/inspection/world_summary.rs`: struct SummarySettings, struct ArchetypeSummary, struct WorldSummary, trait WorldSummaryExt, trait CommandsSummaryExt
- `third_party/bevy/crates/bevy_dev_tools/src/lib.rs`: mod ci_testing, mod diagnostics_overlay, mod fps_overlay, mod frame_time_graph, mod inspection, mod picking_debug, mod schedule_data, mod states, mod synthetic_input, mod render_debug, mod infinite_grid, mod world_asset_helpers
- `third_party/bevy/crates/bevy_dev_tools/src/picking_debug.rs`: enum DebugPickingMode, struct DebugPickingPlugin, fn log_message_debug, fn log_message_trace, struct PointerDebugOverlayOf, struct PointerDebugOverlay, fn add_pointer_debug, fn pointer_debug_visibility, struct PointerDebug, fn update_debug_data, fn debug_draw
- `third_party/bevy/crates/bevy_dev_tools/src/render_debug.rs`: struct RenderDebugOverlayPlugin, struct RenderDebugOverlayKeybindings, fn handle_input, fn update_overlay, enum RenderDebugOverlayEvent, struct RenderDebugOverlay, struct GlobalRenderDebugOverlay, enum RenderDebugMode
- `third_party/bevy/crates/bevy_dev_tools/src/schedule_data/mod.rs`: mod plugin, mod serde
- `third_party/bevy/crates/bevy_dev_tools/src/schedule_data/plugin.rs`: struct SerializeSchedulesPlugin, struct SerializeSchedulesSystems, struct SerializeSchedulesFilePath
- `third_party/bevy/crates/bevy_dev_tools/src/schedule_data/serde.rs`: struct AppData, enum DependencyKind, struct DependencyData, struct ScheduleData, struct ComponentData, struct SystemData, struct AccessData, struct AccessFiltersData, struct FilteredAccessData, struct SystemSetData, struct ConditionData, enum ScheduleIndex, struct SystemConflict, enum AccessConflict, struct SystemSetIndex, enum ExtractAppDataError, mod tests
- `third_party/bevy/crates/bevy_dev_tools/src/states.rs`: fn log_transitions
- `third_party/bevy/crates/bevy_dev_tools/src/synthetic_input.rs`: struct InputQueue, struct QueuedInputId, enum ClickTarget, enum ClickError, fn click, enum KeyAction, enum KeyError, fn key, fn type_text, fn write_due_input
- `third_party/bevy/crates/bevy_dev_tools/src/world_asset_helpers.rs`: fn merge_all_mesh_3d

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
