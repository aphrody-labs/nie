---
name: dioxus-desktop
description: "Use when working with the `dioxus-desktop` crate of dioxus: WebView renderer for Dioxus."
---

# dioxus-desktop

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/desktop/` (manifest `third_party/dioxus/packages/desktop/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/desktop
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

WebView renderer for Dioxus

## README (`third_party/dioxus/packages/desktop/README.md`)

# Dioxus Desktop (webview)

[Website](https://dioxuslabs.com) |
[Guides](https://dioxuslabs.com/learn/0.7/) |
[API Docs](https://docs.rs/dioxus-desktop/latest/dioxus_desktop) |
[Chat](https://discord.gg/XgGxMSkvUM)

## Overview

`dioxus-desktop` provides a webview-based desktop renderer for the Dioxus VirtualDom.

This requires that webview is installed on the target system. WebView is installed by default on macOS and iOS devices, but might not come preinstalled on Windows or Linux devices. To fix these issues, follow the [instructions in the guide](guide-url).

## Features

- Simple, one-line launch for desktop apps
- Dioxus VirtualDom running on a native thread
- Full HTML/CSS support via `wry` and `tao`
- Exposed `window` and `Proxy` types from tao for direct window manipulation
- Helpful hooks for accessing the window, WebView, and running javascript.

## Contributing

- Report issues on our [issue tracker](https://github.com/dioxuslabs/dioxus/issues).
- Join the discord and ask questions!

## License

This project is licensed under the [MIT license].

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Dioxus by you shall be licensed as MIT without any additional
terms or conditions.

## Cargo features (from the manifest)

- `default` (enables 3)
- `tokio_runtime` (enables 1)
- `fullscreen`: No-op since wry 0.56, which always enables fullscreen support. Kept for compatibility.
- `devtools` (enables 3)
- `transparent`: No-op since wry 0.56, which always enables transparency support. Kept for compatibility.
- `gnu`

## Depends on (same ecosystem)

`dioxus-asset-resolver`, `dioxus-cli-config`, `dioxus-devtools`, `dioxus-document`, `dioxus-history`, `dioxus-hooks`, `dioxus-html`, `dioxus-interpreter-js`, `dioxus-signals`, `dioxus-vdom`, `dioxus-vdom-macro`, `generational-box`

## Public items by file

- `third_party/dioxus/packages/desktop/src/android_sync_lock.rs`: fn android_runtime_lock
- `third_party/dioxus/packages/desktop/src/assets.rs`: type AssetRequest, struct AssetHandler, struct AssetHandlerRegistry
- `third_party/dioxus/packages/desktop/src/config.rs`: enum WindowCloseBehaviour, struct WindowConfig, struct Config
- `third_party/dioxus/packages/desktop/src/default_icon.rs`: trait DioxusIconTrait, fn default_icon, fn icon_from_memory, fn icon_from_path
- `third_party/dioxus/packages/desktop/src/desktop_context.rs`: fn window, fn app, type DesktopContext, type WeakDesktopContext, struct DesktopService, struct PendingDesktopWindow
- `third_party/dioxus/packages/desktop/src/desktop_state.rs`: struct DesktopAppContext, struct DesktopWindowContext
- `third_party/dioxus/packages/desktop/src/document.rs`: struct DesktopDocument
- `third_party/dioxus/packages/desktop/src/element.rs`: struct DesktopElement
- `third_party/dioxus/packages/desktop/src/event_handlers.rs`: struct WryEventHandler, struct WindowEventHandlers
- `third_party/dioxus/packages/desktop/src/file_upload.rs`: struct NativeFileHover, struct DesktopFileData, struct DesktopDataTransfer
- `third_party/dioxus/packages/desktop/src/hooks.rs`: fn use_window, fn use_app, fn use_wry_event_handler, fn use_muda_event_handler, fn use_tray_menu_event_handler, fn use_tray_icon_event_handler, fn use_asset_handler, fn use_global_shortcut
- `third_party/dioxus/packages/desktop/src/ipc.rs`: enum UserWindowEvent, struct IpcMessage, enum IpcMethod
- `third_party/dioxus/packages/desktop/src/launch.rs`: fn launch_virtual_dom_blocking, fn launch_virtual_dom, fn launch
- `third_party/dioxus/packages/desktop/src/lib.rs`: mod launch, mod trayicon
- `third_party/dioxus/packages/desktop/src/menubar.rs`: type DioxusMenu, type DioxusMenuIcon, fn init_menu_bar, fn default_menu_bar
- `third_party/dioxus/packages/desktop/src/mobile_shortcut.rs`: struct Accelerator, struct HotKey, struct GlobalHotKeyManager, enum HotkeyError, struct GlobalHotKeyEvent, enum HotKeyState
- `third_party/dioxus/packages/desktop/src/query.rs`: enum QueryError
- `third_party/dioxus/packages/desktop/src/shortcut.rs`: struct ShortcutHandle, enum ShortcutRegistryError, trait IntoAccelerator, trait IntoModifiersState, trait IntoKeyCode
- `third_party/dioxus/packages/desktop/src/trayicon.rs`: type DioxusTrayMenu, type DioxusTrayIcon, type DioxusTray, fn init_tray_icon, fn default_tray_icon, fn use_tray_icon
- `third_party/dioxus/packages/desktop/src/waker.rs`: fn create_dom_waker
- `third_party/dioxus/packages/desktop/src/webview.rs`: struct SynchronousEventResponse
- `third_party/dioxus/packages/desktop/src/window_component.rs`: struct WindowProps, struct InitialWindowConfig, fn Window

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
