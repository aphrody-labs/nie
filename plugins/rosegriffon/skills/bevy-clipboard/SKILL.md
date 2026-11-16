---
name: bevy-clipboard
description: "Use when working with the `bevy_clipboard` crate of bevy: Provides clipboard support for Bevy Engine."
---

# bevy_clipboard

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_clipboard/` (manifest `third_party/bevy/crates/bevy_clipboard/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_clipboard
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides clipboard support for Bevy Engine

## README (`third_party/bevy/crates/bevy_clipboard/README.md`)

# Bevy Clipboard

## Cargo features (from the manifest)

- `default`
- `system_clipboard` (enables 1)
- `image` (enables 5)
- `wayland` (enables 1)

## Depends on (same ecosystem)

`bevy_app`, `bevy_ecs`, `bevy_log`, `bevy_platform`

## Public items by file

- `third_party/bevy/crates/bevy_clipboard/src/lib.rs`: mod prelude, struct ClipboardPlugin, enum ClipboardRead, struct Clipboard, enum ClipboardError

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
