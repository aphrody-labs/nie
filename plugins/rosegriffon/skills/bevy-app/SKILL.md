---
name: bevy-app
description: "Use when working with the `bevy_app` crate of bevy: Provides core App functionality for Bevy Engine."
---

# bevy_app

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_app/` (manifest `third_party/bevy/crates/bevy_app/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_app
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides core App functionality for Bevy Engine

## README (`third_party/bevy/crates/bevy_app/README.md`)

# Bevy App


This crate is about everything concerning the highest-level, application layer of a [Bevy](https://crates.io/crates/bevy) app.

## Cargo features (from the manifest)

- `default` (enables 4)
- `bevy_reflect` (enables 2): Adds runtime reflection support using `bevy_reflect`.
- `reflect_functions` (enables 3): Extends reflection support to functions.
- `reflect_auto_register` (enables 3)
- `trace` (enables 1): Enables `tracing` integration, allowing spans and other metrics to be reported through that framework.
- `bevy_debug_stepping`: Provides system stepping support, allowing them to be paused, stepped, and other debug operations which can help with diagnosing certain behaviors.
- `error_panic_hook`: Will set the BevyError panic hook, which gives cleaner filtered backtraces when a BevyError is hit.
- `std` (enables 5): Allows access to the `std` crate. Enabling this feature will prevent compilation on `no_std` targets, but provides access to certain additional features on supported platforms.
- `critical-section` (enables 3): `critical-section` provides the building blocks for synchronization primitives on all platforms, including `no_std`.
- `web` (enables 5): Enables use of browser APIs. Note this is currently only applicable on `wasm32` architectures.
- `hotpatching` (enables 3)

## Depends on (same ecosystem)

`bevy_derive`, `bevy_ecs`, `bevy_platform`, `bevy_reflect`, `bevy_tasks`, `bevy_utils`, `dioxus-devtools`

## Public items by file

- `third_party/bevy/crates/bevy_app/src/app.rs`: type InternedAppLabel, struct App, struct OnAppExitSystems, enum AppExit
- `third_party/bevy/crates/bevy_app/src/hierarchy.rs`: struct ValidateParentHasComponentPlugin, struct ValidateParentHasComponentSystems
- `third_party/bevy/crates/bevy_app/src/hotpatch.rs`: struct HotPatchPlugin
- `third_party/bevy/crates/bevy_app/src/lib.rs`: mod hotpatch, mod prelude
- `third_party/bevy/crates/bevy_app/src/main_schedule.rs`: struct Main, struct PreStartup, struct Startup, struct PostStartup, struct First, struct PreUpdate, struct RunFixedMainLoop, struct FixedFirst, struct FixedPreUpdate, struct FixedUpdate, struct FixedPostUpdate, struct FixedLast, struct FixedMain, struct Update, struct SpawnScene, struct PostUpdate, struct Last, struct AnimationSystems, enum SceneSpawnerSystems, struct MainScheduleOrder, struct MainSchedulePlugin, struct FixedMainScheduleOrder, enum RunFixedMainLoopSystems, struct TransformGizmoRenderStep
- `third_party/bevy/crates/bevy_app/src/panic_handler.rs`: struct PanicHandlerPlugin
- `third_party/bevy/crates/bevy_app/src/plugin.rs`: trait Plugin, enum PluginsState, trait Plugins
- `third_party/bevy/crates/bevy_app/src/plugin_group.rs`: trait PluginGroup, struct PluginGroupBuilder, struct NoopPluginGroup, macro plugin_group!
- `third_party/bevy/crates/bevy_app/src/propagate.rs`: struct HierarchyPropagatePlugin, struct Propagate, struct PropagateTemplate, struct PropagateOver, struct PropagateStop, struct PropagateSet, struct Inherited, fn update_source, fn on_r_inserted, fn on_r_removed, fn update_removed_limit, fn propagate_inherited, fn propagate_output
- `third_party/bevy/crates/bevy_app/src/schedule_runner.rs`: enum RunMode, struct ScheduleRunnerPlugin
- `third_party/bevy/crates/bevy_app/src/sub_app.rs`: struct SubApp, struct SubApps
- `third_party/bevy/crates/bevy_app/src/task_pool_plugin.rs`: struct TaskPoolPlugin, struct TaskPoolThreadAssignmentPolicy, struct TaskPoolOptions
- `third_party/bevy/crates/bevy_app/src/terminal_ctrl_c_handler.rs`: struct TerminalCtrlCHandlerPlugin

## Examples in the source tree

- `third_party/bevy/examples/remote/client.rs`

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
