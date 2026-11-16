---
name: generational-box
description: "Use when working with the `generational-box` crate of dioxus: A box backed by a generational runtime."
---

# generational-box

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/generational-box/` (manifest `third_party/dioxus/packages/generational-box/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/generational-box
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

A box backed by a generational runtime

## README (`third_party/dioxus/packages/generational-box/README.md`)

# Generational Box

Generational Box is a runtime for Rust that allows any static type to implement `Copy`. It can be combined with a global runtime to create an ergonomic state solution like `dioxus-signals`. This crate doesn't have any `unsafe` code.

Three main types manage state in Generational Box:

- `Store`: Handles recycling generational boxes that have been dropped. Your application should have one store or one store per thread.
- `Owner`: Handles dropping generational boxes. The owner acts like a runtime lifetime guard. Any states that you create with an owner will be dropped when that owner is dropped.
- `GenerationalBox`: The core Copy state type. The generational box will be dropped when the owner is dropped.

Example:

```rust
use generational_box::{UnsyncStorage, AnyStorage};

// Create an owner for some state for a scope
let owner = UnsyncStorage::owner();

// Create some non-copy data, move it into a owner, and work with copy data
let data: String = "hello world".to_string();
let key = owner.insert(data);

// The generational box can be read from and written to like a RefCell
let value = key.read();
assert_eq!(*value, "hello world");
```

## How it works

Internally, `generational-box` creates an arena of generational `RefCell`s that are recycled when the owner is dropped. You can think of the cells as something like `&'static RefCell>` with a generational check to make recycling a cell easier to debug. Then `GenerationalBox`es are `Copy` because the `&'static` pointer is `Copy`.

## Cargo features (from the manifest)

- `debug_borrows`
- `debug_ownership`

## Public items by file

- `third_party/dioxus/packages/generational-box/src/error.rs`: type BorrowResult, type BorrowMutResult, enum BorrowError, enum BorrowMutError, struct ValueDroppedError, struct AlreadyBorrowedMutError, struct AlreadyBorrowedError
- `third_party/dioxus/packages/generational-box/src/lib.rs`: struct GenerationalBoxId, struct GenerationalBox, trait Storage, trait AnyStorage, struct GenerationalPointer, struct Owner
- `third_party/dioxus/packages/generational-box/src/references.rs`: struct GenerationalRef, struct GenerationalRefMut
- `third_party/dioxus/packages/generational-box/src/sync.rs`: struct SyncStorage
- `third_party/dioxus/packages/generational-box/src/unsync.rs`: struct UnsyncStorage

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
