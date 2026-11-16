---
name: blitz-shell
description: "Use when working with the `blitz-shell` crate of blitz: Blitz application shell."
---

# blitz-shell

- Version: 0.3.0-beta.4 (workspace member of `third_party/blitz`, vendored commit `a84a3c51b562`)
- Source: `third_party/blitz/packages/blitz-shell/` (manifest `third_party/blitz/packages/blitz-shell/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/blitz/tree/main/packages/blitz-shell
- License: [object Object]
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Blitz application shell

## Crate docs (`third_party/blitz/packages/blitz-shell/src/lib.rs`)

Event loop, windowing and system integration.

## Feature flags
 - `default`: Enables the features listed below.
 - `accessibility`: Enables [`accesskit`] accessibility support.
 - `hot-reload`: Enables hot-reloading of Dioxus RSX.
 - `tracing`: Enables tracing support.

## Cargo features (from the manifest)

- `default` (enables 2)
- `accessibility` (enables 3)
- `tracing` (enables 2)
- `file-dialog` (enables 1)
- `data-uri` (enables 1): Enables a data-uri-only NetProvider. Only needed if you aren't using the regular NetProvider

## Depends on (same ecosystem)

`accesskit_xplat`, `blitz-dom`, `blitz-paint`, `blitz-traits`, `blitz-vibey-cdp-server`

## Public items by file

- `third_party/blitz/packages/blitz-shell/src/accessibility.rs`: struct AccessibilityState
- `third_party/blitz/packages/blitz-shell/src/application.rs`: struct BlitzApplication
- `third_party/blitz/packages/blitz-shell/src/event.rs`: enum BlitzShellEvent, struct BlitzShellProxy, struct BlitzShellProxyInner, fn create_waker
- `third_party/blitz/packages/blitz-shell/src/lib.rs`: struct Config, fn create_default_event_loop, fn set_android_app, fn current_android_app, struct BlitzShellProvider
- `third_party/blitz/packages/blitz-shell/src/window.rs`: struct WindowConfig, struct View

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
