---
name: dioxus-interpreter-js
description: "Use when working with the `dioxus-interpreter-js` crate of dioxus: JS Interpreter for Dioxus - a concurrent renderer-agnostic Virtual DOM for interactive user experiences."
---

# dioxus-interpreter-js

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/interpreter/` (manifest `third_party/dioxus/packages/interpreter/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/interpreter
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

JS Interpreter for Dioxus - a concurrent renderer-agnostic Virtual DOM for interactive user experiences

## README (`third_party/dioxus/packages/interpreter/README.md`)

# `dioxus-interpreter-js`

[Website](https://dioxuslabs.com) |
[Guides](https://dioxuslabs.com/learn/0.7/) |
[API Docs](https://docs.rs/dioxus-interpreter-js/latest/dioxus_interpreter_js) |
[Chat](https://discord.gg/XgGxMSkvUM)

## Overview

`dioxus-interpreter-js` provides the high-performance JavaScript glue that interprets the stream of edits produced by the Dioxus VirtualDom and converts them into mutations on the actual web DOM.

This crate features bindings for the web and sledgehammer for increased performance.

## Architecture

We use TypeScript to write the bindings and a very simple build.rs along with bun to convert them to javascript, minify them, and glue them into the rest of the project.

Not every snippet of JS will be used, so we split out the snippets from the core interpreter.

In theory, we _could_ use Rust in the browser to do everything these bindings are doing. In reality, we want to stick with JS to skip the need for a WASM build step when running the LiveView and WebView renderers. We also want to use JS to prevent diverging behavior of things like canceling events, uploading files, and collecting form inputs. These details are tough to ensure 1:1 compatibility when implementing them in two languages.

If you want to contribute to the bindings, you'll need to have the typescript compiler installed on your machine as well as bun:

## Contributing

- Report issues on our [issue tracker](https://github.com/dioxuslabs/dioxus/issues).
- Join the discord and ask questions!

## License

This project is licensed under the [MIT license].

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Dioxus by you shall be licensed as MIT without any additional
terms or conditions.

## Depends on (same ecosystem)

`dioxus-html`, `dioxus-vdom`

## Public items by file

- `third_party/dioxus/packages/interpreter/src/lib.rs`: mod unified_bindings, mod minimal_bindings
- `third_party/dioxus/packages/interpreter/src/write_native_mutations.rs`: struct MutationState

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
