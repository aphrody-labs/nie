---
name: bevy-platform
description: "Use when working with the `bevy_platform` crate of bevy: Provides common platform agnostic APIs, as well as platform-specific features for Bevy Engine."
---

# bevy_platform

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_platform/` (manifest `third_party/bevy/crates/bevy_platform/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_platform
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides common platform agnostic APIs, as well as platform-specific features for Bevy Engine

## README (`third_party/bevy/crates/bevy_platform/README.md`)

# Bevy Platform Support


Rust is a fantastic multi-platform language with extensive support for modern targets through its [standard library](https://doc.rust-lang.org/stable/std/).
However, some items within the standard library have alternatives that are better suited for [Bevy](https://crates.io/crates/bevy) and game engines in general.
Additionally, to support embedded and other esoteric platforms, it's often necessary to shed reliance on `std`, making your crate [`no_std`](https://docs.rust-embedded.org/book/intro/no-std.html).

These needs are handled by this crate, `bevy_platform`.
The goal of this crate is to provide alternatives and extensions to the Rust standard library which minimize friction when developing with and for Bevy across multiple platforms.

## Getting Started

Like any dependency from [crates.io](https://crates.io/), use `cargo` to add it to your `Cargo.toml` file:

```sh
cargo add bevy_platform
```

Now, instead of importing from `std` you can use `bevy_platform` for items it has alternative for.
See the documentation for what items are available, and explanations for _why_ you may want to use them.

## `no_std` Support

By default, `bevy_platform` will activate the `std` feature, requiring access to the `std` crate for whichever platforms you're targeting.
To use this crate on `no_std` platforms, disable default features:

```toml
bevy_platform = { version = "x.y.z", default-features = false }
```

## Features

### `std` (_default_)

Enables usage of the standard library. Note that where this crate has alternatives to the standard library that it considers _better_ than what's provided, it will provide the alternative even when `std` is enabled.
This is explicitly incompatible with `no_std` targets.

### `alloc` (_default_)

Enables usage of the [`alloc`](https://doc.rust-lang.org/stable/alloc/) crate. Note that this feature is automatically enabled when enabling `std`.
This is compatible with most `no_std` targets, but not all.

### `critical-section`

Switches to using [`critical-section`](https://docs.rs/critical-section/latest/critical_section/) as a backend for synchronization.
You may need to enable this feature on platforms with little to no support for atomic operations.

## Cargo features (from the manifest)

- `default` (enables 1)
- `serialize` (enables 2): Adds serialization support through `serde`.
- `rayon` (enables 2): Adds integration with Rayon.
- `futures-lite` (enables 3): Provides an implementation of `block_on` from `futures-lite`.
- `async-io` (enables 2): Provides an implementation of `block_on` from `async-io`.
- `std` (enables 11): Allows access to the `std` crate. Enabling this feature will prevent compilation on `no_std` targets, but provides access to certain additional features on supported platforms.
- `alloc` (enables 3): Allows access to the `alloc` crate.
- `critical-section` (enables 2): `critical-section` provides the building blocks for synchronization primitives on all platforms, including `no_std`.
- `web` (enables 4): Enables use of browser APIs if the build is a `wasm32` target.
- `bytemuck` (enables 1): Enable APIs that require `bytemuck`

## Public items by file

- `third_party/bevy/crates/bevy_platform/src/cell/sync_cell.rs`: struct SyncCell
- `third_party/bevy/crates/bevy_platform/src/cell/sync_unsafe_cell.rs`: struct SyncUnsafeCell
- `third_party/bevy/crates/bevy_platform/src/cfg.rs`: macro switch!, macro disabled!, macro enabled!, macro define_alias!
- `third_party/bevy/crates/bevy_platform/src/collections/aligned_vec.rs`: struct AlignedVec
- `third_party/bevy/crates/bevy_platform/src/collections/hash_map.rs`: type Entry, struct HashMap
- `third_party/bevy/crates/bevy_platform/src/collections/hash_set.rs`: type Entry, struct HashSet
- `third_party/bevy/crates/bevy_platform/src/collections/mod.rs`: mod aligned_vec, mod hash_map, mod hash_set, mod hash_table
- `third_party/bevy/crates/bevy_platform/src/dirs/linux.rs`: fn preferences_dir
- `third_party/bevy/crates/bevy_platform/src/dirs/macos.rs`: fn preferences_dir
- `third_party/bevy/crates/bevy_platform/src/dirs/mod.rs`: fn preferences_dir
- `third_party/bevy/crates/bevy_platform/src/dirs/windows.rs`: fn preferences_dir
- `third_party/bevy/crates/bevy_platform/src/hash.rs`: struct FixedHasher, fn fixed_hash_one, struct Hashed, struct PassHash, struct PassHasher, struct NoOpHash, struct NoOpHasher
- `third_party/bevy/crates/bevy_platform/src/lib.rs`: mod cell, mod cfg, mod future, mod hash, mod sync, mod thread, mod time, mod prelude, mod exports
- `third_party/bevy/crates/bevy_platform/src/sync/mod.rs`: mod atomic
- `third_party/bevy/crates/bevy_platform/src/time/fallback.rs`: struct Instant

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
