---
name: bevy-encase-derive
description: "Use when working with the `bevy_encase_derive` crate of bevy: Bevy derive macro for encase."
---

# bevy_encase_derive

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_encase_derive/` (manifest `third_party/bevy/crates/bevy_encase_derive/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_encase_derive
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Bevy derive macro for encase

## README (`third_party/bevy/crates/bevy_encase_derive/README.md`)

# Bevy Encase Derive

## Depends on (same ecosystem)

`bevy_macro_utils`

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
