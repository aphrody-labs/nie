---
name: bevy-gilrs
description: "Use when working with the `bevy_gilrs` crate of bevy: Gamepad system made using Gilrs for Bevy Engine."
---

# bevy_gilrs

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_gilrs/` (manifest `third_party/bevy/crates/bevy_gilrs/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_gilrs
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Gamepad system made using Gilrs for Bevy Engine

## README (`third_party/bevy/crates/bevy_gilrs/README.md`)

# Bevy GilRs

## Depends on (same ecosystem)

`bevy_app`, `bevy_ecs`, `bevy_input`, `bevy_platform`, `bevy_time`

## Public items by file

- `third_party/bevy/crates/bevy_gilrs/src/converter.rs`: fn convert_button, fn convert_axis
- `third_party/bevy/crates/bevy_gilrs/src/gilrs_system.rs`: fn gilrs_event_startup_system, fn gilrs_event_system
- `third_party/bevy/crates/bevy_gilrs/src/lib.rs`: struct GilrsPlugin, struct RumbleSystems

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
