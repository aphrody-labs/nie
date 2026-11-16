---
name: dioxus-autofmt
description: "Use when working with the `dioxus-autofmt` crate of dioxus: Autofomatter for Dioxus RSX."
---

# dioxus-autofmt

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/autofmt/` (manifest `third_party/dioxus/packages/autofmt/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/autofmt
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Autofomatter for Dioxus RSX

## README (`third_party/dioxus/packages/autofmt/README.md`)

# dioxus-autofmt

[Website](https://dioxuslabs.com) |
[Guides](https://dioxuslabs.com/learn/0.7/) |
[API Docs](https://docs.rs/dioxus-autofmt/latest/dioxus_autofmt) |
[Chat](https://discord.gg/XgGxMSkvUM)

## Overview

`dioxus-autofmt` provides a pretty printer for the `rsx` syntax tree.

This is done manually with a via set of formatting rules. The output is not guaranteed to be stable between minor versions of the crate as we might tweak the output.

`dioxus-autofmt` provides an API to perform precision edits as well as just spit out a block of formatted RSX from any RSX syntax tree. This is used by the `rsx-rosetta` crate which can accept various input languages and output valid RSX.

## Contributing

- Report issues on our [issue tracker](https://github.com/dioxuslabs/dioxus/issues).
- Join the discord and ask questions!

## License

This project is licensed under the [MIT license].

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Dioxus by you shall be licensed as MIT without any additional
terms or conditions.

## Depends on (same ecosystem)

`dioxus-rsx`

## Public items by file

- `third_party/dioxus/packages/autofmt/src/buffer.rs`: struct Buffer
- `third_party/dioxus/packages/autofmt/src/collect_macros.rs`: fn collect_from_file, fn byte_offset
- `third_party/dioxus/packages/autofmt/src/indent.rs`: enum IndentType, struct IndentOptions
- `third_party/dioxus/packages/autofmt/src/lib.rs`: struct FormattedBlock, fn fmt_file, fn try_fmt_file, fn write_block_out, fn fmt_block, fn apply_formats
- `third_party/dioxus/packages/autofmt/src/prettier_please.rs`: fn unparse_expr, fn unparse_inner, fn unparse_pat
- `third_party/dioxus/packages/autofmt/src/writer.rs`: struct Writer

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
