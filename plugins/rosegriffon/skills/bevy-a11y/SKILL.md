---
name: bevy-a11y
description: "Use when working with the `bevy_a11y` crate of bevy: Provides accessibility support for Bevy Engine."
---

# bevy_a11y

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_a11y/` (manifest `third_party/bevy/crates/bevy_a11y/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_a11y
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides accessibility support for Bevy Engine

## README (`third_party/bevy/crates/bevy_a11y/README.md`)

# Bevy A11Y (Accessibility)

## Cargo features (from the manifest)

- `default` (enables 3)
- `bevy_reflect` (enables 3): Adds runtime reflection support using `bevy_reflect`.
- `serialize` (enables 3): Adds serialization support through `serde`.
- `std` (enables 3): Allows access to the `std` crate. Enabling this feature will prevent compilation on `no_std` targets, but provides access to certain additional features on supported platforms.
- `critical-section` (enables 3): `critical-section` provides the building blocks for synchronization primitives on all platforms, including `no_std`.

## Depends on (same ecosystem)

`bevy_app`, `bevy_derive`, `bevy_ecs`, `bevy_reflect`

## Public items by file

- `third_party/bevy/crates/bevy_a11y/src/lib.rs`: struct ActionRequest, struct AccessibilityRequested, struct ManageAccessibilityUpdates, struct AccessibilityNode, enum AccessibilitySystems, struct AccessibilityPlugin

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
