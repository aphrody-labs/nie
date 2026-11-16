---
name: dioxus-devtools
description: "Use when working with the `dioxus-devtools` crate of dioxus: Hot reloading utilities for Dioxus."
---

# dioxus-devtools

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/devtools/` (manifest `third_party/dioxus/packages/devtools/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/devtools
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Hot reloading utilities for Dioxus

## Depends on (same ecosystem)

`dioxus-devtools-types`, `dioxus-signals`, `dioxus-vdom`, `dx-env`, `subsecond`

## Public items by file

- `third_party/dioxus/packages/devtools/src/lib.rs`: fn apply_changes, fn try_apply_changes, fn connect, fn connect_subsecond, fn connect_at, fn serve_subsecond, fn serve_subsecond_with_args

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
