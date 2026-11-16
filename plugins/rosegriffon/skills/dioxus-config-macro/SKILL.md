---
name: dioxus-config-macro
description: "Use when working with the `dioxus-config-macro` crate of dioxus: Configuration macros for Dioxus."
---

# dioxus-config-macro

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/config-macro/` (manifest `third_party/dioxus/packages/config-macro/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/config-macro
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Configuration macros for Dioxus

## README (`third_party/dioxus/packages/config-macro/README.md`)

# dioxus-config-macro

[Website](https://dioxuslabs.com) |
[Guide](https://dioxuslabs.com/learn/0.7/) |
[API Docs](https://docs.rs/dioxus-config-macro/latest/dioxus_config_macro/) |
[Chat](https://discord.gg/XgGxMSkvUM)

## Overview

`dioxus-config-macro` provides a handful of helpful macros to make it easier to work with optional sections of the launch builder.

## Contributing

- Report issues on our [issue tracker](https://github.com/dioxuslabs/dioxus/issues).
- Join the discord and ask questions!

## License

This project is licensed under the [MIT license].

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Dioxus by you shall be licensed as MIT without any additional
terms or conditions.

## Cargo features (from the manifest)

- `default`
- `fullstack`
- `desktop`
- `native`
- `mobile`
- `web`
- `ssr`
- `liveview`

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
