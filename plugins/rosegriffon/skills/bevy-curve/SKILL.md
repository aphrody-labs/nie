---
name: bevy-curve
description: "Use when working with the `bevy_curve` crate of bevy: Provides curve functionality for Bevy Engine."
---

# bevy_curve

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_curve/` (manifest `third_party/bevy/crates/bevy_curve/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_curve
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides curve functionality for Bevy Engine

## Crate docs (`third_party/bevy/crates/bevy_curve/src/lib.rs`)

The [`Curve`] trait, providing a domain-agnostic description of curves.

## Overview

At a high level, [`Curve`] is a trait that abstracts away the implementation details of curves,
which comprise any kind of data parametrized by a single continuous variable. For example, that
variable could represent time, in which case a curve would represent a value that changes over
time, as in animation; on the other hand, it could represent something like displacement or
distance, as in graphs, gradients, and curves in space.

The trait itself has two fundamental components: a curve must have a [domain], which is a nonempty
range of `f32` values, and it must be able to be [sampled] on every one of those values, producing
output of some fixed type.

A primary goal of the trait is to allow interfaces to simply accept `impl Curve<T>` as input
rather than requiring for input curves to be defined in data in any particular way. This is
supported by a number of interface methods which allow [changing parametrizations], [mapping output],
and [rasterization].

## Analogy with `Iterator`

The `Curve` API behaves, in many ways, like a continuous counterpart to [`Iterator`]. The analogy
looks something like this with some of the common methods:

|    Iterators     |     Curves      |
| :--------------- | :-------------- |
| `map`            | `map`           |
| `skip`/`step_by` | `reparametrize` |
| `enumerate`      | `graph`         |
| `chain`          | `chain`         |
| `zip`            | `zip`           |
| `rev`            | `reverse`       |
| `by_ref`         | `by_ref`        |

Of course, there are very important differences, as well. For instance, the continuous nature of
curves means that many iterator methods make little sense in the context of curves, or at least
require numerical techniques. For example, the analogue of `sum` would be an integral, approximated
by something like Riemann summation.

Furthermore, the two also differ greatly in their orientation to borrowing and mutation:
iterators are mutated by being iterated, and by contrast, all curve methods are immutable. More
information on the implications of this can be found [below](self#Ownership-and-borrowing).

## Defining curves

Curves may be defined in a number of ways. The following are common:
- using [functions];
- using [sample interpolation];
- using [splines];
- using [easings].

Among these, the first is the most versatile[^footnote]: the domain and the sampling output are just
specified directly in the construction. For this reason, function curves are a reliable go-to for
simple one-off constructions and procedural uses, where flexibility is desirable. For example:
```rust
# use bevy_math::vec3;
# use bevy_curve::prelude::*;
// A sinusoid:
let sine_curve = FunctionCurve::new(Interval::EVERYWHERE, f32::sin);

## Cargo features (from the manifest)

- `alloc` (enables 2)
- `default` (enables 1)
- `std` (enables 6)
- `serialize` (enables 2)
- `approx` (enables 2): Enable approx for bevy_math types to approximate floating point equality comparisons and assertions
- `bevy_reflect` (enables 3): Enable bevy_reflect (requires alloc)

## Depends on (same ecosystem)

`bevy_math`, `bevy_reflect`

## Public items by file

- `third_party/bevy/crates/bevy_curve/src/adaptors.rs`: struct ConstantCurve, struct FunctionCurve, struct MapCurve, struct ReparamCurve, struct LinearReparamCurve, struct CurveReparamCurve, struct GraphCurve, struct ZipCurve, struct ChainCurve, struct ReverseCurve, struct RepeatCurve, struct ForeverCurve, struct PingPongCurve, struct ContinuationCurve
- `third_party/bevy/crates/bevy_curve/src/cores.rs`: enum InterpolationDatum, struct EvenCore, enum EvenCoreError, fn even_interp, struct UnevenCore, enum UnevenCoreError, struct ChunkedUnevenCore, enum ChunkedUnevenCoreError, fn uneven_interp
- `third_party/bevy/crates/bevy_curve/src/cubic_splines/mod.rs`: struct CubicBezier, struct CubicBezierError, struct CubicHermite, struct CubicCardinalSpline, struct CubicBSpline, enum CubicNurbsError, struct CubicNurbs, struct LinearSpline, struct InsufficientDataError, trait CubicGenerator, trait CyclicCubicGenerator, struct CubicSegment, struct CubicCurve, trait RationalGenerator, struct RationalSegment, struct RationalCurve
- `third_party/bevy/crates/bevy_curve/src/derivatives/mod.rs`: mod adaptor_impls, trait CurveWithDerivative, trait CurveWithTwoDerivatives, trait SampleDerivative, trait SampleTwoDerivatives, struct SampleDerivativeWrapper, struct SampleTwoDerivativesWrapper
- `third_party/bevy/crates/bevy_curve/src/easing.rs`: trait Ease, trait EaseVectorSpace, struct EasingCurve, enum JumpAt, enum EaseFunction, struct LinearCurve, struct QuadraticInCurve, struct QuadraticOutCurve, struct QuadraticInOutCurve, struct CubicInCurve, struct CubicOutCurve, struct CubicInOutCurve, struct QuarticInCurve, struct QuarticOutCurve, struct QuarticInOutCurve, struct QuinticInCurve, struct QuinticOutCurve, struct QuinticInOutCurve, struct SmoothStepInCurve, struct SmoothStepOutCurve, struct SmoothStepCurve, struct SmootherStepInCurve, struct SmootherStepOutCurve, struct SmootherStepCurve, ...
- `third_party/bevy/crates/bevy_curve/src/interval.rs`: struct Interval, struct InvalidIntervalError, struct SpacedPointsError, fn interval
- `third_party/bevy/crates/bevy_curve/src/iterable.rs`: trait IterableCurve
- `third_party/bevy/crates/bevy_curve/src/lib.rs`: mod adaptors, mod cores, mod cubic_splines, mod derivatives, mod easing, mod interval, mod iterable, mod sample_curves, mod prelude
- `third_party/bevy/crates/bevy_curve/src/sample_curves.rs`: struct SampleCurve, struct SampleAutoCurve, struct UnevenSampleCurve, struct UnevenSampleAutoCurve

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
