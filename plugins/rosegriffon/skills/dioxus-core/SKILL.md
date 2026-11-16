---
name: dioxus-core
description: "Use when working with the `dioxus-core` crate of dioxus: Core building blocks for Dioxus user interfaces."
---

# dioxus-core

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/core/` (manifest `third_party/dioxus/packages/core/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/core
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Core building blocks for Dioxus user interfaces

## Crate docs (`third_party/dioxus/packages/core/src/lib.rs`)

Core building blocks for Dioxus user interfaces.

The crate published as `dioxus-core` through 0.7 was the VirtualDOM
implementation, which is now published as `dioxus-vdom`. Starting with 0.8,
`dioxus-core` is this facade over the Dioxus Core layer.

By default the prelude includes macros, signals, hooks, stores, HTML, and
document APIs. The `ssr` and `wasm-split` features are opt-in. Disable default
features to use only the virtual DOM and explicitly select the APIs needed.

```
# #[cfg(all(feature = "macro", feature = "signals", feature = "hooks", feature = "stores", feature = "html"))]
# fn example() {
use dioxus_core::prelude::*;

#[derive(Clone, Store)]
struct Counter {
    value: i32,
}

#[component]
fn App() -> Element {
    let count = use_signal(|| 0);
    rsx! { div { "{count}" } }
}
# }
```

## Depends on (same ecosystem)

`dioxus-config-macros`, `dioxus-document`, `dioxus-hooks`, `dioxus-html`, `dioxus-signals`, `dioxus-ssr`, `dioxus-stores`, `dioxus-vdom`, `dioxus-vdom-macro`, `wasm-splitter`

## Public items by file

- `third_party/dioxus/packages/core/src/lib.rs`: mod prelude

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
