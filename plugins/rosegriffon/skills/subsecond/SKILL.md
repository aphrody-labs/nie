---
name: subsecond
description: "Use when working with the `subsecond` crate of dioxus: A runtime hotpatching engine for Rust hot-reloading."
---

# subsecond

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/subsecond/subsecond/` (manifest `third_party/dioxus/packages/subsecond/subsecond/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/subsecond/subsecond
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Orientation

Hot patching for Rust (`third_party/dioxus/packages/subsecond`): `subsecond` is the runtime that applies patches, `subsecond-types` the shared wire types, and the Dioxus CLI (`dx`) produces the patches. Read `packages/subsecond/subsecond/src/lib.rs` for the entry points (`call`, `register_handler`) at the vendored commit before relying on them.

## Purpose

A runtime hotpatching engine for Rust hot-reloading.

## Crate docs (`third_party/dioxus/packages/subsecond/subsecond/src/lib.rs`)

# Subsecond: Hot-patching for Rust

Subsecond is a library that enables hot-patching for Rust applications. This allows you to change
the code of a running application without restarting it. This is useful for game engines, servers,
and other long-running applications where the typical edit-compile-run cycle is too slow.

Subsecond also implements a technique we call "ThinLinking" which makes compiling Rust code
significantly faster in development mode, which can be used outside of hot-patching.

# Usage

Subsecond is designed to be as simple for both application developers and library authors.

Simply call your existing functions with [`call`] and Subsecond will automatically detour
that call to the latest version of the function.

```rust
for x in 0..5 {
    subsecond::call(|| {
        println!("Hello, world! {}", x);
    });
}
```

To actually load patches into your application, a third-party tool that implements the Subsecond
compiler and protocol is required. Subsecond is built and maintained by the Dioxus team, so we
suggest using the dioxus CLI tool to use subsecond.

To install the Dioxus CLI, we recommend using [`cargo binstall`](https://crates.io/crates/cargo-binstall):

```sh
cargo binstall dioxus-cli
```

The Dioxus CLI provides several tools for development. To run your application with Subsecond enabled,
use `dx serve` - this takes the same arguments as `cargo run` but will automatically hot-reload your
application when changes are detected.

As of Dioxus 0.7, "--hotpatch" is required to use hotpatching while Subsecond is still experimental.

```sh
dx serve --hotpatch
```

## How it works

Subsecond works by detouring function calls through a jump table. This jump table contains the latest
version of the program's function pointers, and when a function is called, Subsecond will look up
the function in the jump table and call that instead.

Unlike libraries like [detour](https://crates.io/crates/detour), Subsecond *does not* modify your
process memory. Patching pointers is wildly unsafe and can lead to crashes and undefined behavior.

Instead, an external tool compiles only the parts of your project that changed, links them together
using the addresses of the functions in your running program, and then sends the new jump table to
your application. Subsecond then applies the patch and continues running. Since Subsecond doesn't
modify memory, the program must have a runtime integration to handle the patching.

If the framework you're using doesn't integrate with subsecond, you can rely on the fact that calls
to stale [`call`] instances will emit a safe panic that is automatically caught and retried

## Depends on (same ecosystem)

`subsecond-types`

## Public items by file

- `third_party/dioxus/packages/subsecond/subsecond/src/lib.rs`: fn call, fn register_handler, fn get_jump_table, struct HotFnPanic, struct HotFnPtr, struct HotFn, fn apply_patch, enum PatchError, fn aslr_reference, trait HotFunction

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
