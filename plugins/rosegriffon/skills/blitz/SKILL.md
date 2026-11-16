---
name: blitz
description: "Use when working with the `blitz` crate of blitz: High-level APIs for rendering HTML with Blitz."
---

# blitz

- Version: 0.3.0-beta.4 (workspace member of `third_party/blitz`, vendored commit `a84a3c51b562`)
- Source: `third_party/blitz/packages/blitz/` (manifest `third_party/blitz/packages/blitz/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/blitz/tree/main/packages/blitz
- License: [object Object]
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Orientation

Modular HTML and CSS engine: `blitz-dom` (DOM and Stylo/Taffy layout), `blitz-html` (parsing), `blitz-paint` and the AnyRender backends (Vello, Vello CPU, Vello hybrid, Skia, tiny-skia), `blitz-shell` (winit windows), `blitz-net` (fetching), `blitz-traits` (shared types), and the `blitz` wrapper. See `third_party/blitz/README.md` (status, goals, architecture), `docs/architecture.mmd`, and `examples/` (browser, wgpu_texture, todomvc).

Monorepo rules: Blitz pins Taffy by git rev; the root `Cargo.toml` redirects that source with `[patch."https://github.com/DioxusLabs/taffy"]` to `third_party/taffy`. The fork adds a `rg-wasm` profile (`tiny` plus fat LTO); upstream `production`, `small` and `tiny` profiles are kept.

## Purpose

High-level APIs for rendering HTML with Blitz

## Crate docs (`third_party/blitz/packages/blitz/src/lib.rs`)

Blitz is a modular, embeddable web engine with a native Rust API.

It powers the [`dioxus-native`] UI framework.

This crate exists to collect the most important functionality for users together in one place.
It does not bring any unique functionality, but rather, it re-exports the relevant crates as modules.
The exported crate corresponding to each module is also available in a stand-alone manner, i.e. [`blitz-dom`] as [`blitz::dom`](crate::dom).

[`dioxus-native`]: https://docs.rs/dioxus-native
[`blitz-dom`]: https://docs.rs/blitz-dom

## Cargo features (from the manifest)

- `default` (enables 4)
- `net` (enables 3)
- `accessibility` (enables 1)
- `text-transform-icu` (enables 1)
- `tracing` (enables 4)
- `scrollbars` (enables 1)
- `writing-mode` (enables 1)

## Depends on (same ecosystem)

`blitz-dom`, `blitz-html`, `blitz-net`, `blitz-paint`, `blitz-shell`, `blitz-traits`

## Public items by file

- `third_party/blitz/packages/blitz/src/lib.rs`: fn launch_url, fn launch_static_html, fn launch_static_html_cfg

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
