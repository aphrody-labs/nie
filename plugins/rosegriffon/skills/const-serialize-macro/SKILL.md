---
name: const-serialize-macro
description: "Use when working with the `const-serialize-macro` crate of dioxus: A macro to derive const serialize."
---

# const-serialize-macro

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/const-serialize-macro/` (manifest `third_party/dioxus/packages/const-serialize-macro/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/const-serialize-macro
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

A macro to derive const serialize

## Public items by file

- `third_party/dioxus/packages/const-serialize-macro/src/lib.rs`: derive SerializeConst, fn derive_parse

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
