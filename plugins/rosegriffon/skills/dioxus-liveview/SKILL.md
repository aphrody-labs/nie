---
name: dioxus-liveview
description: "Use when working with the `dioxus-liveview` crate of dioxus: Build server-side apps with Dioxus."
---

# dioxus-liveview

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/liveview/` (manifest `third_party/dioxus/packages/liveview/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/liveview
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Build server-side apps with Dioxus

## README (`third_party/dioxus/packages/liveview/README.md`)

# Dioxus Liveview

[Website](https://dioxuslabs.com) |
[Guides](https://dioxuslabs.com/learn/0.7/) |
[API Docs](https://docs.rs/dioxus-liveview/latest/dioxus_liveview) |
[Chat](https://discord.gg/XgGxMSkvUM)

## Overview

`dioxus-liveview` provides adapters for running the Dioxus VirtualDom over a WebSocket connection.

The current backend frameworks supported include:

- Axum

Dioxus-LiveView exports some primitives to wire up an app into an existing backend framework.

- A ThreadPool for spawning the `!Send` VirtualDom and interacting with it from WebSockets
- An adapter for transforming various socket types into the `LiveViewSocket` type
- The glue to load the interpreter into your app

## Contributing

- Report issues on our [issue tracker](https://github.com/dioxuslabs/dioxus/issues).
- Join the discord and ask questions!

## License

This project is licensed under the [MIT license].

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Dioxus by you shall be licensed as MIT without any additional
terms or conditions.

## Cargo features (from the manifest)

- `default` (enables 2)
- `axum` (enables 1)
- `multi-thread` (enables 1)
- `devtools` (enables 1)

## Depends on (same ecosystem)

`dioxus-cli-config`, `dioxus-devtools`, `dioxus-document`, `dioxus-history`, `dioxus-html`, `dioxus-interpreter-js`, `dioxus-vdom`, `generational-box`

## Public items by file

- `third_party/dioxus/packages/liveview/src/adapters/axum_adapter.rs`: fn axum_socket
- `third_party/dioxus/packages/liveview/src/adapters/mod.rs`: mod axum_adapter, trait LiveviewRouter
- `third_party/dioxus/packages/liveview/src/config.rs`: struct Config
- `third_party/dioxus/packages/liveview/src/document.rs`: fn init_document, struct LiveviewDocument
- `third_party/dioxus/packages/liveview/src/element.rs`: struct LiveviewElement
- `third_party/dioxus/packages/liveview/src/launch.rs`: type Config, fn launch
- `third_party/dioxus/packages/liveview/src/lib.rs`: mod pool, mod launch, trait WebsocketTx, trait WebsocketRx, enum LiveViewError, fn interpreter_glue
- `third_party/dioxus/packages/liveview/src/pool.rs`: struct LiveViewPool, trait LiveViewSocket, fn run
- `third_party/dioxus/packages/liveview/src/query.rs`: enum QueryError

## Examples in the source tree

- `third_party/dioxus/packages/liveview/examples/axum.rs`
- `third_party/dioxus/packages/liveview/examples/axum_stress.rs`

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
