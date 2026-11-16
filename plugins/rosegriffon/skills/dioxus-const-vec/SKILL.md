---
name: dioxus-const-vec
description: "Use when working with the `dioxus-const-vec` crate of dioxus: A fixed-capacity Vec usable in const contexts."
---

# dioxus-const-vec

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/dioxus-const-vec/` (manifest `third_party/dioxus/packages/dioxus-const-vec/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/dioxus-const-vec
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

A fixed-capacity Vec usable in const contexts

## Public items by file

- `third_party/dioxus/packages/dioxus-const-vec/src/lib.rs`: struct ConstReadBuffer, struct ConstVec

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
