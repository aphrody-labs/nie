---
name: dioxus-ssr
description: "Use when working with the `dioxus-ssr` crate of dioxus: Dioxus render-to-string."
---

# dioxus-ssr

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/ssr/` (manifest `third_party/dioxus/packages/ssr/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/ssr
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Dioxus render-to-string

## README (`third_party/dioxus/packages/ssr/README.md`)

Dioxus Server-Side Rendering (SSR)
  
    Render Dioxus to valid html.
  

## Resources

This crate is a part of the broader Dioxus ecosystem. For more resources about Dioxus, check out:

- [Getting Started](https://dioxuslabs.com/learn/0.7/getting_started)
- [Book](https://dioxuslabs.com/learn/0.7/)
- [Examples](https://github.com/DioxusLabs/dioxus/tree/main/examples)

## Overview

Dioxus SSR provides utilities to render Dioxus components to valid HTML. Once rendered, the HTML can be rehydrated client-side or served from your web server of choice.

```rust
# use dioxus::prelude::*;
fn app() -> Element {
  rsx! {
    div {"hello world!"}
  }
}

let mut vdom = VirtualDom::new(app);
vdom.rebuild_in_place();

let text = dioxus_ssr::render(&vdom);
assert_eq!(text, "hello world!")
```

## Basic Usage

The simplest example is to simply render some `rsx!` nodes to HTML. This can be done with the [`render_element`] API.

```rust, no_run
# use dioxus::prelude::*;
let content = dioxus_ssr::render_element(rsx! {
    div {
        for i in 0..5 {
            "Number: {i}"
        }
    }
});
```

## Rendering a VirtualDom

```rust, no_run
# use dioxus::prelude::*;
# fn app() -> Element { todo!() }
let mut vdom = VirtualDom::new(app);
vdom.rebuild_in_place();

let content = dioxus_ssr::render(&vdom);
```

## Usage in pre-rendering

This crate is particularly useful in pre-generating pages server-side and then selectively loading Dioxus client-side to pick up the reactive elements.

This crate supports hydration out of the box. However, both the client and server must generate the _exact_ same VirtualDOMs - the client picks up its VirtualDOM assuming that the pre-rendered page output is the same. To do this, you need to make sure that your VirtualDOM implementation is deterministic! This could involve either serializing our app state and sending it to the client, hydrating only parts of the page, or building tests to ensure what's rendered on the server is the same as the client.

Any SSR output is hydratable, with no extra configuration required.

```rust, no_run
# use dioxus::prelude::*;
# fn App() -> Element { todo!() }
let mut vdom = VirtualDom::new(App);

## Depends on (same ecosystem)

`dioxus-vdom`

## Public items by file

- `third_party/dioxus/packages/ssr/src/cache.rs`: struct StringChain
- `third_party/dioxus/packages/ssr/src/lib.rs`: mod config, mod renderer, fn render_element, fn render
- `third_party/dioxus/packages/ssr/src/renderer.rs`: struct Renderer

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
