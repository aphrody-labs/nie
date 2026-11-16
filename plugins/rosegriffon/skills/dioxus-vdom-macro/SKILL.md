---
name: dioxus-vdom-macro
description: "Use when working with the `dioxus-vdom-macro` crate of dioxus: Core macro for Dioxus Virtual DOM."
---

# dioxus-vdom-macro

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/vdom-macro/` (manifest `third_party/dioxus/packages/vdom-macro/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/vdom-macro
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Core macro for Dioxus Virtual DOM

## README (`third_party/dioxus/packages/vdom-macro/README.md`)

# dioxus-vdom-macro

[Website](https://dioxuslabs.com) |
[Guides](https://dioxuslabs.com/learn/0.7/) |
[API Docs](https://docs.rs/dioxus-vdom-macro/latest/dioxus_vdom_macro) |
[Chat](https://discord.gg/XgGxMSkvUM)

## Overview

`dioxus-vdom-macro` provides a handful of helpful macros used by the `dioxus` crate. These include:

- The `rsx!` macro that underpins templates and node creation.
- The `component` attribute macro denotes a function as a Dioxus component. Currently, this:
  - Transforms function arguments into an auto-derived struct.
  - Ensures that your component name uses PascalCase.
  - Probably more stuff in the future. This macro allows us to have a way of distinguishing functions and components, which can be quite handy.
- The `format_args_f` macro which allows f-string formatting with support for expressions.

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

## Depends on (same ecosystem)

`dioxus-rsx`

## Public items by file

- `third_party/dioxus/packages/vdom-macro/src/component.rs`: struct ComponentBody, struct ComponentMacroOptions
- `third_party/dioxus/packages/vdom-macro/src/lib.rs`: derive Props, fn derive_props, macro rsx, attribute component
- `third_party/dioxus/packages/vdom-macro/src/props/mod.rs`: fn impl_my_derive
- `third_party/dioxus/packages/vdom-macro/src/utils.rs`: fn lit_to_string, fn format_type_string, struct DeprecatedAttribute

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
