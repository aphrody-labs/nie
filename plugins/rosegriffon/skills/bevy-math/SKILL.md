---
name: bevy-math
description: "Use when working with the `bevy_math` crate of bevy: Provides math functionality for Bevy Engine."
---

# bevy_math

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_math/` (manifest `third_party/bevy/crates/bevy_math/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_math
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides math functionality for Bevy Engine

## README (`third_party/bevy/crates/bevy_math/README.md`)

# Bevy Math

## Cargo features (from the manifest)

- `default` (enables 2)
- `std` (enables 7)
- `alloc` (enables 2)
- `serialize` (enables 2)
- `approx` (enables 2): Enable approx for glam types to approximate floating point equality comparisons and assertions
- `mint` (enables 1): Enable interoperation of glam types with mint-compatible libraries
- `libm` (enables 2): Enable libm mathematical functions for glam types to ensure consistent outputs across platforms at the cost of losing hardware-level optimization using intrinsics
- `glam_assert` (enables 1): Enable assertions to check the validity of parameters passed to glam
- `debug_glam_assert` (enables 1): Enable assertions in debug builds to check the validity of parameters passed to glam
- `rand` (enables 2): Enable the rand dependency for shape_sampling
- `bevy_reflect` (enables 2): Enable bevy_reflect (requires alloc)
- `nostd-libm` (enables 2): Enable libm mathematical functions as a fallback for no_std environments. Can be overridden with std feature.

## Depends on (same ecosystem)

`bevy_reflect`

## Public items by file

- `third_party/bevy/crates/bevy_math/src/affine3.rs`: trait Affine3Ext
- `third_party/bevy/crates/bevy_math/src/aspect_ratio.rs`: struct AspectRatio, enum AspectRatioError
- `third_party/bevy/crates/bevy_math/src/common_traits.rs`: trait VectorSpace, trait ScalarField, struct Sum, trait NormedVectorSpace, trait StableInterpolate, struct MismatchedUnitsError, trait TryStableInterpolate, enum VecInterpolateError, trait HasTangent, struct WithDerivative, struct WithTwoDerivatives
- `third_party/bevy/crates/bevy_math/src/compass.rs`: enum CompassQuadrant, enum CompassOctant
- `third_party/bevy/crates/bevy_math/src/direction.rs`: enum InvalidDirectionError, struct Dir2, struct Dir3, struct Dir3A, struct Dir4
- `third_party/bevy/crates/bevy_math/src/float_ord.rs`: struct FloatOrd
- `third_party/bevy/crates/bevy_math/src/isometry.rs`: struct Isometry2d, struct Isometry3d
- `third_party/bevy/crates/bevy_math/src/lib.rs`: mod common_traits, mod ops, mod sampling, mod prelude
- `third_party/bevy/crates/bevy_math/src/mat3.rs`: fn reflection_matrix
- `third_party/bevy/crates/bevy_math/src/ops.rs`: trait FloatPow
- `third_party/bevy/crates/bevy_math/src/rects/irect.rs`: struct IRect
- `third_party/bevy/crates/bevy_math/src/rects/rect.rs`: struct Rect
- `third_party/bevy/crates/bevy_math/src/rects/urect.rs`: struct URect
- `third_party/bevy/crates/bevy_math/src/rotation2d.rs`: struct Rot2
- `third_party/bevy/crates/bevy_math/src/sampling/mod.rs`: mod standard
- `third_party/bevy/crates/bevy_math/src/sampling/standard.rs`: trait FromRng

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
