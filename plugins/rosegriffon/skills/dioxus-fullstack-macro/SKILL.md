---
name: dioxus-fullstack-macro
description: "Use when working with the `dioxus-fullstack-macro` crate of dioxus: Server function macros for Dioxus."
---

# dioxus-fullstack-macro

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/fullstack-macro/` (manifest `third_party/dioxus/packages/fullstack-macro/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/fullstack-macro
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Server function macros for Dioxus

## Public items by file

- `third_party/dioxus/packages/fullstack-macro/src/lib.rs`: attribute server, attribute get, attribute post, attribute put, attribute delete, attribute patch

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
