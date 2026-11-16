---
name: dx-wire-format
description: "Use when working with the `dx-wire-format` crate of dioxus: Wire format for the Dioxus CLI."
---

# dx-wire-format

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/dx-wire-format/` (manifest `third_party/dioxus/packages/dx-wire-format/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/dx-wire-format
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Wire format for the Dioxus CLI

## Depends on (same ecosystem)

`manganis-core`, `subsecond-types`

## Public items by file

- `third_party/dioxus/packages/dx-wire-format/src/lib.rs`: enum StructuredOutput, struct StructuredBuildArtifacts, enum BuildStage

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
