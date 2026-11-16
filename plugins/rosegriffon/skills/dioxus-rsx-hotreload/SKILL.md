---
name: dioxus-rsx-hotreload
description: "Use when working with the `dioxus-rsx-hotreload` crate of dioxus: CLI Configuration for dioxus-cli."
---

# dioxus-rsx-hotreload

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/rsx-hotreload/` (manifest `third_party/dioxus/packages/rsx-hotreload/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/rsx-hotreload
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

CLI Configuration for dioxus-cli

## Depends on (same ecosystem)

`dioxus-rsx`, `dioxus-vdom`, `dioxus-vdom-template`

## Public items by file

- `third_party/dioxus/packages/rsx-hotreload/src/collect.rs`: struct ChangedRsx, enum ReloadableRustCode, fn diff_rsx, fn collect_from_file, fn CoolChild
- `third_party/dioxus/packages/rsx-hotreload/src/diff.rs`: struct HotReloadResult
- `third_party/dioxus/packages/rsx-hotreload/src/hot_reload_context.rs`: trait HotReloadingContext, struct Empty

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
