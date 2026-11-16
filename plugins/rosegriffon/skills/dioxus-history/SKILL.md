---
name: dioxus-history
description: "Use when working with the `dioxus-history` crate of dioxus: History provider for dioxus."
---

# dioxus-history

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/history/` (manifest `third_party/dioxus/packages/history/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/history
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

History provider for dioxus

## Depends on (same ecosystem)

`dioxus-vdom`

## Public items by file

- `third_party/dioxus/packages/history/src/lib.rs`: fn history, fn provide_history_context, trait History
- `third_party/dioxus/packages/history/src/memory.rs`: struct MemoryHistory

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
