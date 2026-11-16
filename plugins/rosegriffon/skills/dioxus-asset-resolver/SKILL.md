---
name: dioxus-asset-resolver
description: "Use when working with the `dioxus-asset-resolver` crate of dioxus: Cross-platform asset resolver for manganis and dioxus."
---

# dioxus-asset-resolver

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/asset-resolver/` (manifest `third_party/dioxus/packages/asset-resolver/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/asset-resolver
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Cross-platform asset resolver for manganis and dioxus

## Crate docs (`third_party/dioxus/packages/asset-resolver/src/lib.rs`)

The asset resolver for the Dioxus bundle format. Each platform has its own way of resolving assets. This crate handles
resolving assets in a cross-platform way.

There are two broad locations for assets depending on the platform:
- **Web**: Assets are stored on a remote server and fetched via HTTP requests.
- **Native**: Assets are read from the local bundle. Each platform has its own bundle structure which may store assets
  as a file at a specific path or in an opaque format like Android's AssetManager.

[`read_asset_bytes`]( abstracts over both of these methods, allowing you to read the bytes of an asset
regardless of the platform.

If you know you are on a desktop platform, you can use [`asset_path`] to resolve the path of an asset and read
the contents with [`std::fs`].

## Example
```rust
# async fn asset_example() {
use dioxus::prelude::*;

// Bundle the static JSON asset into the application
static JSON_ASSET: Asset = asset!("/assets/data.json");

// Read the bytes of the JSON asset
let bytes = dioxus::asset_resolver::read_asset_bytes(&JSON_ASSET).await.unwrap();

// Deserialize the JSON data
let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
assert_eq!(json["key"].as_str(), Some("value"));
# }
```

## Depends on (same ecosystem)

`dx-env`

## Public items by file

- `third_party/dioxus/packages/asset-resolver/src/lib.rs`: mod native, enum AssetPathError, fn asset_path, enum AssetResolveError, fn read_asset_bytes, enum NativeAssetResolveError, struct WebAssetResolveError
- `third_party/dioxus/packages/asset-resolver/src/native.rs`: enum AssetServeError, fn serve_asset, fn get_mime_from_ext

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
