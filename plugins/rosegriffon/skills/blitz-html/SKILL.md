---
name: blitz-html
description: "Use when working with the `blitz-html` crate of blitz: Blitz HTML parser."
---

# blitz-html

- Version: 0.3.0-beta.4 (workspace member of `third_party/blitz`, vendored commit `a84a3c51b562`)
- Source: `third_party/blitz/packages/blitz-html/` (manifest `third_party/blitz/packages/blitz-html/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/blitz/tree/main/packages/blitz-html
- License: [object Object]
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Blitz HTML parser

## Cargo features (from the manifest)

- `accessibility` (enables 1)
- `tracing` (enables 2)

## Depends on (same ecosystem)

`blitz-dom`, `blitz-traits`

## Public items by file

- `third_party/blitz/packages/blitz-html/src/html_document.rs`: struct HtmlDocument
- `third_party/blitz/packages/blitz-html/src/html_sink.rs`: struct HtmlProvider, struct DocumentHtmlParser

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
