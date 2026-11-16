---
name: bevy-macro-utils
description: "Use when working with the `bevy_macro_utils` crate of bevy: A collection of utils for Bevy Engine."
---

# bevy_macro_utils

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_macro_utils/` (manifest `third_party/bevy/crates/bevy_macro_utils/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_macro_utils
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

A collection of utils for Bevy Engine

## README (`third_party/bevy/crates/bevy_macro_utils/README.md`)

# Bevy Macro Utilities

## Public items by file

- `third_party/bevy/crates/bevy_macro_utils/src/attrs.rs`: fn get_lit_str, fn get_lit_bool
- `third_party/bevy/crates/bevy_macro_utils/src/bevy_manifest.rs`: struct BevyManifest
- `third_party/bevy/crates/bevy_macro_utils/src/fq_std.rs`: struct FQAny, struct FQBox, struct FQClone, struct FQDefault, struct FQOption, struct FQResult, struct FQSend, struct FQSync, struct FQInto, struct FQIterator
- `third_party/bevy/crates/bevy_macro_utils/src/label.rs`: fn ensure_no_collision, fn derive_label, fn pascal_to_snake_case
- `third_party/bevy/crates/bevy_macro_utils/src/lib.rs`: mod fq_std
- `third_party/bevy/crates/bevy_macro_utils/src/member.rs`: fn as_member
- `third_party/bevy/crates/bevy_macro_utils/src/parser.rs`: fn terminated_parser
- `third_party/bevy/crates/bevy_macro_utils/src/path.rs`: fn path_to_string
- `third_party/bevy/crates/bevy_macro_utils/src/path_type.rs`: enum PathType
- `third_party/bevy/crates/bevy_macro_utils/src/result_sifter.rs`: struct ResultSifter
- `third_party/bevy/crates/bevy_macro_utils/src/shape.rs`: fn get_struct_fields, fn require_named
- `third_party/bevy/crates/bevy_macro_utils/src/symbol.rs`: struct Symbol

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
