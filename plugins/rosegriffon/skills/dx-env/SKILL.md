---
name: dx-env
description: "Use when working with the `dx-env` crate of dioxus: Environment variables that dx sets for the apps it launches."
---

# dx-env

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/dx-env/` (manifest `third_party/dioxus/packages/dx-env/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/dx-env
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Environment variables that dx sets for the apps it launches

## README (`third_party/dioxus/packages/dx-env/README.md`)

# dx-env

A crate that provides the environment variable names and accessors that `dx` uses to configure the applications it launches at runtime.

Dioxus-specific settings such as the window title live in `dioxus-cli-config`, which re-exports this crate.

This crate exists for us to very cleanly define the exact fields we want to pass down to Dioxus applications at runtime but without exposing the entire config object.

This leads to faster compile times, smaller binaries, and a clearer distinction between the config and the application.

## Public items by file

- `third_party/dioxus/packages/dx-env/src/lib.rs`: fn legacy_env_name, fn devserver_raw_addr, fn devserver_ws_endpoint, fn server_ip, fn server_port, fn fullstack_address_or_localhost, fn is_cli_enabled, fn base_path, fn web_base_path, fn format_base_path_meta_element, fn out_dir, fn session_cache_dir, fn android_session_cache_dir, fn build_id, fn product_name, fn is_bundled_app

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
