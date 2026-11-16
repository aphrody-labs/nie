---
name: dioxus-stores
description: "Use when working with the `dioxus-stores` crate of dioxus: Server function macros for Dioxus."
---

# dioxus-stores

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/stores/` (manifest `third_party/dioxus/packages/stores/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/stores
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Server function macros for Dioxus

## README (`third_party/dioxus/packages/stores/README.md`)

# Dioxus Stores

Stores are an extension to the Dioxus signals system for reactive nested data structures. Each store will lazily create signals for each field/member of the data structure as needed.

By default stores act a lot like [`dioxus_signals::Signal`]s, but they provide more granular
subscriptions without requiring nested signals. You should derive [`Store`](dioxus_stores_macro::Store) on your data
structures to generate selectors that let you scope the store to a specific part of your data structure.

```rust, no_run
use dioxus::prelude::*;
use dioxus_stores::*;

fn main() {
    dioxus::launch(app);
}

// Deriving the store trait provides methods to scope the store to specific parts of your data structure.
// The `Store` macro generates a `count` and `children` method for `Store`
#[derive(Store, Default)]
struct CounterTree {
    count: i32,
    children: Vec,
}

fn app() -> Element {
    let value = use_store(Default::default);

    rsx! {
        Tree {
            value
        }
    }
}

#[component]
fn Tree(value: Store) -> Element {
    // Calling the generated `count` method returns a new store that can only
    // read and write the count field
    let mut count = value.count();
    let mut children = value.children();

    rsx! {
        button {
            // Incrementing the count will only rerun parts of the app that have read the count field
            onclick: move |_| count += 1,
            "Increment"
        }
        button {
            // Stores are aware of data structures like `Vec` and `Hashmap`. When we push an item to the vec
            // it will only rerun the parts of the app that depend on the length of the vec
            onclick: move |_| children.push(Default::default()),
            "Push child"
        }
        ul {
            // Iterating over the children gives us stores scoped to each child.
            for value in children.iter() {
                li {
                    Tree { value }
                }
            }
        }
    }
}
```

## Cargo features (from the manifest)

- `default` (enables 1)
- `macro` (enables 1)
- `large-path`

## Depends on (same ecosystem)

`dioxus-signals`, `dioxus-stores-macro`, `dioxus-vdom`, `generational-box`

## Public items by file

- `third_party/dioxus/packages/stores/src/impls/btreemap.rs`: struct GetWrite
- `third_party/dioxus/packages/stores/src/impls/hashmap.rs`: struct GetWrite
- `third_party/dioxus/packages/stores/src/impls/index.rs`: trait IndexSelector, struct IndexWrite
- `third_party/dioxus/packages/stores/src/impls/mod.rs`: mod btreemap, mod hashmap, mod index, mod slice
- `third_party/dioxus/packages/stores/src/impls/slice.rs`: struct VecGetWrite
- `third_party/dioxus/packages/stores/src/lib.rs`: mod scope, mod macro_helpers
- `third_party/dioxus/packages/stores/src/scope.rs`: struct SelectorScope
- `third_party/dioxus/packages/stores/src/store.rs`: type ReadStore, type WriteStore, type SyncStore, struct Store, struct SuperIntoReadSignalMarker, struct SuperIntoWriteSignalMarker, fn use_store, fn use_store_sync, type GlobalStore

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
