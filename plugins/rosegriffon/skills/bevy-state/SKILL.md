---
name: bevy-state
description: "Use when working with the `bevy_state` crate of bevy: Finite state machines for Bevy."
---

# bevy_state

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_state/` (manifest `third_party/bevy/crates/bevy_state/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_state
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Finite state machines for Bevy

## Crate docs (`third_party/bevy/crates/bevy_state/src/lib.rs`)

In Bevy, states are app-wide interdependent, finite state machines that are generally used to model the large scale structure of your program: whether a game is paused, if the player is in combat, if assets are loaded and so on.

This module provides 3 distinct types of state, all of which implement the [`States`](state::States) trait:

- Standard [`States`](state::States) can only be changed by manually setting the [`NextState<S>`](state::NextState) resource.
  These states are the baseline on which the other state types are built, and can be used on
  their own for many simple patterns. See the [states example](https://github.com/bevyengine/bevy/blob/latest/examples/state/states.rs)
  for a simple use case.
- [`SubStates`](state::SubStates) are children of other states - they can be changed manually using [`NextState<S>`](state::NextState),
  but are removed from the [`World`](bevy_ecs::prelude::World) if the source states aren't in the right state. See the [sub_states example](https://github.com/bevyengine/bevy/blob/latest/examples/state/sub_states.rs)
  for a simple use case based on the derive macro, or read the trait docs for more complex scenarios.
- [`ComputedStates`](state::ComputedStates) are fully derived from other states - they provide a [`compute`](state::ComputedStates::compute) method
  that takes in the source states and returns their derived value. They are particularly useful for situations
  where a simplified view of the source states is necessary - such as having an `InAMenu` computed state, derived
  from a source state that defines multiple distinct menus. See the [computed state example](https://github.com/bevyengine/bevy/blob/latest/examples/state/computed_states.rs)
  to see usage samples for these states.

Most of the utilities around state involve running systems during transitions between states, or
determining whether to run certain systems, though they can be used more directly as well. This
makes it easier to transition between menus, add loading screens, pause games, and more.

Specifically, Bevy provides the following utilities:

- 3 Transition Schedules - [`OnEnter<S>`](crate::state::OnEnter), [`OnExit<S>`](crate::state::OnExit) and [`OnTransition<S>`](crate::state::OnTransition) - which are used
  to trigger systems specifically during matching transitions.
- A [`StateTransitionEvent<S>`](crate::state::StateTransitionEvent) that gets fired when a given state changes.
- The [`in_state<S>`](crate::condition::in_state) and [`state_changed<S>`](crate::condition::state_changed) run conditions - which are used
  to determine whether a system should run based on the current state.

Bevy also provides functionality for managing the lifetime of entities in the context of game states, using the [`state_scoped`] module.
Specifically, the marker components [`DespawnOnEnter<S>`](crate::state_scoped::DespawnOnEnter) and [`DespawnOnExit<S>`](crate::state_scoped::DespawnOnExit) are provided for despawning entities on state transition.
This, especially in combination with system scheduling, enables a flexible and expressive way to manage spawning and despawning entities.

## Cargo features (from the manifest)

- `default` (enables 3)
- `bevy_reflect` (enables 3): Adds runtime reflection support using `bevy_reflect`.
- `bevy_app` (enables 1): Adds integration with the `bevy_app` plugin API.
- `std` (enables 4): Allows access to the `std` crate. Enabling this feature will prevent compilation on `no_std` targets, but provides access to certain additional features on supported platforms.
- `critical-section` (enables 4): `critical-section` provides the building blocks for synchronization primitives on all platforms, including `no_std`.

## Depends on (same ecosystem)

`bevy_app`, `bevy_ecs`, `bevy_platform`, `bevy_reflect`, `bevy_state_macros`, `bevy_utils`

## Public items by file

- `third_party/bevy/crates/bevy_state/src/app.rs`: trait AppExtStates, struct StatesPlugin
- `third_party/bevy/crates/bevy_state/src/commands.rs`: trait CommandsStatesExt
- `third_party/bevy/crates/bevy_state/src/condition.rs`: fn state_exists, fn in_state, fn state_changed
- `third_party/bevy/crates/bevy_state/src/lib.rs`: mod app, mod commands, mod condition, mod state, mod state_scoped, mod state_scoped_events, mod reflect, mod prelude
- `third_party/bevy/crates/bevy_state/src/reflect.rs`: struct ReflectState, struct ReflectStateFns, struct ReflectFreelyMutableState, struct ReflectFreelyMutableStateFns
- `third_party/bevy/crates/bevy_state/src/state/computed_states.rs`: trait ComputedStates
- `third_party/bevy/crates/bevy_state/src/state/freely_mutable_state.rs`: trait FreelyMutableState
- `third_party/bevy/crates/bevy_state/src/state/resources.rs`: struct State, struct PreviousState, enum NextState
- `third_party/bevy/crates/bevy_state/src/state/state_set.rs`: trait StateSet
- `third_party/bevy/crates/bevy_state/src/state/states.rs`: trait States
- `third_party/bevy/crates/bevy_state/src/state/sub_states.rs`: trait SubStates
- `third_party/bevy/crates/bevy_state/src/state/transitions.rs`: struct OnEnter, struct OnExit, struct OnTransition, struct StateTransition, struct StateTransitionEvent, enum StateTransitionSystems, struct ExitSchedules, struct TransitionSchedules, struct EnterSchedules, fn setup_state_transitions_in_world, fn last_transition
- `third_party/bevy/crates/bevy_state/src/state_scoped.rs`: struct DespawnWhen, fn despawn_entities_when_state, struct DespawnOnExit, fn despawn_entities_on_exit_state, struct DespawnOnEnter, fn despawn_entities_on_enter_state, struct DisableWhen, fn disable_entities_when_state, struct DisableOnExit, fn disable_entities_on_exit_state, struct DisableOnEnter, fn disable_entities_on_enter_state, struct EnableWhen, fn enable_entities_when_state, struct EnableOnExit, fn enable_entities_on_exit_state, struct EnableOnEnter, fn enable_entities_on_enter_state
- `third_party/bevy/crates/bevy_state/src/state_scoped_events.rs`: trait StateScopedMessagesAppExt

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
