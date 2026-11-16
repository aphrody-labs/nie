---
name: dioxus-cli-config
description: "Use when working with the `dioxus-cli-config` crate of dioxus: Dioxus-specific settings that dx passes to Dioxus apps, on top of dx-env."
---

# dioxus-cli-config

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/cli-config/` (manifest `third_party/dioxus/packages/cli-config/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/cli-config
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Dioxus-specific settings that dx passes to Dioxus apps, on top of dx-env

## README (`third_party/dioxus/packages/cli-config/README.md`)

# dioxus-cli-config

Dioxus-specific settings that `dx` passes to Dioxus apps at runtime, such as the window title and whether the window floats on top during development.

The generic environment variable contract between `dx` and the app it launches (devserver address, server address, base path, build id, ...) lives in [`dx-env`](https://crates.io/crates/dx-env), which this crate re-exports.

## Depends on (same ecosystem)

`dx-env`

## Public items by file

- `third_party/dioxus/packages/cli-config/src/lib.rs`: fn app_title, fn always_on_top

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
