---
name: bevy-utils
description: "Use when working with the `bevy_utils` crate of bevy: A collection of utils for Bevy Engine."
---

# bevy_utils

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_utils/` (manifest `third_party/bevy/crates/bevy_utils/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_utils
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

A collection of utils for Bevy Engine

## README (`third_party/bevy/crates/bevy_utils/README.md`)

# Bevy Utilities


A Collection of Utilities for the [Bevy Engine](https://bevy.org/).

## Cargo features (from the manifest)

- `default` (enables 2)
- `parallel` (enables 2): Provides access to the `Parallel` type.
- `buffered_channel` (enables 2)
- `std` (enables 1)
- `debug` (enables 1)
- `serialize` (enables 1)

## Depends on (same ecosystem)

`bevy_platform`

## Public items by file

- `third_party/bevy/crates/bevy_utils/src/atomic_id.rs`: macro define_atomic_id!
- `third_party/bevy/crates/bevy_utils/src/bloom_filter.rs`: struct BloomFilter
- `third_party/bevy/crates/bevy_utils/src/buffered_channel.rs`: struct BufferedChannel, struct BufferedReceiver, struct RecycledVec, struct BufferedSender
- `third_party/bevy/crates/bevy_utils/src/debug_info.rs`: struct DebugName
- `third_party/bevy/crates/bevy_utils/src/default.rs`: fn default
- `third_party/bevy/crates/bevy_utils/src/lib.rs`: mod cfg, mod prelude, mod memory_size, struct OnDrop
- `third_party/bevy/crates/bevy_utils/src/map.rs`: type PreHashMap, trait PreHashMapExt, type TypeIdHashMap, type TypeIdIndexMap, trait TypeIdMapExt, trait TypeIdHashMapExt
- `third_party/bevy/crates/bevy_utils/src/memory_size.rs`: struct MemorySize, enum MemoryUnit
- `third_party/bevy/crates/bevy_utils/src/once.rs`: struct OnceFlag, macro once!
- `third_party/bevy/crates/bevy_utils/src/parallel_queue.rs`: struct Parallel

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
