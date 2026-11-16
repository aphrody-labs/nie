---
name: blitz-examples
description: "Use when working with the `blitz-examples` crate of blitz: Top level crate for Blitz."
---

# blitz-examples

- Version: 0.0.1 (workspace member of `third_party/blitz`, vendored commit `a84a3c51b562`)
- Source: `third_party/blitz/` (manifest `third_party/blitz/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/blitz/tree/main/
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Top level crate for Blitz

## README (`third_party/blitz/README.md`)

**A modular HTML/CSS rendering engine**

Talk to us in: the [#native](https://discord.gg/AnNPqT95pu) channel in the [Dioxus Discord](https://discord.gg/AnNPqT95pu)

## Status

Blitz is currently in a **beta** state. It can already render many popular no-JS websites (Wikipedia, (Old) Reddit, etc), and is usable for making apps if you are an early adopter and willing to live on the bleeding edge. But there are also still many bugs and missing features. We are actively working on bringing it up to production quality.

Check out:
- The [downloads page](https://blitz.is/downloads) for runnable builds of the Browser UI (Windows / macOS / Linux / Android)
- The [status page](https://blitz.is/status/css) for current status
- The [roadmap issue](https://github.com/DioxusLabs/blitz/issues/119) for planned work

## Trying it out

1. Clone this repo
2. Run one of our examples:
    - The Browser UI: `cargo run -rp browser`
    - The Markdown viewer: `cargo run -rp rdme ./README.md`
    - TodoMVC app: `cargo run -rp todomvc`
    - WGPU texture integration example: `cargo run -rp wgpu_texture`

Other examples are available in the [examples/](./examples/) folder.

## Goals

Blitz is designed to render HTML and CSS - we *don't* want to support the entirety of browser features (or at least we want to make all such "extra" features opt-in). In our opinion, the browser is bloated for the basic use case of rendering HTML/CSS.

We do intend to support:

- Modern HTML layout (flexbox, grid, table, block, inline, absolute/fixed, etc).
- Advanced CSS (complex selectors, media queries, css variables)
- HTML Form controls
- Accessibility using AccessKit
- Extensibility via custom widgets

Notably we *don't* provide features like webrtc, websockets, bluetooth, localstorage, etc. In a native app, much of this functionality can be fulfilled using regular Rust crates and doesn't need to be coupled with the renderer.

We don't yet have Blitz bindings for other languages (JavaScript, Python, etc) but would accept contributions along those lines.

## Architecture

Blitz consists of a core DOM abstraction; several modular pieces which provide additional functionality like networking, rendering, windows, and state management; and two high-level wrappers that support rendering either a Dioxus application or HTML with a simplified API.

These pieces can be combined together to make a cohesive web engine.

### High-level "wrapper" crates

- **`blitz`** - An HTML/markdown frontend that can render an HTML string. This is useful for previewing HTML and/or markdown files but currently lacks interactivity.
Uses: `blitz-dom`, `blitz-html`, `blitz-shell`, `blitz-renderer-vello`
- **`dioxus-native`** - A Dioxus frontend that can render a Dioxus VirtualDom. This has full interactivity support via Dioxus's event handling.
Uses: `blitz-dom`, `dioxus-core`, `blitz-shell`, `blitz-renderer-vello`

Both wrappers can optionally use `blitz-net` to fetch sub-resources.

### Using the git version of Dioxus Native

The latest development version of the Dioxus Native lives in this repository. As Dioxus Native is under rapid development it can be useful to use this version to get access to the latest features and bug fixes sooner than they are available in an official release.

To use the git version of `dioxus-native`:

- Remove your dependency on the `dioxus` crate entirely.
- Add `dioxus-native = { git = "https://github.com/DioxusLabs/blitz", rev = "e64a3d8", features = ["prelude"] }`
- (replace `e64a3d8` with the git commit id of the version you want to use)
- In your rust code change all instances of `use dioxus::prelude::*` to `use dioxus_native::prelude::*`.
- If you need to access additional functionality from the `dioxus` crate that is not exported from the Dioxus Native prelude then you can import it from the individual sub-crates (`dioxus-html`, `dioxus-signals`, `dioxus-router`, etc) instead.

The git versions of Dioxus Native currently depend on Dioxus `0.8.0-alpha.3` from crates.io (commits before `286874c` use the stable v0.7.x version), so any additional libraries that you are using (`dioxus-sdk`, `dioxus-components`, `dioxus-free-icons`, etc) need to be versions that work with Dioxus `0.8.0-alpha.3`.

### Modular Components

## Cargo features (from the manifest)

- `log-times` (enables 2)
- `log-frame-times` (enables 1)
- `log-phase-times` (enables 1)

## Examples in the source tree

- `third_party/blitz/examples/box_shadow.rs`
- `third_party/blitz/examples/counter/src/app.rs`
- `third_party/blitz/examples/counter/src/lib.rs`
- `third_party/blitz/examples/counter/src/main.rs`
- `third_party/blitz/examples/custom_widget.rs`
- `third_party/blitz/examples/flex.rs`
- `third_party/blitz/examples/form.rs`
- `third_party/blitz/examples/gradient.rs`

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
