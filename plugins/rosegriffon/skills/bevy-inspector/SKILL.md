---
name: bevy-inspector
description: "Use when working with the `bevy_inspector` crate of bevy: An entity inspector built on bevy_ui and bevy_feathers."
---

# bevy_inspector

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_inspector/` (manifest `third_party/bevy/crates/bevy_inspector/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_inspector
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

An entity inspector built on bevy_ui and bevy_feathers

## Crate docs (`third_party/bevy/crates/bevy_inspector/src/lib.rs`)

An entity inspector built on `bevy_ui` and `bevy_feathers`.

The inspector presents the data gathered by [`bevy_dev_tools::inspection`] as interactive
widgets. This crate provides an entity tree panel, see [`entity_tree`], and a details panel
for the selected entity, see [`details_panel`].

With the `remote` feature the same panels can inspect a separate running app over the Bevy
Remote Protocol, see the `remote` module.

Apps are expected to add `bevy_feathers::FeathersPlugins` themselves, alongside
[`InspectorPlugin`]. The `inspector` cargo feature of `bevy` instead adds the
[`InspectorOverlayPlugin`](overlay::InspectorOverlayPlugin) to `DefaultPlugins`, so that any
app can open the inspector with a key.

## Cargo features (from the manifest)

- `default`
- `remote` (enables 5): Inspect a separate running app over the Bevy Remote Protocol

## Depends on (same ecosystem)

`bevy_annotations`, `bevy_app`, `bevy_camera`, `bevy_color`, `bevy_dev_tools`, `bevy_ecs`, `bevy_feathers`, `bevy_input`, `bevy_input_focus`, `bevy_log`, `bevy_picking`, `bevy_platform`, `bevy_reflect`, `bevy_remote`, `bevy_scene`, `bevy_tasks`, `bevy_text`, `bevy_time`, `bevy_ui`, `bevy_ui_widgets`, `bevy_utils`, `bevy_window`

## Public items by file

- `third_party/bevy/crates/bevy_inspector/src/column_split.rs`: struct ColumnSplit, struct ColumnSplitLeading, struct ColumnSplitHandle, struct ColumnSplitPlugin
- `third_party/bevy/crates/bevy_inspector/src/connection_panel.rs`: struct ConnectionStatusMainLabel, struct ConnectionStatusRenderLabel, struct ConnectionAddressHostInput, struct ConnectionAddressMainPort, struct ConnectionAddressRenderPort, fn connection_panel, fn init_address_input, fn connect_both_clicked, fn connect_main_clicked, fn local_clicked, fn sync_connection_status
- `third_party/bevy/crates/bevy_inspector/src/details_panel.rs`: struct InspectorDetailsBody, struct InspectorDetailsFields, struct ComponentKey, struct InspectorFieldGroupBody, struct InspectorFieldGroupToggle, struct InspectorSegment, struct InspectorDetailsComponent, enum FieldKind, enum FieldValue, struct FieldEdit, struct InspectorField, struct FieldEntry, struct FieldAnnotations, struct DetailsIndex, struct DetailsCollapsed, struct DetailsColumnSplits, fn store_column_splits, struct DetailsPanelSync, fn details_panel, fn details_body, fn inspector_details_toggled, fn sync_details_panel, fn field_entries
- `third_party/bevy/crates/bevy_inspector/src/entity_tree.rs`: struct InspectorUi, struct InspectorTreeView, struct InspectorHeaderRow, struct InspectorEntityRow, struct InspectorRowLabel, struct InspectorRowPopulated, struct TreeRowIndex, struct EntityTreeSync, fn entity_tree_panel, fn entity_tree_body, fn inspector_tree_selected, fn inspector_tree_expanded, fn sync_entity_tree
- `third_party/bevy/crates/bevy_inspector/src/lib.rs`: mod column_split, mod connection_panel, mod details_panel, mod entity_tree, mod overlay, mod remote, struct InspectorSources, enum WorldKind, enum InspectorSource, struct InspectableEntity, struct InspectorSelection, fn entity_tree_visible, fn details_panel_visible, struct InspectorPlugin
- `third_party/bevy/crates/bevy_inspector/src/overlay.rs`: struct InspectorOverlayPlugin, struct InspectorOverlay, enum InspectorOverlayPanel
- `third_party/bevy/crates/bevy_inspector/src/remote/mod.rs`: mod stepping, mod world
- `third_party/bevy/crates/bevy_inspector/src/remote/source.rs`: struct RemoteSource, enum RemoteConnectionState, struct RemoteConnections, struct RemoteConnection, struct RemoteSnapshots, struct RemoteSnapshot, fn sync_remote_sources, fn poll_remote_connections, fn sync_remote_worlds
- `third_party/bevy/crates/bevy_inspector/src/remote/stepping.rs`: enum SteppingCall, enum SteppingStatus, struct RemoteStepping, struct SteppingCursorLabel, struct SteppingButton, fn stepping_controls, fn stepping_button_clicked, fn poll_remote_stepping, fn sync_stepping_controls
- `third_party/bevy/crates/bevy_inspector/src/remote/world.rs`: struct RemoteWorlds, struct RemoteWorld, struct RemoteComponents

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
