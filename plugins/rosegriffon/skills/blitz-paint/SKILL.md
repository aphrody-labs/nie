---
name: blitz-paint
description: "Use when working with the `blitz-paint` crate of blitz: Paint a Blitz Document using anyrender."
---

# blitz-paint

- Version: 0.3.0-beta.4 (workspace member of `third_party/blitz`, vendored commit `a84a3c51b562`)
- Source: `third_party/blitz/packages/blitz-paint/` (manifest `third_party/blitz/packages/blitz-paint/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/blitz/tree/main/packages/blitz-paint
- License: [object Object]
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Paint a Blitz Document using anyrender

## Crate docs (`third_party/blitz/packages/blitz-paint/src/lib.rs`)

Paint a [`blitz_dom::BaseDocument`] by pushing [`anyrender`] drawing commands into
an impl [`anyrender::PaintScene`].

## Cargo features (from the manifest)

- `default` (enables 1)
- `tracing` (enables 1)
- `svg` (enables 2)
- `custom-widget` (enables 1)
- `font-embolden`
- `apple-font-embolden`
- `scrollbars` (enables 1): Paint overlay scrollbar thumbs on scroll containers, and enable their drag/hover interaction in blitz-dom (off by default while the feature matures).

## Depends on (same ecosystem)

`blitz-dom`, `blitz-traits`, `taffy`

## Public items by file

- `third_party/blitz/packages/blitz-paint/src/color.rs`: type Color, trait ToColorColor
- `third_party/blitz/packages/blitz-paint/src/kurbo_css/css_box.rs`: struct CssBox
- `third_party/blitz/packages/blitz-paint/src/kurbo_css/mod.rs`: enum Edge
- `third_party/blitz/packages/blitz-paint/src/kurbo_css/non_uniform_radii.rs`: struct NonUniformRoundedRectRadii
- `third_party/blitz/packages/blitz-paint/src/lib.rs`: fn paint_scene
- `third_party/blitz/packages/blitz-paint/src/render.rs`: struct BlitzDomPainter

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
