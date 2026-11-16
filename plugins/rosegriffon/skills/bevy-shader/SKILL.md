---
name: bevy-shader
description: "Use when working with the `bevy_shader` crate of bevy: Provides shader asset types and import resolution for Bevy."
---

# bevy_shader

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_shader/` (manifest `third_party/bevy/crates/bevy_shader/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_shader
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides shader asset types and import resolution for Bevy

## Crate docs (`third_party/bevy/crates/bevy_shader/src/lib.rs`)

Provides [`Shader`] assets for Bevy.

## Depends on (same ecosystem)

`bevy_asset`, `bevy_platform`, `bevy_reflect`, `bevy_utils`

## Public items by file

- `third_party/bevy/crates/bevy_shader/src/lib.rs`: mod prelude, mod _macro, macro load_shader_library!
- `third_party/bevy/crates/bevy_shader/src/shader.rs`: enum ValidateShader, struct Shader, enum Source, struct ShaderLoader, enum ShaderLoaderError, struct ShaderSettings, enum ShaderImport, enum ShaderRef
- `third_party/bevy/crates/bevy_shader/src/shader_cache.rs`: enum ShaderCacheSource, type CachedPipelineId, struct ShaderCache, enum ShaderDefVal, struct ShaderResolver, enum ShaderCacheError

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
