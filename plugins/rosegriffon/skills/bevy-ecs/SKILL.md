---
name: bevy-ecs
description: "Use when working with the `bevy_ecs` crate of bevy: Bevy Engine's entity component system."
---

# bevy_ecs

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_ecs/` (manifest `third_party/bevy/crates/bevy_ecs/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_ecs
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Bevy Engine's entity component system

## README (`third_party/bevy/crates/bevy_ecs/README.md`)

# Bevy ECS


## What is Bevy ECS?

Bevy ECS is an Entity Component System custom-built for the [Bevy][bevy] game engine.
It aims to be simple to use, ergonomic, fast, massively parallel, opinionated, and featureful.
It was created specifically for Bevy's needs, but it can easily be used as a standalone crate in other projects.

## ECS

All app logic in Bevy uses the Entity Component System paradigm, which is often shortened to ECS. ECS is a software pattern that involves breaking your program up into Entities, Components, and Systems. Entities are unique "things" that are assigned groups of Components, which are then processed using Systems.

For example, one entity might have a `Position` and `Velocity` component, whereas another entity might have a `Position` and `UI` component. You might have a movement system that runs on all entities with a Position and Velocity component.

The ECS pattern encourages clean, decoupled designs by forcing you to break up your app data and logic into its core components. It also helps make your code faster by optimizing memory access patterns and making parallelism easier.

## Concepts

Bevy ECS is Bevy's implementation of the ECS pattern. Unlike other Rust ECS implementations, which often require complex lifetimes, traits, builder patterns, or macros, Bevy ECS uses normal Rust data types for all of these concepts:

### Components

Components are normal Rust structs. They are data stored in a `World` and specific instances of Components correlate to Entities.

```rust
use bevy_ecs::prelude::*;

#[derive(Component)]
struct Position { x: f32, y: f32 }
```

### Worlds

Entities, Components, and Resources are stored in a `World`. Worlds, much like `std::collections`'s `HashSet` and `Vec`, expose operations to insert, read, write, and remove the data they store.

```rust
use bevy_ecs::world::World;

let world = World::default();
```

### Entities

Entities are unique identifiers that correlate to zero or more Components.

```rust
use bevy_ecs::prelude::*;

#[derive(Component)]
struct Position { x: f32, y: f32 }
#[derive(Component)]
struct Velocity { x: f32, y: f32 }

let mut world = World::new();

let entity = world
    .spawn((Position { x: 0.0, y: 0.0 }, Velocity { x: 1.0, y: 0.0 }))
    .id();

let entity_ref = world.entity(entity);
let position = entity_ref.get::().unwrap();
let velocity = entity_ref.get::().unwrap();
```

### Systems

Systems are normal Rust functions. Thanks to the Rust type system, Bevy ECS can use function parameter types to determine what data needs to be sent to the system. It also uses this "data access" information to determine what Systems can run in parallel with each other.

