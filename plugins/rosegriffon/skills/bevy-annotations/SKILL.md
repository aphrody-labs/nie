---
name: bevy-annotations
description: "Use when working with the `bevy_annotations` crate of bevy: Well-known reflection annotations for Bevy tools such as inspectors and editors."
---

# bevy_annotations

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_annotations/` (manifest `third_party/bevy/crates/bevy_annotations/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_annotations
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Well-known reflection annotations for Bevy tools such as inspectors and editors

## Crate docs (`third_party/bevy/crates/bevy_annotations/src/lib.rs`)

Standard ("well-known") reflection annotations for tools such as inspectors and editors.

The mechanism lives in `bevy_reflect`: custom attributes are attached with
`#[reflect(@...)]` on types, fields and variants, and read back with
`get_attribute::<T>()`. This crate only defines a shared vocabulary of
attribute types, so that tools can agree on what to look for.

The types have no dependencies, and `bevy_reflect` implements `Reflect` for them.
This lets the reflection impls in `bevy_reflect` itself use them.

Several annotations can be attached to the same item.

```ignore
use bevy_annotations::{DisplayName, ValueRange, ReadOnly, Step, Tooltip, Unit};
use bevy_reflect::{Reflect, TypeInfo, Typed};

#[derive(Reflect)]
#[reflect(@Tooltip("Movement settings for a character"))]
struct Movement {
    #[reflect(@ValueRange(0.0..=20.0), @Step(0.5), @Unit("m/s"))]
    speed: f32,
    #[reflect(@DisplayName("Jump Height"), @Tooltip("Peak height of a jump"))]
    jump: f32,
    #[reflect(@ReadOnly)]
    grounded: bool,
}

let TypeInfo::Struct(info) = Movement::type_info() else {
    panic!("expected struct info");
};

let speed = info.field("speed").unwrap();
assert_eq!(speed.get_attribute::<ValueRange>(), Some(&ValueRange(0.0..=20.0)));
assert_eq!(speed.get_attribute::<Step>(), Some(&Step(0.5)));
assert_eq!(speed.get_attribute::<Unit>(), Some(&Unit("m/s")));

let jump = info.field("jump").unwrap();
assert_eq!(jump.get_attribute::<DisplayName>(), Some(&DisplayName("Jump Height")));

assert!(info.field("grounded").unwrap().has_attribute::<ReadOnly>());
assert!(info.has_attribute::<Tooltip>());
```

## Public items by file

- `third_party/bevy/crates/bevy_annotations/src/lib.rs`: struct ValueRange, struct Step, struct Unit, struct ReadOnly, struct Tooltip, struct DisplayName

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
