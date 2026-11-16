---
name: bevy-time
description: "Use when working with the `bevy_time` crate of bevy: Provides time functionality for Bevy Engine."
---

# bevy_time

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_time/` (manifest `third_party/bevy/crates/bevy_time/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_time
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides time functionality for Bevy Engine

## README (`third_party/bevy/crates/bevy_time/README.md`)

# Bevy Time


The built-in timekeeping plugin for the Bevy game engine.

## Cargo features (from the manifest)

- `default` (enables 3)
- `bevy_reflect` (enables 3): Adds runtime reflection support using `bevy_reflect`.
- `serialize` (enables 3): Adds serialization support through `serde`.
- `std` (enables 6): Allows access to the `std` crate. Enabling this feature will prevent compilation on `no_std` targets, but provides access to certain additional features on supported platforms.
- `critical-section` (enables 4): `critical-section` provides the building blocks for synchronization primitives on all platforms, including `no_std`.

## Depends on (same ecosystem)

`bevy_app`, `bevy_ecs`, `bevy_platform`, `bevy_reflect`

## Public items by file

- `third_party/bevy/crates/bevy_time/src/common_conditions.rs`: fn on_timer, fn on_real_timer, fn once_after_delay, fn once_after_real_delay, fn repeating_after_delay, fn repeating_after_real_delay, fn paused
- `third_party/bevy/crates/bevy_time/src/delayed_commands.rs`: struct DelayedCommands, trait DelayedCommandsExt, struct DelayedCommandQueue, fn check_delayed_command_queues, fn silence_delayed_command_queues_on_exit
- `third_party/bevy/crates/bevy_time/src/fixed.rs`: struct Fixed, fn run_fixed_main_schedule
- `third_party/bevy/crates/bevy_time/src/lib.rs`: mod common_conditions, mod prelude, struct TimePlugin, struct TimeSystems, enum TimeUpdateStrategy, struct TimeReceiver, struct TimeSender, fn create_time_channels, fn time_system
- `third_party/bevy/crates/bevy_time/src/moment.rs`: struct Moment
- `third_party/bevy/crates/bevy_time/src/real.rs`: struct Real
- `third_party/bevy/crates/bevy_time/src/stopwatch.rs`: struct Stopwatch
- `third_party/bevy/crates/bevy_time/src/time.rs`: struct Time
- `third_party/bevy/crates/bevy_time/src/timer.rs`: struct Timer, enum TimerMode
- `third_party/bevy/crates/bevy_time/src/virt.rs`: struct Virtual, fn update_virtual_time

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
