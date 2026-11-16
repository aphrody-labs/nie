---
name: bevy-input
description: "Use when working with the `bevy_input` crate of bevy: Provides input functionality for Bevy Engine."
---

# bevy_input

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_input/` (manifest `third_party/bevy/crates/bevy_input/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_input
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides input functionality for Bevy Engine

## README (`third_party/bevy/crates/bevy_input/README.md`)

# Bevy Input

## Cargo features (from the manifest)

- `default` (enables 4)
- `mouse`: Input sources
- `keyboard`
- `gamepad`
- `touch`
- `gestures`
- `bevy_reflect` (enables 4): Adds runtime reflection support using `bevy_reflect`.
- `serialize` (enables 5): Adds serialization support through `serde`.
- `smol_str` (enables 2): Uses the small-string optimization provided by `smol_str`.
- `std` (enables 5): Allows access to the `std` crate. Enabling this feature will prevent compilation on `no_std` targets, but provides access to certain additional features on supported platforms.
- `critical-section` (enables 4): `critical-section` provides the building blocks for synchronization primitives on all platforms, including `no_std`.
- `libm` (enables 1): Uses the `libm` maths library instead of the one provided in `std` and `core`.

## Depends on (same ecosystem)

`bevy_app`, `bevy_ecs`, `bevy_math`, `bevy_platform`, `bevy_reflect`

## Public items by file

- `third_party/bevy/crates/bevy_input/src/axis.rs`: struct Axis
- `third_party/bevy/crates/bevy_input/src/button_input.rs`: struct ButtonInput
- `third_party/bevy/crates/bevy_input/src/common_conditions.rs`: fn input_toggle_active, fn input_pressed, fn input_just_pressed, fn input_just_released
- `third_party/bevy/crates/bevy_input/src/gamepad.rs`: enum GamepadEvent, enum RawGamepadEvent, struct RawGamepadButtonChangedEvent, struct RawGamepadAxisChangedEvent, struct GamepadConnectionEvent, struct GamepadButtonStateChangedEvent, struct GamepadButtonChangedEvent, struct GamepadAxisChangedEvent, enum AxisSettingsError, enum ButtonSettingsError, struct Gamepad, enum GamepadButton, enum GamepadAxis, enum GamepadInput, struct GamepadSettings, struct ButtonSettings, struct AxisSettings, struct ButtonAxisSettings, fn gamepad_connection_system, enum GamepadConnection, fn gamepad_event_processing_system, struct GamepadRumbleIntensity, enum GamepadRumbleRequest
- `third_party/bevy/crates/bevy_input/src/gestures.rs`: struct PinchGesture, struct RotationGesture, struct DoubleTapGesture, struct PanGesture
- `third_party/bevy/crates/bevy_input/src/keyboard.rs`: struct KeyboardInput, struct KeyboardFocusLost, fn keyboard_input_system, enum NativeKeyCode, enum KeyCode, enum NativeKey, enum Key
- `third_party/bevy/crates/bevy_input/src/lib.rs`: mod common_conditions, mod gamepad, mod gestures, mod keyboard, mod mouse, mod touch, mod prelude, struct InputPlugin, struct InputSystems, enum ButtonState
- `third_party/bevy/crates/bevy_input/src/mouse.rs`: struct MouseButtonInput, enum MouseButton, struct MouseMotion, enum MouseScrollUnit, struct MouseScrollPixelsPerLine, struct MouseWheel, fn mouse_button_input_system, struct AccumulatedMouseMotion, struct AccumulatedMouseScroll, fn accumulate_mouse_motion_system, fn accumulate_mouse_scroll_system
- `third_party/bevy/crates/bevy_input/src/touch.rs`: struct TouchInput, enum ForceTouch, enum TouchPhase, struct Touch, struct Touches, fn touch_screen_input_system

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
