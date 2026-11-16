---
name: dioxus-web
description: "Use when working with the `dioxus-web` crate of dioxus: Web-sys renderer for Dioxus: Build fullstack web, desktop, and mobile apps with a single codebase."
---

# dioxus-web

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/web/` (manifest `third_party/dioxus/packages/web/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/web
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Web-sys renderer for Dioxus: Build fullstack web, desktop, and mobile apps with a single codebase.

## README (`third_party/dioxus/packages/web/README.md`)

# Dioxus-web

[Website](https://dioxuslabs.com) |
[Guides](https://dioxuslabs.com/learn/0.7/) |
[API Docs](https://docs.rs/dioxus-web/latest/dioxus_web) |
[Chat](https://discord.gg/XgGxMSkvUM)

## Overview

Run Dioxus in the browser using WebAssembly.

- Relies on [sledgehammer-bindgen](https://github.com/Demonthos/sledgehammer_bindgen) and [web-sys](https://github.com/rustwasm/wasm-bindgen/tree/main/crates/web-sys) to modify the dom
- Supports instant hot reloading via the Dioxus CLI
- Around 60k gzipped

## Contributing

- Report issues on our [issue tracker](https://github.com/dioxuslabs/dioxus/issues).
- Join the discord and ask questions!

## License

This project is licensed under the [MIT license].

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Dioxus by you shall be licensed as MIT without any additional
terms or conditions.

## Cargo features (from the manifest)

- `default` (enables 3)
- `hydrate` (enables 3)
- `mounted` (enables 7)
- `devtools` (enables 8)
- `document` (enables 3)

## Depends on (same ecosystem)

`dioxus-devtools`, `dioxus-document`, `dioxus-fullstack-core`, `dioxus-history`, `dioxus-html`, `dioxus-interpreter-js`, `dioxus-signals`, `dioxus-vdom`, `dx-env`, `generational-box`

## Public items by file

- `third_party/dioxus/packages/web/src/cfg.rs`: struct Config
- `third_party/dioxus/packages/web/src/data_transfer.rs`: struct WebDataTransfer
- `third_party/dioxus/packages/web/src/document.rs`: struct JSOwner, fn init_document, fn init_fullstack_document, struct WebDocument
- `third_party/dioxus/packages/web/src/dom.rs`: struct WebsysDom
- `third_party/dioxus/packages/web/src/events/mod.rs`: trait WebEventExt
- `third_party/dioxus/packages/web/src/files.rs`: struct WebFileData, trait WebFileExt
- `third_party/dioxus/packages/web/src/history.rs`: struct WebHistory, struct HashHistory
- `third_party/dioxus/packages/web/src/launch.rs`: fn launch, fn launch_virtual_dom, fn launch_cfg
- `third_party/dioxus/packages/web/src/lib.rs`: mod launch, fn run

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
