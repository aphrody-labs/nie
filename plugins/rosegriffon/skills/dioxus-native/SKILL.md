---
name: dioxus-native
description: "Use when working with the `dioxus-native` crate of dioxus: Native renderer for Dioxus based on blitz."
---

# dioxus-native

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/native/` (manifest `third_party/dioxus/packages/native/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/native
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Orientation

Dioxus renderer that draws through Blitz instead of a WebView (`third_party/dioxus/packages/native`, DOM bridge in `packages/native-dom`). Status in the plan files: experimental; the desktop and mobile stable path still uses a WebView.

## Purpose

Native renderer for Dioxus based on blitz

## Crate docs (`third_party/dioxus/packages/native/src/lib.rs`)

A native renderer for Dioxus.

## Feature flags
 - `default`: Enables the features listed below.
 - `accessibility`: Enables [`accesskit`](https://docs.rs/accesskit/latest/accesskit/) accessibility support.
 - `hot-reload`: Enables hot-reloading of Dioxus RSX.
 - `menu`: Enables the [`muda`](https://docs.rs/muda/latest/muda/) menubar.
 - `tracing`: Enables tracing support.

## Cargo features (from the manifest)

- `default` (enables 12)
- `prelude` (enables 6)
- `svg` (enables 2): DOM features
- `floats` (enables 1)
- `autofocus` (enables 1)
- `system-fonts` (enables 1)
- `complex-scripts` (enables 1): complex-scripts: enable dictionary-based line-breaking
- `woff` (enables 1): WOFF/WOFF2 decoder. Default-on so wasm callers (which can't use system fonts) can bundle compressed fonts without extra setup.
- `text-transform-icu` (enables 1)
- `writing-mode` (enables 2)
- `clipboard` (enables 1): Shell Features
- `file-dialog` (enables 1)
- `accessibility` (enables 2)
- `data-uri` (enables 1)
- `vello` (enables 2): Renderer features
- `vello-hybrid` (enables 2)
- `skia` (enables 1)
- `skia-pixels` (enables 2)
- `skia-softbuffer` (enables 2)
- `skia-raster-base` (enables 1)
- `vello-cpu` (enables 1)
- `vello-cpu-pixels` (enables 2)
- `vello-cpu-softbuffer` (enables 2)
- `vello-cpu-base` (enables 1)
- `font-embolden` (enables 1)
- `apple-font-embolden` (enables 1)
- `scrollbars` (enables 1)
- `net` (enables 2): Other features No-op on wasm32 (blitz_net::Provider needs tokio's threaded runtime); launch_cfg falls back to the in-process DioxusNativeNetProvider there.
- `html` (enables 1)
- `hot-reload` (enables 1): Dev
- `log-times` (enables 2): Debug/Logging
- `log-phase-times` (enables 1)
- `log-frame-times` (enables 4)
- `tracing` (enables 6)

## Depends on (same ecosystem)

`blitz-dom`, `blitz-html`, `blitz-net`, `blitz-paint`, `blitz-shell`, `blitz-traits`, `dioxus-asset-resolver`, `dioxus-cli-config`, `dioxus-devtools`, `dioxus-document`, `dioxus-history`, `dioxus-hooks`, `dioxus-html`, `dioxus-native-dom`, `dioxus-signals`, `dioxus-stores`, `dioxus-vdom`, `dioxus-vdom-macro`, `manganis`

## Public items by file

- `third_party/dioxus/packages/native/src/assets.rs`: struct DioxusNativeNetProvider
- `third_party/dioxus/packages/native/src/config.rs`: struct Config
- `third_party/dioxus/packages/native/src/contexts.rs`: struct DioxusNativeDocument
- `third_party/dioxus/packages/native/src/dioxus_application.rs`: enum DioxusNativeEvent, struct DioxusNativeApplication
- `third_party/dioxus/packages/native/src/dioxus_renderer.rs`: struct RendererOptions, struct DioxusNativeWindowRenderer
- `third_party/dioxus/packages/native/src/event_handlers.rs`: struct WinitEventHandlerId
- `third_party/dioxus/packages/native/src/hooks.rs`: fn use_window_event, fn use_back_button
- `third_party/dioxus/packages/native/src/lib.rs`: mod prelude, fn set_android_app, fn current_android_app, fn use_window, fn use_raw_window_handle, fn launch, fn launch_cfg, fn launch_cfg_with_props

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
