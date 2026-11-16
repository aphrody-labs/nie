---
name: lazy-js-bundle
description: "Use when working with the `lazy-js-bundle` crate of dioxus: A codegen library to bundle TypeScript into JavaScript without requiring a bundler to be installed."
---

# lazy-js-bundle

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/lazy-js-bundle/` (manifest `third_party/dioxus/packages/lazy-js-bundle/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/lazy-js-bundle
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

A codegen library to bundle TypeScript into JavaScript without requiring a bundler to be installed

## Public items by file

- `third_party/dioxus/packages/lazy-js-bundle/src/lib.rs`: struct LazyTypeScriptBindings, enum MinifyLevel

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
