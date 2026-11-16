---
name: dioxus-devtools-types
description: "Use when working with the `dioxus-devtools-types` crate of dioxus: CLI Configuration for dioxus-cli."
---

# dioxus-devtools-types

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/devtools-types/` (manifest `third_party/dioxus/packages/devtools-types/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/devtools-types
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

CLI Configuration for dioxus-cli

## Depends on (same ecosystem)

`dioxus-vdom`, `subsecond-types`

## Public items by file

- `third_party/dioxus/packages/devtools-types/src/lib.rs`: enum DevserverMsg, enum ClientMsg, struct HotReloadMsg

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
