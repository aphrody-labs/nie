---
name: blitz-net
description: "Use when working with the `blitz-net` crate of blitz: Blitz networking."
---

# blitz-net

- Version: 0.3.0-beta.4 (workspace member of `third_party/blitz`, vendored commit `a84a3c51b562`)
- Source: `third_party/blitz/packages/blitz-net/` (manifest `third_party/blitz/packages/blitz-net/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/blitz/tree/main/packages/blitz-net
- License: [object Object]
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Blitz networking

## Crate docs (`third_party/blitz/packages/blitz-net/src/lib.rs`)

Networking (HTTP, filesystem, Data URIs) for Blitz

Provides an implementation of the [`blitz_traits::net::NetProvider`] trait.

## Cargo features (from the manifest)

- `http2` (enables 1)
- `cookies` (enables 1)
- `multipart` (enables 3)
- `cache` (enables 4)
- `tracing` (enables 1)

## Depends on (same ecosystem)

`blitz-traits`

## Public items by file

- `third_party/blitz/packages/blitz-net/src/lib.rs`: struct Provider, enum ProviderError

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
