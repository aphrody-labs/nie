---
name: manganis
description: "Use when working with the `manganis` crate of dioxus: Ergonomic, automatic, cross crate asset collection and optimization."
---

# manganis

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/manganis/manganis/` (manifest `third_party/dioxus/packages/manganis/manganis/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/manganis/manganis
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Orientation

Asset bundling for Dioxus: the `asset!` macro (`manganis-macro`) records assets at compile time (`manganis-core` types) and the Dioxus CLI optimises and hashes them. Read `third_party/dioxus/packages/manganis/manganis/README.md` and the `assets/` folder next to it. Game assets of Inazuma Eleven are never bundled this way; they are read from the user's install through the VFS.

## Purpose

Ergonomic, automatic, cross crate asset collection and optimization

## README (`third_party/dioxus/packages/manganis/manganis/README.md`)

# Manganis

Manganis is a tool for submitting assets and native source files to the program linker. It makes it easy to self-host assets and native plugins that are distributed throughout your libraries.

## Assets

Including assets is as simple as using the `asset!()` macro:

```rust
use manganis::{Asset, asset};
const STYLE: Asset = asset!("/assets/style.css");
```

After cargo builds your app, the asset path is embedded directly in the data section. A tool like the Dioxus CLI can extract this metadata and post-process these assets.

```rust, ignore
use manganis::{ImageFormat, Asset, asset, ImageSize, AssetOptions};
// You can collect arbitrary files. Absolute paths are resolved relative to the package root
const _: Asset = asset!("/assets/script.js");

// You can collect images which will be automatically optimized
pub const PNG_ASSET: Asset =
    asset!("/assets/image.png");
// Resize the image at compile time to make the assets smaller
pub const RESIZED_PNG_ASSET: Asset =
    asset!("/assets/image.png", AssetOptions::image().with_size(ImageSize::Manual { width: 52, height: 52 }));
// Or convert the image at compile time to a web friendly format
pub const AVIF_ASSET: Asset = asset!("/assets/image.png", AssetOptions::image().with_format(ImageFormat::Avif));
```

## option_asset

If you have assets that may not always be bundled, you can fall back gracefully with `option_asset!`:

```rust
use manganis::{Asset, asset, option_asset};
const REQUIRED: Asset = asset!("/assets/style.css");
const OPTIONAL: Option = option_asset!("/assets/missing.css");
```

## JavaScript assets

The CLI auto-detects whether each `.js` asset is an ES module (top-level
`import`/`export` or `import.meta`) or a classic script and processes it
accordingly.

- **`with_minify(true)` (default)**: the file is minified in place. Classic
  scripts are minified verbatim with their format preserved (no IIFE
  re-wrapping). ES modules are minified and bundled — local relative imports
  are inlined into a single ESM file, while `http://` / `https://` imports
  remain as external `import` statements for the browser to resolve at runtime.
- **`with_minify(false)`**: the file is copied byte-for-byte. Use this when
  shipping pre-built third-party libraries you do not want re-processed.
- **`with_static_head(true)`**: appends a `` tag to the document head
  pointing at the asset. The CLI emits `` when the
  file is detected (or declared) as an ES module, and a classic ``
  otherwise.
- **`with_preload(true)`**: emits a `` for the asset.
- **`with_module(true)`**: forces the file to be treated as an ES module even
  when auto-detection would say otherwise. Useful when you author a
  side-effect-only module without top-level `import`/`export` declarations.
  Files named `*.mjs` are always treated as modules; files named `*.cjs` are
  always treated as classic.

```rust, ignore
use manganis::{asset, Asset, AssetOptions};

// Vendored UMD library: copy verbatim, emitted as a classic .
const SWEETALERT: Asset = asset!(
    "/assets/sweetalert2.all.min.js",

## Depends on (same ecosystem)

`const-serialize`, `manganis-core`, `manganis-macro`

## Public items by file

- `third_party/dioxus/packages/manganis/manganis/src/android/activity.rs`: fn set_activity, fn with_activity
- `third_party/dioxus/packages/manganis/manganis/src/android/callback.rs`: struct CallbackSystem
- `third_party/dioxus/packages/manganis/manganis/src/android/java.rs`: type JniResult, fn new_string, fn new_object_array, fn set_object_array_element, fn call_static_method, fn call_method, fn find_class, fn new_object, fn check_self_permission, fn request_permissions_via_helper, fn load_class_from_classloader
- `third_party/dioxus/packages/manganis/manganis/src/android/metadata.rs`: type AndroidMetadataBuffer, fn serialize_android_metadata
- `third_party/dioxus/packages/manganis/manganis/src/android/mod.rs`: mod activity, mod callback, mod java, mod metadata, mod macro_helpers
- `third_party/dioxus/packages/manganis/manganis/src/darwin/mod.rs`: mod macro_helpers, mod metadata
- `third_party/dioxus/packages/manganis/manganis/src/lib.rs`: mod macro_helpers, mod android, mod darwin
- `third_party/dioxus/packages/manganis/manganis/src/macro_helpers.rs`: fn copy_bytes, fn serialize_symbol_data, fn serialize_android_artifact, fn serialize_swift_package, fn create_bundled_asset, fn create_bundled_asset_relative, fn serialize_asset, fn deserialize_asset, mod dx_macro_helpers

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
