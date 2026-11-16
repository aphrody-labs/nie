---
name: manganis-macro
description: "Use when working with the `manganis-macro` crate of dioxus: Ergonomic, automatic, cross crate asset collection and optimization."
---

# manganis-macro

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/manganis/manganis-macro/` (manifest `third_party/dioxus/packages/manganis/manganis-macro/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/manganis/manganis-macro
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Ergonomic, automatic, cross crate asset collection and optimization

## README (`third_party/dioxus/packages/manganis/manganis-macro/README.md`)

# Manganis Macro

This crate contains the macro used to interact with the Manganis asset system.

## Cargo features (from the manifest)

- `default`

## Depends on (same ecosystem)

`manganis-core`

## Public items by file

- `third_party/dioxus/packages/manganis/manganis-macro/src/asset.rs`: struct AssetParser
- `third_party/dioxus/packages/manganis/manganis-macro/src/ffi.rs`: enum ForeignAbi, struct ForeignTypeDecl, enum ForeignType, struct ForeignArg, struct ForeignFunctionDecl, struct FfiBridgeParser, struct FfiAttribute
- `third_party/dioxus/packages/manganis/manganis-macro/src/lib.rs`: macro asset, macro option_asset, attribute css_module, attribute ffi
- `third_party/dioxus/packages/manganis/manganis-macro/src/linker.rs`: fn generate_link_section_inner, fn generate_link_section

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
