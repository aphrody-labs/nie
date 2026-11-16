---
name: bevy-diagnostic
description: "Use when working with the `bevy_diagnostic` crate of bevy: Provides diagnostic functionality for Bevy Engine."
---

# bevy_diagnostic

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_diagnostic/` (manifest `third_party/bevy/crates/bevy_diagnostic/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_diagnostic
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides diagnostic functionality for Bevy Engine

## README (`third_party/bevy/crates/bevy_diagnostic/README.md`)

# Bevy Diagnostic

## Cargo features (from the manifest)

- `default` (enables 2)
- `serialize` (enables 4): Adds serialization support through `serde`.
- `dynamic_linking`: Disables diagnostics that are unsupported when Bevy is dynamically linked
- `sysinfo_plugin` (enables 1): Adds integration with `sysinfo`.
- `std` (enables 5): Allows access to the `std` crate. Enabling this feature will prevent compilation on `no_std` targets, but provides access to certain additional features on supported platforms.
- `critical-section` (enables 4): `critical-section` provides the building blocks for synchronization primitives on all platforms, including `no_std`.

## Depends on (same ecosystem)

`bevy_app`, `bevy_ecs`, `bevy_platform`, `bevy_tasks`, `bevy_time`

## Public items by file

- `third_party/bevy/crates/bevy_diagnostic/src/diagnostic.rs`: struct DiagnosticPath, struct DiagnosticMeasurement, struct Diagnostic, struct DiagnosticsStore, struct Diagnostics, trait RegisterDiagnostic
- `third_party/bevy/crates/bevy_diagnostic/src/entity_count_diagnostics_plugin.rs`: struct EntityCountDiagnosticsPlugin
- `third_party/bevy/crates/bevy_diagnostic/src/frame_count.rs`: struct FrameCount, struct FrameCountPlugin, fn update_frame_count
- `third_party/bevy/crates/bevy_diagnostic/src/frame_time_diagnostics_plugin.rs`: struct FrameTimeDiagnosticsPlugin
- `third_party/bevy/crates/bevy_diagnostic/src/lib.rs`: struct DiagnosticsPlugin
- `third_party/bevy/crates/bevy_diagnostic/src/log_diagnostics_plugin.rs`: struct LogDiagnosticsPlugin, struct LogDiagnosticsState
- `third_party/bevy/crates/bevy_diagnostic/src/system_information_diagnostics_plugin.rs`: struct SystemInformationDiagnosticsPlugin, struct SystemInfo

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
