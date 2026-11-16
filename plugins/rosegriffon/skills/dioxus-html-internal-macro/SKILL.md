---
name: dioxus-html-internal-macro
description: "Use when working with the `dioxus-html-internal-macro` crate of dioxus: HTML function macros for Dioxus."
---

# dioxus-html-internal-macro

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/html-internal-macro/` (manifest `third_party/dioxus/packages/html-internal-macro/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/html-internal-macro
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

HTML function macros for Dioxus

## Public items by file

- `third_party/dioxus/packages/html-internal-macro/src/elements.rs`: macro __dioxus_html_define_elements_with_detected_gated_attributes!
- `third_party/dioxus/packages/html-internal-macro/src/lib.rs`: macro impl_extension_attributes, macro define_elements, macro impl_event_extensions

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
