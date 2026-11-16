---
name: dioxus-rsx-rosetta
description: "Use when working with the `dioxus-rsx-rosetta` crate of dioxus: Autofomatter for Dioxus RSX."
---

# dioxus-rsx-rosetta

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/rsx-rosetta/` (manifest `third_party/dioxus/packages/rsx-rosetta/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/rsx-rosetta
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Autofomatter for Dioxus RSX

## README (`third_party/dioxus/packages/rsx-rosetta/README.md`)

# `rsx-rosetta`

[Website](https://dioxuslabs.com) |
[Guides](https://dioxuslabs.com/learn/0.7/) |
[API Docs](https://docs.rs/rsx-rosetta/latest/rsx_rosetta) |
[Chat](https://discord.gg/XgGxMSkvUM)

## Overview

Dioxus sports its own templating language inspired by C#/Kotlin/RTMP, etc. It's pretty straightforward.

However, it's NOT HTML. This is done since HTML is verbose and you'd need a dedicated LSP or IDE integration to get a good DX in .rs files.

RSX is simple... It's similar enough to regular Rust code to trick most IDEs into automatically providing support for things like block selections, folding, highlighting, etc.

To accommodate the transition from HTML to RSX, you might need to translate some existing code.

This library provides a central AST that can accept a number of inputs:

- HTML
- Syn (todo)
- Akama (todo)
- Jinja (todo)

From there, you can convert directly to a string or into some other AST.

## Contributing

- Report issues on our [issue tracker](https://github.com/dioxuslabs/dioxus/issues).
- Join the discord and ask questions!

## License

This project is licensed under the [MIT license].

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Dioxus by you, shall be licensed as MIT, without any additional
terms or conditions.

## Depends on (same ecosystem)

`dioxus-autofmt`, `dioxus-html`, `dioxus-rsx`

## Public items by file

- `third_party/dioxus/packages/rsx-rosetta/src/lib.rs`: fn rsx_from_html, fn rsx_node_from_html, fn collect_svgs

## Examples in the source tree

- `third_party/dioxus/packages/rsx-rosetta/examples/html.rs`

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
