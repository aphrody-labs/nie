---
name: bevy-log
description: "Use when working with the `bevy_log` crate of bevy: Provides logging for Bevy Engine."
---

# bevy_log

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_log/` (manifest `third_party/bevy/crates/bevy_log/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_log
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides logging for Bevy Engine

## README (`third_party/bevy/crates/bevy_log/README.md`)

# Bevy Log

## Cargo features (from the manifest)

- `trace` (enables 1)
- `trace_tracy_memory` (enables 1)

## Depends on (same ecosystem)

`bevy_app`, `bevy_ecs`, `bevy_platform`, `bevy_utils`

## Public items by file

- `third_party/bevy/crates/bevy_log/src/lib.rs`: mod prelude, struct LogPlugin, type BoxedLayer, type BoxedFmtLayer
- `third_party/bevy/crates/bevy_log/src/once.rs`: macro trace_once!, macro debug_once!, macro info_once!, macro warn_once!, macro error_once!

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