```rust

## Cargo features (from the manifest)

- `default` (enables 4)
- `multi_threaded` (enables 1): Enables multithreading support. Schedules will attempt to run systems on multiple threads whenever possible.
- `serialize` (enables 3): Adds serialization support through `serde`.
- `bevy_reflect` (enables 1): Adds runtime reflection support using `bevy_reflect`.
- `reflect_functions` (enables 2): Extends reflection support to functions.
- `reflect_auto_register` (enables 2)
- `backtrace` (enables 1): Enables automatic backtrace capturing in BevyError
- `trace` (enables 2): Enables `tracing` integration, allowing spans and other metrics to be reported through that framework.
- `debug` (enables 3): Enable collecting debug information about systems and components to help with diagnostics
- `detailed_trace` (enables 1): Enables a more detailed set of traces which may be noisy if left on by default.
- `bevy_debug_stepping`: Provides system stepping support, allowing them to be paused, stepped, and other debug operations which can help with diagnosing certain behaviors.
- `track_location`: Provides more detailed tracking of the cause of various effects within the ECS. This will often provide more detailed error messages.
- `async_executor` (enables 2): Uses `async-executor` as a task execution backend. This backend is incompatible with `no_std` targets.
- `std` (enables 12): Allows access to the `std` crate. Enabling this feature will prevent compilation on `no_std` targets, but provides access to certain additional features on supported platforms.
- `critical-section` (enables 2): `critical-section` provides the building blocks for synchronization primitives on all platforms, including `no_std`.
- `hotpatching` (enables 1)

## Depends on (same ecosystem)

`bevy_ecs_macros`, `bevy_platform`, `bevy_ptr`, `bevy_reflect`, `bevy_tasks`, `bevy_utils`, `subsecond`

## Public items by file

- `third_party/bevy/crates/bevy_ecs/src/archetype.rs`: struct ArchetypeRow, struct ArchetypeId, struct Edges, struct ArchetypeEntity, struct Archetype, struct ArchetypeGeneration, type ComponentIndex, struct Archetypes, struct ArchetypeRecord
- `third_party/bevy/crates/bevy_ecs/src/batching.rs`: struct BatchingStrategy
- `third_party/bevy/crates/bevy_ecs/src/bundle/info.rs`: struct BundleId, enum InsertMode, struct BundleInfo, struct Bundles
- `third_party/bevy/crates/bevy_ecs/src/bundle/mod.rs`: trait Bundle, trait BundleFromComponents, trait DynamicBundle, trait NoBundleEffect
- `third_party/bevy/crates/bevy_ecs/src/bundle/tests.rs`: struct SparseV, struct SparseA, struct Exported
- `third_party/bevy/crates/bevy_ecs/src/bundle/writer.rs`: struct BundleScratch, struct BundleWriter
- `third_party/bevy/crates/bevy_ecs/src/change_detection/maybe_location.rs`: struct MaybeLocation
- `third_party/bevy/crates/bevy_ecs/src/change_detection/params.rs`: struct ContiguousComponentTicksRef, struct ContiguousComponentTicksMut, struct Res, struct ResMut, struct NonSend, struct NonSendMut, struct Ref, struct ContiguousRef, struct Mut, struct ContiguousMut, struct MutUntyped
- `third_party/bevy/crates/bevy_ecs/src/change_detection/tick.rs`: struct Tick, struct AtomicTick, struct CheckChangeTicks, struct ComponentTickCells, struct ComponentTicks
- `third_party/bevy/crates/bevy_ecs/src/change_detection/traits.rs`: trait DetectChanges, trait DetectChangesMut
- `third_party/bevy/crates/bevy_ecs/src/component/clone.rs`: type ComponentCloneFn, enum ComponentCloneBehavior, fn component_clone_via_clone, fn component_clone_via_reflect, fn component_clone_ignore, struct DefaultCloneBehaviorSpecialization, trait DefaultCloneBehaviorBase, trait DefaultCloneBehaviorViaClone
- `third_party/bevy/crates/bevy_ecs/src/component/info.rs`: struct ComponentInfo, struct ComponentId, struct ComponentDescriptor, struct Components
- `third_party/bevy/crates/bevy_ecs/src/component/mod.rs`: trait Component, trait ComponentMutability, struct Immutable, struct Mutable, enum StorageType, struct ComponentIdFor
- `third_party/bevy/crates/bevy_ecs/src/component/register.rs`: struct ComponentIds, struct ComponentsRegistrator, struct QueuedComponents, struct ComponentsQueuedRegistrator
- `third_party/bevy/crates/bevy_ecs/src/component/required.rs`: struct RequiredComponent, struct RequiredComponentConstructor, struct RequiredComponents, enum RequiredComponentsError, struct RequiredComponentsRegistrator
- `third_party/bevy/crates/bevy_ecs/src/entity/clone_entities.rs`: struct SourceComponent, struct ComponentCloneCtx, struct EntityCloner, struct EntityClonerBuilder, trait CloneByFilter, enum EntityClonerFilter, struct OptOut, struct OptIn
- `third_party/bevy/crates/bevy_ecs/src/entity/entity_set.rs`: trait ContainsEntity, trait EntityEquivalent, trait EntitySet, trait EntitySetIterator, trait FromEntitySetIterator, struct UniqueEntityIter
- `third_party/bevy/crates/bevy_ecs/src/entity/hash.rs`: struct EntityHash, struct EntityHasher
- `third_party/bevy/crates/bevy_ecs/src/entity/hash_map.rs`: struct EntityEquivalentHashMap, type EntityHashMap, struct Keys, struct IntoKeys
- `third_party/bevy/crates/bevy_ecs/src/entity/hash_set.rs`: struct EntityEquivalentHashSet, type EntityHashSet, struct Iter, struct IntoIter, struct Drain, struct ExtractIf
- `third_party/bevy/crates/bevy_ecs/src/entity/index_map.rs`: struct EntityEquivalentIndexMap, type EntityIndexMap, struct Slice, struct Iter, struct IterMut, struct IntoIter, struct Drain, struct Keys, struct IntoKeys
- `third_party/bevy/crates/bevy_ecs/src/entity/index_set.rs`: struct EntityEquivalentIndexSet, type EntityIndexSet, struct Slice, struct Iter, struct IntoIter, struct Drain
- `third_party/bevy/crates/bevy_ecs/src/entity/map_entities.rs`: trait MapEntities, trait EntityMapper, struct SceneEntityMapper
- `third_party/bevy/crates/bevy_ecs/src/entity/mod.rs`: mod hash_map, mod hash_set, mod index_map, mod index_set, mod unique_array, mod unique_slice, mod unique_vec, struct EntityIndex, struct EntityGeneration, struct Entity, struct ParseEntityError, struct EntityAllocator, struct AllocEntitiesIterator, struct Entities, enum SpawnError, struct InvalidEntityError, struct EntityValidButNotSpawnedError, enum EntityNotSpawnedError, struct EntityLocation
- `third_party/bevy/crates/bevy_ecs/src/entity/remote_allocator.rs`: struct RemoteAllocator
- `third_party/bevy/crates/bevy_ecs/src/entity/unique_array.rs`: struct UniqueEntityEquivalentArray, type UniqueEntityArray, type IntoIter
- `third_party/bevy/crates/bevy_ecs/src/entity/unique_slice.rs`: struct UniqueEntityEquivalentSlice, type UniqueEntitySlice, fn from_ref, fn from_mut, fn from_raw_parts, fn from_raw_parts_mut, fn cast_slice_of_unique_entity_slice, fn cast_slice_of_unique_entity_slice_mut, fn cast_slice_of_mut_unique_entity_slice_mut, type Iter, type IterMut, struct UniqueEntityEquivalentSliceIter, type Windows, type Chunks, type ChunksExact, type RChunks, type RChunksExact, type ChunkBy, type Split, type SplitInclusive, type RSplit, type SplitN, type RSplitN, struct UniqueEntityEquivalentSliceIterMut, ...
- `third_party/bevy/crates/bevy_ecs/src/entity/unique_vec.rs`: struct UniqueEntityEquivalentVec, type UniqueEntityVec, type IntoIter, type Drain, type Splice
- `third_party/bevy/crates/bevy_ecs/src/entity_disabling.rs`: struct Disabled, struct DefaultQueryFilters
- `third_party/bevy/crates/bevy_ecs/src/error/bevy_error.rs`: struct BevyError, enum Severity, trait ResultSeverityExt, trait ContextExt, fn bevy_error_panic_hook, macro bevy_error!, macro bail!, macro ensure!
- `third_party/bevy/crates/bevy_ecs/src/error/command_handling.rs`: trait CommandOutput, trait EntityCommandOutput
- `third_party/bevy/crates/bevy_ecs/src/error/handler.rs`: enum ErrorContext, type ErrorHandler, struct FallbackErrorHandler, fn match_severity, fn panic, fn error, fn warn, fn info, fn debug, fn trace, fn ignore
- `third_party/bevy/crates/bevy_ecs/src/error/mod.rs`: type Result
- `third_party/bevy/crates/bevy_ecs/src/event/mod.rs`: trait Event, trait EventPattern, trait EntityEvent, trait SetEntityEventTarget, struct EventKey
- `third_party/bevy/crates/bevy_ecs/src/event/trigger.rs`: trait Trigger, type EventTriggerState, type EventTriggerView, struct GlobalTrigger, struct EntityTrigger, fn trigger_entity_internal, struct PropagateEntityTrigger, struct EntityComponentsTrigger
- `third_party/bevy/crates/bevy_ecs/src/hierarchy.rs`: struct ChildOf, struct Children, type ChildSpawner, type ChildSpawnerCommands, macro children!
- `third_party/bevy/crates/bevy_ecs/src/intern.rs`: struct Interned, trait Internable, struct Interner
- `third_party/bevy/crates/bevy_ecs/src/label.rs`: trait DynEq, trait DynHash, macro define_label!
- `third_party/bevy/crates/bevy_ecs/src/lib.rs`: mod archetype, mod batching, mod bundle, mod change_detection, mod component, mod entity, mod entity_disabling, mod error, mod event, mod hierarchy, mod intern, mod label, mod lifecycle, mod message, mod name, mod observer, mod query, mod reflect, mod relationship, mod resource, mod schedule, mod spawn, mod storage, mod system, ...
- `third_party/bevy/crates/bevy_ecs/src/lifecycle.rs`: type ComponentHook, struct HookContext, struct ComponentHooks, struct AddEvent, struct Add, struct InsertEvent, struct Insert, struct DiscardEvent, struct Discard, struct RemoveEvent, struct Remove, struct DespawnEvent, struct Despawn, struct RemovedComponentEntity, struct RemovedComponentReader, struct RemovedComponentMessages, struct RemovedComponents, type RemovedIter, type RemovedIterWithId
- (99 more files)

## Examples in the source tree

- `third_party/bevy/crates/bevy_ecs/examples/change_detection.rs`
- `third_party/bevy/crates/bevy_ecs/examples/events.rs`
- `third_party/bevy/crates/bevy_ecs/examples/resources.rs`

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
