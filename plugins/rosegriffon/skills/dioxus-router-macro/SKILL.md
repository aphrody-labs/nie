---
name: dioxus-router-macro
description: "Use when working with the `dioxus-router-macro` crate of dioxus: Macro for Dioxus Router."
---

# dioxus-router-macro

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/router-macro/` (manifest `third_party/dioxus/packages/router-macro/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/router-macro
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Macro for Dioxus Router

## Cargo features (from the manifest)

- `default`
- `web`

## Public items by file

- `third_party/dioxus/packages/router-macro/src/hash.rs`: struct HashFragment
- `third_party/dioxus/packages/router-macro/src/layout.rs`: struct LayoutId, struct Layout
- `third_party/dioxus/packages/router-macro/src/lib.rs`: fn routable
- `third_party/dioxus/packages/router-macro/src/nest.rs`: struct NestId, struct Nest
- `third_party/dioxus/packages/router-macro/src/query.rs`: enum QuerySegment, struct FullQuerySegment, struct QueryArgument
- `third_party/dioxus/packages/router-macro/src/route_tree.rs`: struct StaticErrorVariant, struct PathIter
- `third_party/dioxus/packages/router-macro/src/segment.rs`: enum RouteSegment, fn static_segment_idx, fn parse_route_segments

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
