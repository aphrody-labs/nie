---
name: bevy-winit
description: "Use when working with the `bevy_winit` crate of bevy: A winit window and input backend for Bevy Engine."
---

# bevy_winit

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_winit/` (manifest `third_party/bevy/crates/bevy_winit/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_winit
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

A winit window and input backend for Bevy Engine

## README (`third_party/bevy/crates/bevy_winit/README.md`)

# Bevy Winit

## Cargo features (from the manifest)

- `default` (enables 1)
- `trace`
- `wayland` (enables 2)
- `x11` (enables 1)
- `accesskit_unix` (enables 3)
- `serialize` (enables 3)
- `android-native-activity` (enables 1)
- `android-game-activity` (enables 1)
- `custom_cursor` (enables 5)
- `linux_game_mode` (enables 1)

## Depends on (same ecosystem)

`bevy_a11y`, `bevy_app`, `bevy_asset`, `bevy_derive`, `bevy_ecs`, `bevy_image`, `bevy_input`, `bevy_input_focus`, `bevy_log`, `bevy_math`, `bevy_platform`, `bevy_reflect`, `bevy_tasks`, `bevy_window`

## Public items by file

- `third_party/bevy/crates/bevy_winit/src/accessibility.rs`: struct AccessKitAdapters, struct WinitActionRequestHandlers, struct WinitActionRequestHandler, struct AccessKitPlugin
- `third_party/bevy/crates/bevy_winit/src/converters.rs`: fn convert_keyboard_input, fn convert_element_state, fn convert_mouse_button, fn convert_touch_phase, fn convert_touch_input, fn convert_physical_native_key_code, fn convert_physical_key_code, fn convert_logical_key, fn convert_native_key, fn convert_system_cursor_icon, fn convert_window_level, fn convert_winit_theme, fn convert_window_theme, fn convert_enabled_buttons, fn convert_resize_direction
- `third_party/bevy/crates/bevy_winit/src/cursor/custom_cursor.rs`: struct WinitCustomCursorCache, enum CustomCursorCacheKey
- `third_party/bevy/crates/bevy_winit/src/cursor/mod.rs`: enum CursorSource, struct PendingCursor
- `third_party/bevy/crates/bevy_winit/src/game_mode.rs`: struct GameModePlugin
- `third_party/bevy/crates/bevy_winit/src/lib.rs`: mod accessibility, mod converters, struct WinitPlugin, enum WinitUserEvent, struct RawWinitWindowEvent, struct EventLoopProxyWrapper, struct DisplayHandleWrapper, type CreateWindowParams, type CreateMonitorParams
- `third_party/bevy/crates/bevy_winit/src/state.rs`: fn winit_runner
- `third_party/bevy/crates/bevy_winit/src/system.rs`: fn create_windows, fn create_monitors, struct WinitWindowPressedKeys
- `third_party/bevy/crates/bevy_winit/src/winit_config.rs`: struct WinitSettings, enum UpdateMode
- `third_party/bevy/crates/bevy_winit/src/winit_monitors.rs`: struct WinitMonitors
- `third_party/bevy/crates/bevy_winit/src/winit_windows.rs`: struct WinitWindows, fn get_selected_videomode, fn winit_window_position, fn select_monitor

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
