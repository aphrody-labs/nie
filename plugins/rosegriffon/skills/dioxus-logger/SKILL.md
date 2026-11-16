---
name: dioxus-logger
description: "Use when working with the `dioxus-logger` crate of dioxus: A logging utility to provide a standard interface whether you're targeting web desktop, fullstack, and more."
---

# dioxus-logger

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/logger/` (manifest `third_party/dioxus/packages/logger/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/logger
- License: MIT
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

A logging utility to provide a standard interface whether you're targeting web desktop, fullstack, and more.

## README (`third_party/dioxus/packages/logger/README.md`)

📡 Dioxus Logger 🛰️
  A logging utility to provide a standard interface whether you're targeting web, desktop, fullstack, and more.

  
  
    
  
  
  
    
  
  
  
    
  

-----

`dioxus-logger` is a basic cross-platform facade for logging in [Dioxus](https://dioxuslabs.com/) that uses the [tracing](https://crates.io/crates/tracing) crate.

```rust
use dioxus::prelude::*;
use dioxus_logger::tracing::{Level, info};

fn main() {
  dioxus_logger::init(Level::INFO).expect("logger failed to init");
  dioxus::launch(App);
}

#[component]
fn App() -> Element {
  info!("App rendered");
  rsx! {
    p { "hi" }
  }
}
```

## Dioxus support

As of v0.6, dioxus_logger is part of dioxus itself. Dioxus will call `init` with a default Level, though you can still override the default with `init`.

```rust
use dioxus::prelude::*;
use dioxus::logger::tracing::{Level, info};

fn main() {
  dioxus::logger::init(Level::INFO).expect("logger failed to init");
  dioxus::launch(App);
}

#[component]
fn App() -> Element {
  info!("App rendered");
  rsx! {
    p { "hi" }
  }
}
```
For non-wasm targets, runtime filtering is based on the `RUST_LOG` environment variable. e.g. for `RUST_LOG=none,crateName=trace` only logs trace and above for `crateName` will be captured. See [here](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives) for syntax. For crates with `-` in the name, these need to be changed to `_` in `RUST_LOG`.

## Platform Support
Dioxus logger will eventually support every target that Dioxus does. Currently mobile and TUI are not supported.

## Installation
`dioxus_logger` is part of Dioxus v0.6. If you're using Dioxus v0.6, then no installation is required!

If you're on Dioxus v0.5 and below, you can add `dioxus-logger` to your application by adding it to your dependencies.
```toml
[dependencies]

## Cargo features (from the manifest)

- `default`

## Depends on (same ecosystem)

`dx-env`

## Public items by file

- `third_party/dioxus/packages/logger/src/lib.rs`: fn initialize_default, fn init

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
