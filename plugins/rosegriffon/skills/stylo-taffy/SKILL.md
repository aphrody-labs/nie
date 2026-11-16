---
name: stylo-taffy
description: "Use when working with the `stylo_taffy` crate of blitz: Interop crate for the stylo and taffy crates."
---

# stylo_taffy

- Version: 0.3.0-beta.4 (workspace member of `third_party/blitz`, vendored commit `a84a3c51b562`)
- Source: `third_party/blitz/packages/stylo_taffy/` (manifest `third_party/blitz/packages/stylo_taffy/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/blitz/tree/main/packages/stylo_taffy
- License: MIT OR Apache-2.0 OR MPL-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Interop crate for the stylo and taffy crates

## Crate docs (`third_party/blitz/packages/stylo_taffy/src/lib.rs`)

Conversion functions from Stylo types to Taffy types

This crate is an implementation detail of [`blitz-dom`](https://docs.rs/blitz-dom), but can also be
used standalone, and serves as useful reference for anyone wanting to integrate [`stylo`](::style) with [`taffy`]

## Depends on (same ecosystem)

`taffy`

## Public items by file

- `third_party/blitz/packages/stylo_taffy/src/convert.rs`: fn length_percentage, fn dimension, fn min_size, fn max_size, fn margin, fn border, fn inset, fn button_input_display, fn is_block, fn is_table, fn display, fn box_generation_mode, fn box_sizing, fn resolve_calc_value, fn inset_rect, fn position, fn establishes_fixed_containing_block, fn establishes_absolute_containing_block, fn containing_block_claims, fn inline_containing_block_claims, fn overflow, fn contain, fn direction, fn aspect_ratio, ...
- `third_party/blitz/packages/stylo_taffy/src/lib.rs`: mod convert, mod writing_mode
- `third_party/blitz/packages/stylo_taffy/src/wrapper.rs`: struct TaffyStyloStyle, struct GridAreaWrapper, struct StyloLineNameIter, struct RepetitionWrapper
- `third_party/blitz/packages/stylo_taffy/src/writing_mode.rs`: trait WritingModeExt

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
