---
name: bevy-ecs-macros
description: "Use when working with the `bevy_ecs_macros` crate of bevy: Bevy ECS Macros."
---

# bevy_ecs_macros

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_ecs/macros/` (manifest `third_party/bevy/crates/bevy_ecs/macros/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_ecs/macros
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Bevy ECS Macros

## Crate docs (`third_party/bevy/crates/bevy_ecs/macros/src/lib.rs`)

Macros for deriving ECS traits.

## Depends on (same ecosystem)

`bevy_ecs_macro_logic`, `bevy_macro_utils`

## Public items by file

- `third_party/bevy/crates/bevy_ecs/macros/src/event.rs`: fn derive_event, fn derive_entity_event
- `third_party/bevy/crates/bevy_ecs/macros/src/lib.rs`: derive Bundle, fn derive_bundle, derive MapEntities, fn derive_map_entities, derive SystemParam, fn derive_system_param, derive QueryData, fn derive_query_data, derive QueryFilter, fn derive_query_filter, derive ScheduleLabel, fn derive_schedule_label, derive SystemSet, fn derive_system_set, derive Event, fn derive_event, derive EntityEvent, fn derive_entity_event, derive Message, fn derive_message, derive Resource, fn derive_resource, derive SettingsGroup, fn derive_settings_group, ...
- `third_party/bevy/crates/bevy_ecs/macros/src/message.rs`: fn derive_message
- `third_party/bevy/crates/bevy_ecs/macros/src/query_data.rs`: fn derive_query_data_impl
- `third_party/bevy/crates/bevy_ecs/macros/src/query_filter.rs`: fn derive_query_filter_impl
- `third_party/bevy/crates/bevy_ecs/macros/src/resource.rs`: fn derive_resource

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
