---
name: debug-timer
description: "Use when working with the `debug_timer` crate of blitz: Utilities for simple timings."
---

# debug_timer

- Version: 0.1.3 (workspace member of `third_party/blitz`, vendored commit `a84a3c51b562`)
- Source: `third_party/blitz/packages/debug_timer/` (manifest `third_party/blitz/packages/debug_timer/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/blitz/tree/main/packages/debug_timer
- License: [object Object]
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Utilities for simple timings

## Public items by file

- `third_party/blitz/packages/debug_timer/src/lib.rs`: macro debug_timer!, macro debug_timer_type!

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
