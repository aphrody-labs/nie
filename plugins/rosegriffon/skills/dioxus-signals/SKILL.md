---
name: dioxus-signals
description: "Use when working with the `dioxus-signals` crate of dioxus: Reactivie signals for Dioxus: Build fullstack web, desktop, and mobile apps with a single codebase."
---

# dioxus-signals

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/signals/` (manifest `third_party/dioxus/packages/signals/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/signals
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Reactivie signals for Dioxus: Build fullstack web, desktop, and mobile apps with a single codebase.

## README (`third_party/dioxus/packages/signals/README.md`)

# Dioxus Signals

Dioxus Signals is an ergonomic Copy runtime for data with local subscriptions.

## Copy Data

All signals implement Copy, even if the inner value does not implement copy. This makes it easy to move any data into futures or children.

```rust
use dioxus::prelude::*;
use dioxus_signals::*;

#[component]
fn App() -> Element {
    let signal = use_signal(|| "hello world".to_string());

    spawn(async move {
        // signal is Copy even though String is not copy
        print!("{signal}");
    });

    rsx! {
        "{signal}"
    }
}
```

## Local Subscriptions

Signals will only subscribe to components when you read from the signal in that component. It will never subscribe to a component when reading data in a future or event handler.

```rust
use dioxus::prelude::*;
use dioxus_signals::*;

#[component]
fn App() -> Element {
    // Because signal is never read in this component, this component will not rerun when the signal changes
    let mut signal = use_signal(|| 0);

    rsx! {
        button {
            onclick: move |_| {
                signal += 1;
            },
            "Increase"
        }
        for id in 0..10 {
            Child {
                signal,
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct ChildProps {
    signal: Signal,
}

fn Child(props: ChildProps) -> Element {
    // This component does read from the signal, so when the signal changes it will rerun
    rsx! {
        "{props.signal}"
    }
}
```

Because subscriptions happen when you read from (not create) the data, you can provide signals through the normal context API:


## Cargo features (from the manifest)

- `default`
- `serialize` (enables 1)

## Depends on (same ecosystem)

`dioxus-vdom`, `generational-box`

## Public items by file

- `third_party/dioxus/packages/signals/src/boxed.rs`: type ReadOnlySignal, struct ReadSignal, struct WriteSignal, trait BoxedSignalStorage, trait CreateBoxedSignalStorage
- `third_party/dioxus/packages/signals/src/copy_value.rs`: struct CopyValue
- `third_party/dioxus/packages/signals/src/global/memo.rs`: type GlobalMemo
- `third_party/dioxus/packages/signals/src/global/mod.rs`: trait InitializeFromFunction, struct Global, struct GlobalLazyContext, enum GlobalKey, fn get_global_context
- `third_party/dioxus/packages/signals/src/global/signal.rs`: type GlobalSignal
- `third_party/dioxus/packages/signals/src/impls.rs`: macro default_impl!, macro read_impls!, macro fmt_impls!, macro eq_impls!, macro write_impls!
- `third_party/dioxus/packages/signals/src/lib.rs`: mod warnings
- `third_party/dioxus/packages/signals/src/map.rs`: struct MappedSignal
- `third_party/dioxus/packages/signals/src/map_mut.rs`: struct MappedMutSignal
- `third_party/dioxus/packages/signals/src/memo.rs`: struct Memo
- `third_party/dioxus/packages/signals/src/props.rs`: struct ReadFromMarker
- `third_party/dioxus/packages/signals/src/read.rs`: type ReadableRef, trait Readable, trait ReadableExt, trait ReadableBoxExt, trait ReadableVecExt, struct ReadableValueIterator, trait ReadableOptionExt, trait ReadableResultExt, trait ReadableStringExt, trait ReadableStrExt, trait ReadableHashMapExt, trait ReadableHashSetExt
- `third_party/dioxus/packages/signals/src/set_compare.rs`: struct SetCompare
- `third_party/dioxus/packages/signals/src/signal.rs`: struct Signal, type SyncSignal, struct SignalData, struct SignalSubscriberDrop
- `third_party/dioxus/packages/signals/src/warnings.rs`: fn copy_value_hoisted
- `third_party/dioxus/packages/signals/src/write.rs`: type WritableRef, trait Writable, struct WriteLock, trait WritableExt, trait WritableBoxedExt, trait WritableOptionExt, trait WritableResultExt, trait WritableVecExt, struct WritableValueIterator, trait WritableStringExt, trait WritableHashMapExt, trait WritableHashSetExt

## Examples in the source tree

- `third_party/dioxus/packages/signals/examples/context.rs`
- `third_party/dioxus/packages/signals/examples/dependencies.rs`
- `third_party/dioxus/packages/signals/examples/map_signal.rs`
- `third_party/dioxus/packages/signals/examples/read_only_degrade.rs`
- `third_party/dioxus/packages/signals/examples/selector.rs`
- `third_party/dioxus/packages/signals/examples/send.rs`
- `third_party/dioxus/packages/signals/examples/send_store.rs`
- `third_party/dioxus/packages/signals/examples/split_subscriptions.rs`

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
