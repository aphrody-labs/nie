---
name: bevy-tasks
description: "Use when working with the `bevy_tasks` crate of bevy: A task executor for Bevy Engine."
---

# bevy_tasks

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_tasks/` (manifest `third_party/bevy/crates/bevy_tasks/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_tasks
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

A task executor for Bevy Engine

## README (`third_party/bevy/crates/bevy_tasks/README.md`)

# Bevy Tasks


A refreshingly simple task executor for bevy. :)

This is a simple threadpool with minimal dependencies. The main usecase is a scoped fork-join, i.e. spawning tasks from
a single thread and having that thread await the completion of those tasks. This is intended specifically for
[`bevy`][bevy] as a lighter alternative to [`rayon`][rayon] for this specific usecase. There are also utilities for
generating the tasks from a slice of data. This library is intended for games and makes no attempt to ensure fairness
or ordering of spawned tasks.

It is based on [`async-executor`][async-executor], a lightweight executor that allows the end user to manage their own threads.
`async-executor` is based on async-task, a core piece of async-std.

## Usage

In order to be able to optimize task execution in multi-threaded environments,
bevy provides three different thread pools via which tasks of different kinds can be spawned.
(The same API is used in single-threaded environments, even if execution is limited to a single thread.
This currently applies to Wasm targets.)
The determining factor for what kind of work should go in each pool is latency requirements:

* For CPU-intensive work (tasks that generally spin until completion) we have a standard
  [`ComputeTaskPool`] and an [`AsyncComputeTaskPool`]. Work that does not need to be completed to
  present the next frame should go to the [`AsyncComputeTaskPool`].

* For IO-intensive work (tasks that spend very little time in a "woken" state) we have an
  [`IoTaskPool`] whose tasks are expected to complete very quickly. Generally speaking, they should just
  await receiving data from somewhere (i.e. disk) and signal other systems when the data is ready
  for consumption. (likely via channels)

## `no_std` Support

To enable `no_std` support in this crate, you will need to disable default features, and enable the `edge_executor` and `critical-section` features.

## Cargo features (from the manifest)

- `default` (enables 2)
- `multi_threaded` (enables 4): Enables multi-threading support. Without this feature, all tasks will be run on a single thread.
- `async_executor` (enables 3): Uses `async-executor` as a task execution backend. This backend is incompatible with `no_std` targets.
- `futures-lite` (enables 1): Provide an implementation of `block_on` from  `futures-lite`.
- `async-io` (enables 1): Use async-io's implementation of block_on instead of futures-lite's implementation. This is preferred if your application uses async-io.

## Depends on (same ecosystem)

`bevy_platform`

## Public items by file

- `third_party/bevy/crates/bevy_tasks/src/edge_executor.rs`: struct Executor, struct LocalExecutor
- `third_party/bevy/crates/bevy_tasks/src/executor.rs`: struct Executor, struct LocalExecutor
- `third_party/bevy/crates/bevy_tasks/src/futures.rs`: fn now_or_never, fn check_ready
- `third_party/bevy/crates/bevy_tasks/src/iter/adapters.rs`: struct Chain, struct Map, struct Filter, struct FilterMap, struct FlatMap, struct Flatten, struct Fuse, struct Inspect, struct Copied, struct Cloned, struct Cycle
- `third_party/bevy/crates/bevy_tasks/src/iter/mod.rs`: trait ParallelIterator
- `third_party/bevy/crates/bevy_tasks/src/lib.rs`: mod cfg, trait ConditionalSendFuture, type BoxedFuture, mod futures, mod prelude, fn available_parallelism
- `third_party/bevy/crates/bevy_tasks/src/single_threaded_task_pool.rs`: struct TaskPoolBuilder, struct ThreadExecutor, struct TaskPool, struct Scope
- `third_party/bevy/crates/bevy_tasks/src/slice.rs`: trait ParallelSlice, trait ParallelSliceMut
- `third_party/bevy/crates/bevy_tasks/src/task_pool.rs`: struct TaskPoolBuilder, struct TaskPool, struct Scope
- `third_party/bevy/crates/bevy_tasks/src/thread_executor.rs`: struct ThreadExecutor, struct ThreadExecutorTicker

## Examples in the source tree

- `third_party/bevy/crates/bevy_tasks/examples/busy_behavior.rs`
- `third_party/bevy/crates/bevy_tasks/examples/idle_behavior.rs`

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
