---
name: bevy-fuzzy
description: "Use when working with the `bevy_fuzzy` crate of bevy: A small fuzzy matcher for Bevy tools such as inspectors, pickers and command palettes."
---

# bevy_fuzzy

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_fuzzy/` (manifest `third_party/bevy/crates/bevy_fuzzy/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_fuzzy
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

A small fuzzy matcher for Bevy tools such as inspectors, pickers and command palettes

## README (`third_party/bevy/crates/bevy_fuzzy/README.md`)

# Bevy Fuzzy


A small fuzzy matcher for tools such as inspectors, pickers and command palettes.

## Public items by file

- `third_party/bevy/crates/bevy_fuzzy/src/lib.rs`: struct FuzzyPattern, struct FuzzyMatch, struct RankedMatch, fn segments, struct Segments

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
