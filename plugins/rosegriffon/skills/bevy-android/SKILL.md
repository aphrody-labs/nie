---
name: bevy-android
description: "Use when working with the `bevy_android` crate of bevy: Provides android functionality for Bevy Engine."
---

# bevy_android

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_android/` (manifest `third_party/bevy/crates/bevy_android/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_android
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides android functionality for Bevy Engine.

## Crate docs (`third_party/bevy/crates/bevy_android/src/lib.rs`)

Provides Android functionality for Bevy Engine.

## Cargo features (from the manifest)

- `default`

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
