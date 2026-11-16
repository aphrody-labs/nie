---
name: dioxus-document
description: "Use when working with the `dioxus-document` crate of dioxus: CLI Configuration for dioxus-cli."
---

# dioxus-document

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/document/` (manifest `third_party/dioxus/packages/document/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/document
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

CLI Configuration for dioxus-cli

## Depends on (same ecosystem)

`dioxus-html`, `dioxus-vdom`, `dioxus-vdom-macro`, `generational-box`

## Public items by file

- `third_party/dioxus/packages/document/src/document.rs`: type DocumentContext, fn create_element_in_head, trait Document, struct NoOpDocument
- `third_party/dioxus/packages/document/src/elements/link.rs`: struct OtherLinkProps, struct LinkProps, fn Link, fn insert_stylesheet
- `third_party/dioxus/packages/document/src/elements/meta.rs`: struct MetaProps, fn Meta
- `third_party/dioxus/packages/document/src/elements/mod.rs`: enum ExtractSingleTextNodeError
- `third_party/dioxus/packages/document/src/elements/script.rs`: struct ScriptProps, fn Script
- `third_party/dioxus/packages/document/src/elements/style.rs`: struct StyleProps, fn Style
- `third_party/dioxus/packages/document/src/elements/stylesheet.rs`: fn Stylesheet
- `third_party/dioxus/packages/document/src/elements/title.rs`: struct TitleProps, fn Title
- `third_party/dioxus/packages/document/src/error.rs`: enum EvalError
- `third_party/dioxus/packages/document/src/eval.rs`: struct Eval, trait Evaluator
- `third_party/dioxus/packages/document/src/lib.rs`: fn document, fn eval

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
