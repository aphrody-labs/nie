---
name: dioxus-server
description: "Use when working with the `dioxus-server` crate of dioxus: Fullstack utilities for Dioxus: Build fullstack web, desktop, and mobile apps with a single codebase."
---

# dioxus-server

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/fullstack-server/` (manifest `third_party/dioxus/packages/fullstack-server/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/fullstack-server
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Fullstack utilities for Dioxus: Build fullstack web, desktop, and mobile apps with a single codebase.

## README (`third_party/dioxus/packages/fullstack-server/README.md`)

# Dioxus Fullstack

[Website](https://dioxuslabs.com) |
[Guides](https://dioxuslabs.com/learn/0.7/) |
[API Docs](https://docs.rs/dioxus-fullstack/latest/dioxus_fullstack/) |
[Chat](https://discord.gg/XgGxMSkvUM)

Fullstack utilities for the [`Dioxus`](https://dioxuslabs.com) framework.

# Features

- Integrates with the [Axum](./examples/axum-hello-world/src/main.rs) server framework with utilities for serving and rendering Dioxus applications.
- Server functions allow you to call code on the server from the client as if it were a normal function.
- Instant RSX Hot reloading with [`dioxus-hot-reload`](https://crates.io/crates/dioxus-hot-reload).
- Passing root props from the server to the client.

# Example
Quickly build fullstack Rust apps with axum and dioxus.

```rust, no_run
use dioxus::prelude::*;

fn main() {
    dioxus::launch(|| {
        let mut meaning = use_action(|| get_meaning("life the universe and everything".into()));

        rsx! {
            h1 { "Meaning of life: {meaning:?}" }
            button { onclick: move |_| meaning.call(), "Run a server function" }
        }
    });
}

#[get("/meaning/?of")]
async fn get_meaning(of: String) -> Result> {
    Ok(of.contains("life").then(|| 42))
}
```

## Axum Integration

If you have an existing Axum router or you need more control over the server, you can use the [`DioxusRouterExt`](https://docs.rs/dioxus-fullstack/0.6.0-alpha.2/dioxus_fullstack/prelude/trait.DioxusRouterExt.html) trait to integrate with your existing Axum router.

First, make sure your `axum` dependency is optional and enabled by the server feature flag. Axum cannot be compiled to wasm, so if it is enabled by default, it will cause a compile error:

```toml
[dependencies]
dioxus = { version = "*", features = ["fullstack"] }
axum = { version = "0.8.0", optional = true }
tokio = { version = "1.0", features = ["full"], optional = true }
dioxus-cli-config = { version = "*", optional = true }

[features]
server = ["dioxus/server", "dep:axum", "dep:tokio", "dioxus-cli-config"]
web = ["dioxus/web"]
```

Then we can set up dioxus with the axum server:

```rust, no_run
#![allow(non_snake_case)]
use dioxus::prelude::*;

// The entry point for the server
#[cfg(feature = "server")]
#[tokio::main]
async fn main() {
    use dioxus_server::DioxusRouterExt;

    // Get the address the server should run on. If the CLI is running, the CLI proxies fullstack into the main address

## Cargo features (from the manifest)

- `default` (enables 2)
- `document`
- `router` (enables 1)
- `default-tls`
- `rustls` (enables 2)
- `axum-no-default`
- `rkyv` (enables 1)
- `server`

## Depends on (same ecosystem)

`dioxus-devtools`, `dioxus-document`, `dioxus-fullstack-core`, `dioxus-history`, `dioxus-hooks`, `dioxus-html`, `dioxus-interpreter-js`, `dioxus-logger`, `dioxus-router`, `dioxus-signals`, `dioxus-ssr`, `dioxus-vdom`, `dioxus-vdom-macro`, `dx-env`, `generational-box`, `subsecond`

## Public items by file

- `third_party/dioxus/packages/fullstack-server/src/config.rs`: struct ServeConfig, enum StreamingMode
- `third_party/dioxus/packages/fullstack-server/src/document.rs`: struct ServerDocument
- `third_party/dioxus/packages/fullstack-server/src/index_html.rs`: struct IndexHtml
- `third_party/dioxus/packages/fullstack-server/src/isrg/config.rs`: struct IncrementalRendererConfig
- `third_party/dioxus/packages/fullstack-server/src/isrg/freshness.rs`: struct RenderFreshness
- `third_party/dioxus/packages/fullstack-server/src/isrg/mod.rs`: struct CachedRender, struct IncrementalRenderer, enum IncrementalRendererError
- `third_party/dioxus/packages/fullstack-server/src/launch.rs`: fn launch, fn launch_cfg, fn router, fn serve, fn serve_router
- `third_party/dioxus/packages/fullstack-server/src/lib.rs`: mod redirect, mod server, mod config, mod serverfn, mod isrg
- `third_party/dioxus/packages/fullstack-server/src/redirect.rs`: type RedirectHook, fn set_redirect_hook, fn call_redirect_hook
- `third_party/dioxus/packages/fullstack-server/src/server.rs`: trait DioxusRouterExt, fn render_handler, struct FullstackState
- `third_party/dioxus/packages/fullstack-server/src/serverfn.rs`: struct ServerFunction
- `third_party/dioxus/packages/fullstack-server/src/ssr.rs`: enum SSRError

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
