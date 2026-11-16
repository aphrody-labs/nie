---
name: bevy-bsn
description: "Use when working with the `bevy_bsn` crate of bevy: Enables parsing the BSN file format."
---

# bevy_bsn

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_bsn/` (manifest `third_party/bevy/crates/bevy_bsn/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_bsn
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Enables parsing the BSN file format

## Crate docs (`third_party/bevy/crates/bevy_bsn/src/lib.rs`)

Support for parsing the BSN file format

## Public items by file

- `third_party/bevy/crates/bevy_bsn/src/lex.rs`: enum Token, struct SpannedToken, enum LexError
- `third_party/bevy/crates/bevy_bsn/src/lib.rs`: mod lex, mod parse, mod parse_stream, mod span, mod types
- `third_party/bevy/crates/bevy_bsn/src/parse_stream.rs`: enum ParseError, struct OwnedSpannedToken, struct ParseStream, trait Parse, trait Peek
- `third_party/bevy/crates/bevy_bsn/src/span.rs`: struct Span, struct OwnedSpan
- `third_party/bevy/crates/bevy_bsn/src/types.rs`: struct BsnRoot, struct Bsn, enum BsnEntry, enum BsnScene, struct BsnSceneList, struct BsnSceneListItems, struct BsnType, enum BsnFields, struct BsnNamedField, enum BsnValue, struct Path, struct PathSegment, struct Bool, struct Ident, struct StringLit, struct Int, struct Float, struct LBracket, struct RBracket, struct LParen, struct RParen, struct LBrace, struct RBrace, struct Comma, ...

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
