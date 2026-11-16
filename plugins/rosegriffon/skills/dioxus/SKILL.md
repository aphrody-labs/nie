---
name: dioxus
description: "Use when working with the `dioxus` crate of dioxus: Build fullstack web, desktop, and mobile apps with a single codebase."
---

# dioxus

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/dioxus/` (manifest `third_party/dioxus/packages/dioxus/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/dioxus
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Orientation

The `dioxus` crate is a facade over `dioxus-core` (virtual DOM), `dioxus-rsx`/`dioxus-core-macro` (the `rsx!` macro), `dioxus-hooks` and `dioxus-signals` (state), `dioxus-html` (typed elements and events), plus renderer crates (`dioxus-web`, `dioxus-desktop`, `dioxus-native` on Blitz, `dioxus-ssr`, `dioxus-liveview`, fullstack crates) selected by features in `third_party/dioxus/packages/dioxus/Cargo.toml`.

Reading order at the vendored commit: `third_party/dioxus/README.md`, `packages/dioxus/src/lib.rs` (prelude and feature gates), `packages/signals/README.md` and `packages/signals/docs`, `packages/router`, then `examples/`. The CLI is `packages/cli` (package `dioxus-cli`, binary `dx`); build tooling runs in the prd container, not on the workstation.

Monorepo rules: `dioxus` and `dioxus-ssr` are in `[workspace.dependencies]` at `0.8.0-alpha.3` and patched to `third_party/dioxus`; crates use `dioxus = { workspace = true, features = [...] }`, never a registry version, so Bevy, Dioxus and Blitz share one Taffy. The fork adds `[profile.rg-release]` and `[profile.rg-wasm]` (fat LTO, `codegen-units = 1`, size-optimised WASM).

## Purpose

Build fullstack web, desktop, and mobile apps with a single codebase.

## README (`third_party/dioxus/packages/dioxus/README.md`)

Website 
     | 
     Getting Started 
     | 
     Book (0.7) 
     | 
     Examples 
     | 
     Tutorial 
  

---

Build for web, desktop, and mobile, and more with a single codebase. Zero-config setup, integrated hot-reloading, and signals-based state management. Add backend functionality with Server Functions and bundle with our CLI.

```rust, ignore
use dioxus::prelude::*;

fn main() {
    dioxus::launch(app);
}

fn app() -> Element {
    let mut count = use_signal(|| 0);

    rsx! {
        h1 { "High-Five counter: {count}" }
        button { onclick: move |_| count += 1, "Up high!" }
        button { onclick: move |_| count -= 1, "Down low!" }
    }
}
```

## ⭐️ Unique features

- Cross-platform apps in three lines of code (web, desktop, mobile, server, and more)
- [Ergonomic state management](https://dioxuslabs.com/blog/release-050) combines the best of React, Solid, and Svelte
- Built-in featureful, type-safe, fullstack web framework
- Integrated bundler for deploying to the web, macOS, Linux, and Windows
- Subsecond Rust hot-patching and asset hot-reloading
- And more! [Take a tour of Dioxus](https://dioxuslabs.com/learn/0.7/).

## Quick start

Grab the Dioxus CLI (`dx`) and spin up a new app in seconds:

```shell
# install the dioxus-cli
curl -fsSL https://dioxuslabs.com/install.sh | bash

# create a new app, following the template instructions
dx new my-app && cd my-app

# and then serve on `--web, --desktop, --ios, or --android`
dx serve --desktop
```

## Guides

This overview doesn't cover everything. Make sure to check out:

- [Long-form tutorial](https://dioxuslabs.com/learn/0.7/tutorial)
- [Core concept guide](https://dioxuslabs.com/learn/0.7/essentials/)
- [Platform-specific guides](https://dioxuslabs.com/learn/0.7/guides/)
- [Code examples on GitHub](https://github.com/DioxusLabs/dioxus/tree/main/examples)

## Instant hot-reloading

With one command, `dx serve` and your app is running. Edit your markup, styles, and see changes in milliseconds. Use our experimental `dx serve --hotpatch` to update Rust code in real time.


## Cargo features (from the manifest)

- `default` (enables 4)
- `lib` (enables 9): All features recommended for use in libraries
- `minimal` (enables 5): The minimal set of features required to use dioxus renderers for minimal binary size
- `signals` (enables 2)
- `macro` (enables 1)
- `html` (enables 1)
- `hooks` (enables 1)
- `devtools` (enables 2)
- `mounted` (enables 1)
- `asset` (enables 3)
- `document` (enables 3)
- `logger` (enables 1)
- `cli-config` (enables 1)
- `warnings` (enables 1)
- `wasm-split` (enables 1)
- `launch` (enables 1)
- `router` (enables 1)
- `fullstack` (enables 9): Platforms
- `desktop` (enables 3)
- `mobile` (enables 3)
- `web` (enables 6)
- `ssr` (enables 2)
- `liveview` (enables 2)
- `native` (enables 4)
- `server` (enables 5)
- `third-party-renderer`: This feature just disables the no-renderer-enabled warning

## Depends on (same ecosystem)

`dioxus-asset-resolver`, `dioxus-cli-config`, `dioxus-config-macro`, `dioxus-core`, `dioxus-desktop`, `dioxus-devtools`, `dioxus-fullstack`, `dioxus-fullstack-macro`, `dioxus-history`, `dioxus-liveview`, `dioxus-logger`, `dioxus-native`, `dioxus-router`, `dioxus-server`, `dioxus-web`, `manganis`, `subsecond`

## Public items by file

- `third_party/dioxus/packages/dioxus/src/launch.rs`: fn launch, struct LaunchBuilder, type LaunchFn, type ContextFn
- `third_party/dioxus/packages/dioxus/src/lib.rs`: mod events, mod prelude

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
