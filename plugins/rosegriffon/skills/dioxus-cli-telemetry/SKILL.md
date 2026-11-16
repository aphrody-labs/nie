---
name: dioxus-cli-telemetry
description: "Use when working with the `dioxus-cli-telemetry` crate of dioxus: Wire format for the dioxus CLI telemetry type."
---

# dioxus-cli-telemetry

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/cli-telemetry/` (manifest `third_party/dioxus/packages/cli-telemetry/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/cli-telemetry
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Wire format for the dioxus CLI telemetry type

## Crate docs (`third_party/dioxus/packages/cli-telemetry/src/lib.rs`)

# Telemetry for the Dioxus CLI

Dioxus uses telemetry in the CLI to get insight into metrics like performance, panics, and usage
of various arguments. This data helps us track down bugs and improve quality of the tooling.

Usage of telemetry in open source products can be controversial. Our goal here is to collect
minimally invasive data used exclusively to improve our tooling. Github issues only show *some*
of the problem, but many users stumble into issues which go unreported.

Our policy follows:
- minimally invasive
- anonymous
- periodic
- transparent
- easy to disable

We send a heartbeat when the CLI is executed and then rollups of logs over time.
- Heartbeat: helps us track version distribution of the CLI and critical "failures on launch" useful during new version rollouts.
- Rollups: helps us track performance and issues over time, as well as usage of various commands.

Rollups are not done in background processes, but rather directly by the `dx` CLI.
If you don't run the CLI, then we won't send any data.

We don't collect any PII, but we do collect three "controversial" pieces of data:
- the target triple of your system (OS, arch, etc)
- a session ID which is a random number generated on each run
- a distinct ID per `.dx` installation which is a random number generated on initial run.

The distinct ID is used to track the same installation over time, but it is not tied to any user
account or PII. Since `dx` doesn't have any accounts or authentication mechanism, this ID is used
as a "best effort" identifier. If you still want to participate in telemetry but don't want a
distinct ID, you can replace the stable_id.json file in the `.dx` directory with an empty string.

In the CLI, you can disable this by using any of the methods:
- installing with the "disable-telemetry" feature flag
- setting TELEMETRY=false in your env
- setting `dx config set disable-telemetry true`

## Public items by file

- `third_party/dioxus/packages/cli-telemetry/src/lib.rs`: struct TelemetryEventData, struct StackFrame, fn strip_paths

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
