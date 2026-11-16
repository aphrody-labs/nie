---
name: depinfo
description: "Use when working with the `depinfo` crate of dioxus: Depinfo parser for Rust."
---

# depinfo

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/depinfo/` (manifest `third_party/dioxus/packages/depinfo/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/depinfo
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Depinfo parser for Rust

## README (`third_party/dioxus/packages/depinfo/README.md`)

# Rustc Dep-Info Parser

This crate parses the output of rustc's `.d` dep-info file. It is used by the hot-reloading engine and other libraries to provide higher quality dependency analysis for the user's project.

## License

This project is licensed under either the [MIT license] or the [Apache-2 License].

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Dioxus by you, shall be licensed as MIT or Apache-2, without any additional
terms or conditions.

## Public items by file

- `third_party/dioxus/packages/depinfo/src/lib.rs`: enum DepInfoParseError, struct RustcDepInfo

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
