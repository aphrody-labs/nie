---
name: dioxus-examples
description: "Use when working with the `dioxus-examples` crate of dioxus: Top level crate for the Dioxus repository."
---

# dioxus-examples

- Version: 0.7.5 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/` (manifest `third_party/dioxus/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Top level crate for the Dioxus repository

## README (`third_party/dioxus/README.md`)

Website 
     | 
     Examples 
     | 
     Tutorial 
     | 
     中文 
     | 
     PT-BR 
     | 
     日本語 
     | 
     Türkçe 
     | 
     한국어 
  

Build for web, desktop, and mobile, and more with a single codebase. Zero-config setup, integrated hot-reloading, and signals-based state management. Add backend functionality with Server Functions and bundle with our CLI.

```rust
fn app() -> Element {
    let mut count = use_signal(|| 0);

    rsx! {
        h1 { "High-Five counter: {count}" }
        button { onclick: move |_| count += 1, "Up high!" }
        button { onclick: move |_| count -= 1, "Down low!" }
    }
}
```

## ⭐️ Unique features:

- Cross-platform apps in three lines of code (web, desktop, mobile, server, and more)
- [Ergonomic state management](https://dioxuslabs.com/blog/release-050) combines the best of React, Solid, and Svelte
- Built-in featureful, type-safe, fullstack web framework
- Integrated bundler for deploying to the web, macOS, Linux, and Windows
- Subsecond Rust hot-patching and asset hot-reloading
- And more! [Take a tour of Dioxus](https://dioxuslabs.com/learn/0.7/).

## Instant hot-reloading

With one command, `dx serve` and your app is running. Edit your markup, styles, and see changes in milliseconds. Use our experimental `dx serve --hotpatch` to update Rust code in real time.

  
  
  

## Build Beautiful Apps

Dioxus apps are styled with HTML and CSS. Use the built-in TailwindCSS support or load your favorite CSS library. Easily call into native code (objective-c, JNI, Web-Sys) for a perfect native touch.

  

## Truly fullstack applications

Dioxus deeply integrates with [axum](https://github.com/tokio-rs/axum) to provide powerful fullstack capabilities for both clients and servers. Pick from a wide array of built-in batteries like WebSockets, SSE, Streaming, File Upload/Download, Server-Side-Rendering, Forms, Middleware, and Hot-Reload, or go fully custom and integrate your existing axum backend.

  

## Experimental Native Renderer

Render using web-sys, webview, server-side-rendering, liveview, or even with our experimental WGPU-based renderer. Embed Dioxus in Bevy, WGPU, or even run on embedded Linux!

  

## First-party primitive components

Get started quickly with a complete set of primitives modeled after shadcn/ui and Radix-Primitives.


## Cargo features (from the manifest)

- `default` (enables 1)
- `desktop` (enables 1)
- `native` (enables 2)
- `liveview` (enables 1)
- `server` (enables 1)
- `mobile` (enables 1)
- `web` (enables 1)
- `gpu` (enables 2)

## Depends on (same ecosystem)

`wasm-splitter`

## Examples in the source tree

- `third_party/dioxus/examples/01-app-demos/bluetooth-scanner/src/main.rs`
- `third_party/dioxus/examples/01-app-demos/calculator.rs`
- `third_party/dioxus/examples/01-app-demos/calculator_mutable.rs`
- `third_party/dioxus/examples/01-app-demos/counters.rs`
- `third_party/dioxus/examples/01-app-demos/crm.rs`
- `third_party/dioxus/examples/01-app-demos/dog_app.rs`
- `third_party/dioxus/examples/01-app-demos/ecommerce-site/src/api.rs`
- `third_party/dioxus/examples/01-app-demos/ecommerce-site/src/components/error.rs`

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
