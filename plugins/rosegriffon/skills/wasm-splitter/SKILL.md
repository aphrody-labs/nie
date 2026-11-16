---
name: wasm-splitter
description: "Use when working with the `wasm-splitter` crate of dioxus: A tool for splitting up large WASM binaries into smaller chunks."
---

# wasm-splitter

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/wasm-split/wasm-split/` (manifest `third_party/dioxus/packages/wasm-split/wasm-split/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/wasm-split/wasm-split
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

A tool for splitting up large WASM binaries into smaller chunks

## Depends on (same ecosystem)

`wasm-split-macro`

## Public items by file

- `third_party/dioxus/packages/wasm-split/wasm-split/src/lib.rs`: type Result, enum SplitLoaderError, struct LazyLoader, struct LazySplitLoader

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
