---
name: bevy-window
description: "Use when working with the `bevy_window` crate of bevy: Provides windowing functionality for Bevy Engine."
---

# bevy_window

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_window/` (manifest `third_party/bevy/crates/bevy_window/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_window
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides windowing functionality for Bevy Engine

## README (`third_party/bevy/crates/bevy_window/README.md`)

# Bevy Window

## Cargo features (from the manifest)

- `default` (enables 2)
- `bevy_reflect` (enables 5): Adds runtime reflection support using `bevy_reflect`.
- `serialize` (enables 4): Adds serialization support through `serde`.
- `custom_cursor` (enables 2): Enable custom cursor support
- `std` (enables 9): Allows access to the `std` crate. Enabling this feature will prevent compilation on `no_std` targets, but provides access to certain additional features on supported platforms.
- `libm` (enables 2): Uses the `libm` maths library instead of the one provided in `std` and `core`.

## Depends on (same ecosystem)

`bevy_app`, `bevy_asset`, `bevy_color`, `bevy_ecs`, `bevy_image`, `bevy_input`, `bevy_math`, `bevy_platform`, `bevy_reflect`

## Public items by file

- `third_party/bevy/crates/bevy_window/src/cursor/custom_cursor.rs`: struct CustomCursorImage, struct CustomCursorUrl, enum CustomCursor
- `third_party/bevy/crates/bevy_window/src/cursor/mod.rs`: enum CursorSystems, enum CursorIcon
- `third_party/bevy/crates/bevy_window/src/cursor/system_cursor.rs`: enum SystemCursorIcon
- `third_party/bevy/crates/bevy_window/src/display_target.rs`: struct DisplayTarget, enum SurfaceColorSpace, enum TransferFunction, struct ResolvedDisplayTarget, struct SurfaceColorSpaces, struct WindowSurfaceColorSpaces
- `third_party/bevy/crates/bevy_window/src/event.rs`: struct WindowResized, struct RequestRedraw, struct WindowCreated, struct WindowCloseRequested, struct WindowClosed, struct WindowClosing, struct WindowDestroyed, struct CursorMoved, struct RawCursorMoved, struct CursorEntered, struct CursorLeft, enum Ime, struct WindowFocused, struct WindowOccluded, struct WindowScaleFactorChanged, struct WindowBackendScaleFactorChanged, enum FileDragAndDrop, struct WindowMoved, struct WindowThemeChanged, enum AppLifecycle, enum WindowEvent
- `third_party/bevy/crates/bevy_window/src/lib.rs`: mod prelude, struct WindowPlugin, enum ExitCondition
- `third_party/bevy/crates/bevy_window/src/monitor.rs`: struct Monitor, struct PrimaryMonitor, struct HasWindows, struct VideoMode
- `third_party/bevy/crates/bevy_window/src/raw_handle.rs`: struct WindowWrapper, struct RawHandleWrapper, struct ThreadLockedRawWindowHandleWrapper, struct RawHandleWrapperHolder
- `third_party/bevy/crates/bevy_window/src/system.rs`: struct ExitSystems, struct WindowEventSystems, fn send_typed_window_events, fn exit_on_all_closed, fn exit_on_primary_closed, fn close_when_requested
- `third_party/bevy/crates/bevy_window/src/window.rs`: struct PrimaryWindow, enum WindowRef, struct NormalizedWindowRef, struct Window, struct WindowResizeConstraints, struct CursorOptions, enum WindowPosition, struct WindowResolution, enum CursorGrabMode, struct InternalWindowState, enum MonitorSelection, enum VideoModeSelection, enum PresentMode, enum CompositeAlphaMode, enum WindowMode, enum WindowLevel, enum WindowTheme, struct EnabledButtons, struct ClosingWindow, enum ScreenEdge, struct OnMonitor

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
