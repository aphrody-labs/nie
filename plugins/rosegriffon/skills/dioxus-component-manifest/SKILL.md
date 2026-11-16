---
name: dioxus-component-manifest
description: "Use when working with the `dioxus-component-manifest` crate of dioxus: Wire format for the dioxus CLI telemetry type."
---

# dioxus-component-manifest

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/component-manifest/` (manifest `third_party/dioxus/packages/component-manifest/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/component-manifest
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Wire format for the dioxus CLI telemetry type

## Public items by file

- `third_party/dioxus/packages/component-manifest/src/lib.rs`: struct Component, enum ComponentDependency, enum CargoDependency, fn component_manifest_schema

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
