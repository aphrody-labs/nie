---
name: subsecond-types
description: "Use when working with the `subsecond-types` crate of dioxus: Types crate for the Subsecond hotpatch engine."
---

# subsecond-types

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/subsecond/subsecond-types/` (manifest `third_party/dioxus/packages/subsecond/subsecond-types/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/subsecond/subsecond-types
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Types crate for the Subsecond hotpatch engine.

## Public items by file

- `third_party/dioxus/packages/subsecond/subsecond-types/src/lib.rs`: struct JumpTable, type AddressMap, type BuildAddressHasher, struct AddressHasher

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
