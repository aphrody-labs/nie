---
name: bevy-settings
description: "Use when working with the `bevy-settings` crate of bevy: User settings framework for Bevy Engine."
---

# bevy-settings

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_settings/` (manifest `third_party/bevy/crates/bevy_settings/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_settings
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

User settings framework for Bevy Engine

## Crate docs (`third_party/bevy/crates/bevy_settings/src/lib.rs`)

Framework for saving and loading user settings files in Bevy
applications.

The core of the framework is [`SettingsPlugin`], which
loads and synchronizes settings with the filesystem or browser
local storage, depending on platform.

Settings are loaded into resources that implement [`SettingsGroup`](trait@SettingsGroup),
which is best implemented using the derive macro [`SettingsGroup`](derive@SettingsGroup).
In addition, the resource must have the `#[reflect(SettingsGroup, Default)]` annotation in
order to be saved and loaded by the plugin.

Once all these conditions are met, and when [`SettingsPlugin`] is added, systems can query
for settings using [`Res`] and [`ResMut`] like any other resource.

Refer to [`SettingsPlugin`] for detailed usage information.

## Cargo features (from the manifest)

- `default`

## Depends on (same ecosystem)

`bevy_app`, `bevy_ecs`, `bevy_ecs_macros`, `bevy_log`, `bevy_platform`, `bevy_reflect`, `bevy_tasks`, `bevy_time`

## Public items by file

- `third_party/bevy/crates/bevy_settings/src/lib.rs`: struct SettingsPlugin, trait SettingsGroup, struct ReflectSettingsGroup, enum SaveSettingsSync, enum SaveSettings, struct SaveSettingsDeferred

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
