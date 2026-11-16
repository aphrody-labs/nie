---
name: dioxus-config-macros
description: "Use when working with the `dioxus-config-macros` crate of dioxus: Macros used internally by codegen."
---

# dioxus-config-macros

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/config-macros/` (manifest `third_party/dioxus/packages/config-macros/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/config-macros
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Macros used internally by codegen

## README (`third_party/dioxus/packages/config-macros/README.md`)

# Dioxus Config Macros

These macros are used internally by codegen and are not intended for general use.

Dioxus will export its feature flags into this crate, allowing downstream codegen to use them under the "dioxus" namespace.

## License

This project is licensed under the [MIT license].

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Dioxus by you shall be licensed as MIT without any additional
terms or conditions.

## Public items by file

- `third_party/dioxus/packages/config-macros/src/lib.rs`: macro maybe_wasm_split!

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
