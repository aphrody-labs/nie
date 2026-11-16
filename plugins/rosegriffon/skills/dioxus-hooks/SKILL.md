---
name: dioxus-hooks
description: "Use when working with the `dioxus-hooks` crate of dioxus: Basic useful hooks for Dioxus."
---

# dioxus-hooks

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/hooks/` (manifest `third_party/dioxus/packages/hooks/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/hooks
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Basic useful hooks for Dioxus.

## README (`third_party/dioxus/packages/hooks/README.md`)

# Dioxus Hooks

[Website](https://dioxuslabs.com) |
[Guides](https://dioxuslabs.com/learn/0.7/) |
[API Docs](https://docs.rs/dioxus-hooks/latest/dioxus_hooks) |
[Chat](https://discord.gg/XgGxMSkvUM)

## Overview

`dioxus-hooks` includes some basic useful hooks for Dioxus such as:

- use_signal
- use_effect
- use_resource
- use_memo
- use_coroutine

Unlike React, none of these hooks are foundational since they all build off the primitive `use_hook`. You can extend these hooks with [custom hooks](https://dioxuslabs.com/learn/0.7/essentials/advanced/custom_hooks) in your own code. If you think they would be useful for the broader community, you can open a PR to add your hook to the [Dioxus Awesome](https://github.com/DioxusLabs/awesome-dioxus) list.

## State Cheat Sheet

If you aren't sure what hook to use, you can use this cheat sheet to help you decide:

### State Location

Depending on where you need to access the state, you can put your state in one of three places:

| Location                                                                                 | Where can you access the state? | Recommended for Libraries? | Examples                                                                    |
| ---------------------------------------------------------------------------------------- | ------------------------------- | -------------------------- | --------------------------------------------------------------------------- |
| [Hooks](https://docs.rs/dioxus-hooks/latest/dioxus_hooks/fn.use_signal.html)             | Any components you pass it to   | ✅                          | `use_signal(\|\| 0)`, `use_memo(\|\| state() * 2)`                          |
| [Context](https://docs.rs/dioxus-hooks/latest/dioxus_hooks/fn.use_context_provider.html) | Any child components            | ✅                          | `use_context_provider(\|\| Signal::new(0))`, `use_context::>()` |
| [Global](https://docs.rs/dioxus/latest/dioxus/prelude/struct.Signal.html#method.global)  | Anything in your app            | ❌                          | `Signal::global(\|\| 0)`                                                    |

### Derived State

If you don't have an initial value for your state, you can derive your state from other states with a closure or asynchronous function:

| Hook                                                                                | Reactive (reruns when dependencies change) | Async | Memorizes Output | Example                                                                             |
| ----------------------------------------------------------------------------------- | ------------------------------------------ | ----- | ---------------- | ----------------------------------------------------------------------------------- |
| [`use_memo`](https://docs.rs/dioxus/latest/dioxus/prelude/fn.use_memo.html)         | ✅                                          | ❌     | ✅                | `use_memo(move \|\| count() * 2)`                                                   |
| [`use_resource`](https://docs.rs/dioxus/latest/dioxus/prelude/fn.use_resource.html) | ✅                                          | ✅     | ❌                | `use_resource(move \|\| reqwest::get(format!("/users/{user_id}")))`                 |
| [`use_future`](https://docs.rs/dioxus/latest/dioxus/prelude/fn.use_future.html)     | ❌                                          | ✅     | ❌                | `use_future(move \|\| println!("{:?}", reqwest::get(format!("/users/{user_id}"))))` |

### Persistent State

The core hooks library doesn't provide hooks for persistent state, but you can extend the core hooks with hooks from [dioxus-sdk](https://crates.io/crates/dioxus-sdk) and the [dioxus-router](https://crates.io/crates/dioxus-router) to provide persistent state management.

| State                                                                              | Sharable | Example                                                                                           |
| ---------------------------------------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------- |
| [`use_persistent`](https://github.com/DioxusLabs/sdk/tree/master/examples/storage) | ❌        | `use_persistent("unique_key", move \|\| initial_state)`                                           |
| [`Router {}`](https://dioxuslabs.com/learn/0.7/essentials/router/)          | ✅        | `#[derive(Routable, Clone, PartialEq)] enum Route { #[route("/user/:id")] Homepage { id: u32 } }` |

## Contributing

- Report issues on our [issue tracker](https://github.com/dioxuslabs/dioxus/issues).
- Join the discord and ask questions!

## License

This project is licensed under the [MIT license].

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Dioxus by you shall be licensed as MIT without any additional
terms or conditions.

## Cargo features (from the manifest)

- `default`
- `nightly-features`

## Depends on (same ecosystem)

`dioxus-signals`, `dioxus-vdom`, `generational-box`

## Public items by file

- `third_party/dioxus/packages/hooks/src/use_action.rs`: fn use_action, struct Action, struct Dispatching, trait ActionCallback
- `third_party/dioxus/packages/hooks/src/use_after_suspense_resolved.rs`: fn use_after_suspense_resolved
- `third_party/dioxus/packages/hooks/src/use_callback.rs`: fn use_callback
- `third_party/dioxus/packages/hooks/src/use_context.rs`: fn try_use_context, fn use_context, fn use_context_provider
- `third_party/dioxus/packages/hooks/src/use_coroutine.rs`: fn use_coroutine, fn use_coroutine_handle, struct Coroutine
- `third_party/dioxus/packages/hooks/src/use_effect.rs`: fn use_effect, struct Effect
- `third_party/dioxus/packages/hooks/src/use_future.rs`: fn use_future, struct UseFuture, enum UseFutureState
- `third_party/dioxus/packages/hooks/src/use_hook_did_run.rs`: fn use_hook_did_run
- `third_party/dioxus/packages/hooks/src/use_memo.rs`: fn use_memo
- `third_party/dioxus/packages/hooks/src/use_on_destroy.rs`: fn use_on_unmount
- `third_party/dioxus/packages/hooks/src/use_reactive.rs`: trait Dependency, trait DependencyElement, fn use_reactive, macro use_reactive!
- `third_party/dioxus/packages/hooks/src/use_resource.rs`: fn use_resource, struct Resource, enum UseResourceState
- `third_party/dioxus/packages/hooks/src/use_root_context.rs`: fn use_root_context
- `third_party/dioxus/packages/hooks/src/use_set_compare.rs`: fn use_set_compare, fn use_set_compare_equal
- `third_party/dioxus/packages/hooks/src/use_signal.rs`: fn use_signal, fn use_signal_sync
- `third_party/dioxus/packages/hooks/src/use_sorted.rs`: fn use_sorted
- `third_party/dioxus/packages/hooks/src/use_waker.rs`: fn use_waker, struct UseWaker

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
