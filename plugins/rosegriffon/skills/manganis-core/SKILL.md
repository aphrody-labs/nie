---
name: manganis-core
description: "Use when working with the `manganis-core` crate of dioxus: core for manganis."
---

# manganis-core

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/manganis/manganis-core/` (manifest `third_party/dioxus/packages/manganis/manganis-core/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/manganis/manganis-core
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

core for manganis

## Depends on (same ecosystem)

`const-serialize`, `dx-env`

## Public items by file

- `third_party/dioxus/packages/manganis/manganis-core/src/asset.rs`: struct BundledAsset, struct Asset
- `third_party/dioxus/packages/manganis/manganis-core/src/css.rs`: struct CssAssetOptions
- `third_party/dioxus/packages/manganis/manganis-core/src/css_module.rs`: struct CssModuleAssetOptions, fn create_module_hash, fn collect_css_idents
- `third_party/dioxus/packages/manganis/manganis-core/src/css_module_parser.rs`: struct Global, enum CssFragment, fn transform_css, fn get_class_mappings, fn parse_css
- `third_party/dioxus/packages/manganis/manganis-core/src/ffi.rs`: enum SymbolData, enum Platform, struct PlatformFlags, struct PlatformIdentifiers, struct AndroidArtifactMetadata, struct SwiftPackageMetadata
- `third_party/dioxus/packages/manganis/manganis-core/src/folder.rs`: struct FolderAssetOptions
- `third_party/dioxus/packages/manganis/manganis-core/src/images.rs`: enum ImageFormat, enum ImageSize, struct ImageAssetOptions
- `third_party/dioxus/packages/manganis/manganis-core/src/js.rs`: struct JsAssetOptions
- `third_party/dioxus/packages/manganis/manganis-core/src/options.rs`: struct AssetOptions, struct AssetOptionsBuilder, enum AssetVariant

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
