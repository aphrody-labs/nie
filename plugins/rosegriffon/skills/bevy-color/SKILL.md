---
name: bevy-color
description: "Use when working with the `bevy_color` crate of bevy: Types for representing and manipulating color values."
---

# bevy_color

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_color/` (manifest `third_party/bevy/crates/bevy_color/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_color
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Types for representing and manipulating color values

## README (`third_party/bevy/crates/bevy_color/README.md`)

# Bevy Color

## Cargo features (from the manifest)

- `default` (enables 3)
- `std` (enables 6)
- `alloc` (enables 3)
- `serialize` (enables 3)
- `bevy_reflect` (enables 1)
- `wgpu-types` (enables 1)
- `encase` (enables 2)
- `libm` (enables 1)
- `critical-section` (enables 1)

## Depends on (same ecosystem)

`bevy_curve`, `bevy_math`, `bevy_reflect`

## Public items by file

- `third_party/bevy/crates/bevy_color/src/color.rs`: enum Color
- `third_party/bevy/crates/bevy_color/src/color_difference.rs`: trait EuclideanDistance
- `third_party/bevy/crates/bevy_color/src/color_gradient.rs`: struct ColorCurve
- `third_party/bevy/crates/bevy_color/src/color_ops.rs`: trait Luminance, trait Mix, trait Gray, trait Alpha, trait Hue, trait Saturation, trait ColorToComponents, trait ColorToPacked
- `third_party/bevy/crates/bevy_color/src/color_range.rs`: trait ColorRange
- `third_party/bevy/crates/bevy_color/src/hsla.rs`: struct Hsla
- `third_party/bevy/crates/bevy_color/src/hsva.rs`: struct Hsva
- `third_party/bevy/crates/bevy_color/src/hwba.rs`: struct Hwba
- `third_party/bevy/crates/bevy_color/src/laba.rs`: struct Laba
- `third_party/bevy/crates/bevy_color/src/lcha.rs`: struct Lcha
- `third_party/bevy/crates/bevy_color/src/lib.rs`: mod color_difference, mod palettes, mod prelude
- `third_party/bevy/crates/bevy_color/src/linear_rgba.rs`: struct LinearRgba
- `third_party/bevy/crates/bevy_color/src/okhsla.rs`: struct Okhsla
- `third_party/bevy/crates/bevy_color/src/okhsva.rs`: struct Okhsva
- `third_party/bevy/crates/bevy_color/src/okhwba.rs`: struct Okhwba
- `third_party/bevy/crates/bevy_color/src/oklaba.rs`: struct Oklaba
- `third_party/bevy/crates/bevy_color/src/oklcha.rs`: struct Oklcha
- `third_party/bevy/crates/bevy_color/src/palettes/mod.rs`: mod basic, mod css, mod tailwind
- `third_party/bevy/crates/bevy_color/src/primaries.rs`: struct Chromaticity, struct RgbPrimaries
- `third_party/bevy/crates/bevy_color/src/srgba.rs`: struct Srgba, enum HexColorError
- `third_party/bevy/crates/bevy_color/src/test_colors.rs`: struct TestColor
- `third_party/bevy/crates/bevy_color/src/xyza.rs`: struct Xyza

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
