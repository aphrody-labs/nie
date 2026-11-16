---
name: dioxus-native-dom
description: "Use when working with the `dioxus-native-dom` crate of dioxus: Core headless native renderer for Dioxus based on blitz."
---

# dioxus-native-dom

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/native-dom/` (manifest `third_party/dioxus/packages/native-dom/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/native-dom
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Core headless native renderer for Dioxus based on blitz

## Crate docs (`third_party/dioxus/packages/native-dom/src/lib.rs`)

Core headless native renderer for Dioxus.

## Feature flags
 - `default`: Enables the features listed below.
 - `accessibility`: Enables [`accesskit`](https://docs.rs/accesskit/latest/accesskit/) accessibility support.
 - `hot-reload`: Enables hot-reloading of Dioxus RSX.
 - `menu`: Enables the [`muda`](https://docs.rs/muda/latest/muda/) menubar.
 - `tracing`: Enables tracing support.

## Cargo features (from the manifest)

- `default` (enables 4)
- `svg` (enables 1): DOM features
- `floats` (enables 1)
- `accessibility` (enables 1)
- `tracing` (enables 2)
- `system-fonts` (enables 1)
- `complex-scripts` (enables 1): complex-scripts: enable dictionary-based line-breaking
- `autofocus` (enables 1)
- `text-transform-icu` (enables 1)
- `writing-mode` (enables 1)

## Depends on (same ecosystem)

`blitz-dom`, `blitz-traits`, `dioxus-html`, `dioxus-vdom`

## Public items by file

- `third_party/dioxus/packages/native-dom/src/dioxus_document.rs`: struct DioxusDocument, struct DioxusEventHandler
- `third_party/dioxus/packages/native-dom/src/events.rs`: struct NativeConverter, struct NodeHandle, struct NativeFormData, struct NativePointerData, struct NativeTouchData, struct NativeTouchPointData, struct NativeFocusData, struct NativeScrollData, struct NativeWheelData, fn synthetic_click_event, fn synthetic_form_event
- `third_party/dioxus/packages/native-dom/src/mutation_writer.rs`: struct DioxusState
- `third_party/dioxus/packages/native-dom/src/write_once_attr.rs`: struct SubDocumentAttr, struct CustomWidgetAttr

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
