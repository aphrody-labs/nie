---
name: bevy-dylib
description: "Use when working with the `bevy_dylib` crate of bevy: Force the Bevy Engine to be dynamically linked for faster linking."
---

# bevy_dylib

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_dylib/` (manifest `third_party/bevy/crates/bevy_dylib/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_dylib
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Force the Bevy Engine to be dynamically linked for faster linking

## README (`third_party/bevy/crates/bevy_dylib/README.md`)

# Bevy Dynamic Library

## Depends on (same ecosystem)

`bevy_internal`

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
