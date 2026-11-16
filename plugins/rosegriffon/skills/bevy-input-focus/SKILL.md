---
name: bevy-input-focus
description: "Use when working with the `bevy_input_focus` crate of bevy: Keyboard focus management."
---

# bevy_input_focus

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_input_focus/` (manifest `third_party/bevy/crates/bevy_input_focus/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_input_focus
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Keyboard focus management

## README (`third_party/bevy/crates/bevy_input_focus/README.md`)

# Bevy Input Focus

## Cargo features (from the manifest)

- `default` (enables 6)
- `gamepad` (enables 1)
- `keyboard` (enables 1)
- `mouse` (enables 1)
- `bevy_reflect` (enables 6): Adds runtime reflection support using `bevy_reflect`.
- `serialize` (enables 4): Adds serialization support through `serde`.
- `std` (enables 6): Allows access to the `std` crate. Enabling this feature will prevent compilation on `no_std` targets, but provides access to certain additional features on supported platforms.
- `critical-section` (enables 4): `critical-section` provides the building blocks for synchronization primitives on all platforms, including `no_std`.
- `libm` (enables 2): Uses the `libm` maths library instead of the one provided in `std` and `core`.

## Depends on (same ecosystem)

`bevy_app`, `bevy_camera`, `bevy_ecs`, `bevy_input`, `bevy_math`, `bevy_picking`, `bevy_reflect`, `bevy_window`

## Public items by file

- `third_party/bevy/crates/bevy_input_focus/src/autofocus.rs`: struct AutoFocus
- `third_party/bevy/crates/bevy_input_focus/src/directional_navigation.rs`: struct DirectionalNavigationPlugin, struct AutoNavigationConfig, enum NavNeighbor, struct NavNeighbors, struct DirectionalNavigationMap, struct DirectionalNavigation, enum DirectionalNavigationError, struct FocusableArea, trait Navigable, fn auto_generate_navigation_edges
- `third_party/bevy/crates/bevy_input_focus/src/gained_and_lost.rs`: enum FocusCause, struct FocusGained, struct FocusLost, fn process_recorded_focus_changes
- `third_party/bevy/crates/bevy_input_focus/src/lib.rs`: mod directional_navigation, mod navigator, mod pointer_focus, mod tab_navigation, struct Focusable, struct InputFocus, struct InputFocusVisible, struct FocusedInput, struct AcquireFocus, struct WindowTraversal, fn on_window_acquire_focus_clear, fn acquire_focus, struct InputFocusPlugin, struct InputDispatchPlugin, enum InputFocusSystems, fn set_initial_focus, fn dispatch_focused_input, trait IsFocused, struct IsFocusedHelper
- `third_party/bevy/crates/bevy_input_focus/src/navigator.rs`: fn find_best_candidate
- `third_party/bevy/crates/bevy_input_focus/src/pointer_focus.rs`: struct PointerFocusPlugin
- `third_party/bevy/crates/bevy_input_focus/src/tab_navigation.rs`: struct TabIndex, struct TabGroup, enum NavAction, enum TabNavigationError, struct TabNavigation, struct TabNavigationPlugin, fn handle_tab_navigation

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
