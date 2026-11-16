---
name: dioxus-fullstack-core
description: "Use when working with the `dioxus-fullstack-core` crate of dioxus: Hooks for serializing futures, values in dioxus-fullstack and other utilities."
---

# dioxus-fullstack-core

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/fullstack-core/` (manifest `third_party/dioxus/packages/fullstack-core/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/fullstack-core
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Hooks for serializing futures, values in dioxus-fullstack and other utilities

## README (`third_party/dioxus/packages/fullstack-core/README.md`)

# Dioxus Fullstack Core

Dioxus-fullstack-core provides types, traits, hooks, contexts for [`dioxus-fullstack`](https://crates.io/crates/dioxus-fullstack). Libraries that need to integrate with dioxus-fullstack should rely on this crate instead of the full-fledged renderer for quicker build times.

## Usage

To start using this crate, you can run the following command:

```bash
cargo add dioxus-fullstack-hooks
```

Then you can use hooks like `use_server_future` in your components:

```rust
use dioxus::prelude::*;

fn App() -> Element {
    let mut article_id = use_signal(|| 0);

    // `use_server_future` will spawn a task that runs on the server and serializes the result to send to the client.
    // The future will rerun any time the
    // Since we bubble up the suspense with `?`, the server will wait for the future to resolve before rendering
    let article = use_server_future(move || fetch_article(article_id()))?;

    rsx! {
        "{article().unwrap()}"
    }
}

async fn fetch_article(id: u32) -> String {
    format!("Article {}", id)
}
```

## Cargo features (from the manifest)

- `web`
- `server`

## Depends on (same ecosystem)

`dioxus-document`, `dioxus-history`, `dioxus-hooks`, `dioxus-signals`, `dioxus-vdom`, `generational-box`

## Public items by file

- `third_party/dioxus/packages/fullstack-core/src/document.rs`: struct FullstackWebDocument
- `third_party/dioxus/packages/fullstack-core/src/error.rs`: type ServerFnResult, enum ServerFnError, enum RequestError
- `third_party/dioxus/packages/fullstack-core/src/errors.rs`: fn init_error_boundary
- `third_party/dioxus/packages/fullstack-core/src/history.rs`: fn provide_fullstack_history_context
- `third_party/dioxus/packages/fullstack-core/src/httperror.rs`: struct HttpError, trait OrHttpError, struct AnyhowMarker
- `third_party/dioxus/packages/fullstack-core/src/lib.rs`: mod document, mod history, mod error, mod httperror
- `third_party/dioxus/packages/fullstack-core/src/loader.rs`: fn use_loader, struct Loader, enum LoaderState, struct LoaderHandle, enum Loading
- `third_party/dioxus/packages/fullstack-core/src/server_cached.rs`: fn use_server_cached
- `third_party/dioxus/packages/fullstack-core/src/server_future.rs`: fn use_server_future
- `third_party/dioxus/packages/fullstack-core/src/streaming.rs`: struct FullstackContext, struct FullstackContextInner, enum StreamingStatus, fn commit_initial_chunk, fn extract, fn current_status, fn status_code_from_error
- `third_party/dioxus/packages/fullstack-core/src/transport.rs`: struct HydrationContext, struct SerializeContextEntry, fn is_hydrating, fn serialize_context, struct SerializedHydrationData, enum TakeDataError, fn head_element_hydration_entry, trait Transportable, struct TransportViaErrMarker, struct TransportCapturedError

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
