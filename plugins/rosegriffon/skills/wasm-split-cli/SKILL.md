---
name: wasm-split-cli
description: "Use when working with the `wasm-split-cli` crate of dioxus: CLI-support for wasm-split - a tool for splitting up large WASM binaries into smaller chunks."
---

# wasm-split-cli

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/wasm-split/wasm-split-cli/` (manifest `third_party/dioxus/packages/wasm-split/wasm-split-cli/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/wasm-split/wasm-split-cli
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

CLI-support for wasm-split - a tool for splitting up large WASM binaries into smaller chunks

## Public items by file

- `third_party/dioxus/packages/wasm-split/wasm-split-cli/src/lib.rs`: struct Splitter, struct OutputModules, struct SplitModule, struct SplitPoint, enum Node

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
