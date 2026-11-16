---
name: dioxus-vdom
description: "Use when working with the `dioxus-vdom` crate of dioxus: Build fullstack web, desktop, and mobile apps with a single codebase."
---

# dioxus-vdom

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/vdom/` (manifest `third_party/dioxus/packages/vdom/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/vdom
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Build fullstack web, desktop, and mobile apps with a single codebase.

## README (`third_party/dioxus/packages/vdom/README.md`)

# dioxus-vdom

`dioxus-vdom` provides a fast and featureful VirtualDom implementation for Rust.

```rust, no_run
# tokio::runtime::Runtime::new().unwrap().block_on(async {
use dioxus_vdom::{VirtualDom, Event, Element, Mutations, VNode, ElementId};

let mut vdom = VirtualDom::new(app);
let real_dom = SomeRenderer::new();

loop {
    tokio::select! {
        evt = real_dom.event() => {
            let evt = Event::new(evt, true);
            vdom.runtime().handle_event("onclick", evt, ElementId::from_raw(0))
        },
        _ = vdom.wait_for_work() => {}
    }
    vdom.render_immediate(&mut real_dom.apply())
}

# fn app() -> Element { VNode::empty() }
# struct SomeRenderer; impl SomeRenderer { fn new() -> SomeRenderer { SomeRenderer } async fn event(&self) -> std::rc::Rc { unimplemented!() } fn apply(&self) -> Mutations { Mutations::default() } }
# });
```

## Features

A virtualdom is an efficient and flexible tree data structure that allows you to manage state for a graphical user interface. The Dioxus VirtualDom is perhaps the most fully-featured virtualdom implementation in Rust and powers renderers running across Web, Desktop, Mobile, SSR, TUI, LiveView, and more. When you use the Dioxus VirtualDom, you immediately enable users of your renderer to leverage the wide ecosystem of Dioxus components, hooks, and associated tooling.

Some features of `dioxus-vdom` include:

- UI components are just functions
- State is provided by hooks
- Deep integration with async
- Strong focus on performance
- Integrated hotreloading support
- Extensible system for UI elements and their attributes

If you are just starting, check out the Guides first.

## Understanding the implementation

`dioxus-vdom` is designed to be a lightweight crate that. It exposes a number of flexible primitives without being deeply concerned about the intracices of state management itself. We provide a number of useful abstractions built on these primitives in the `dioxus-hooks` crate as well as the `dioxus-signals` crate.

The important abstractions to understand are:

- The [`VirtualDom`]
- The [`Component`] and its [`Properties`]
- Handling events
- Working with async
- Suspense

## Usage

The `dioxus` crate exports the `rsx` macro which transforms a helpful, simpler syntax of Rust.

First, start with your app:

```rust
# use dioxus::dioxus_vdom::{Mutations, VirtualDom};
use dioxus::prelude::*;

// First, declare a root component
fn app() -> Element {
    rsx! {
        div { "hello world" }
    }
}

## Cargo features (from the manifest)

- `serialize` (enables 2)
- `manganis_08` (enables 1)

## Depends on (same ecosystem)

`dioxus-const-vec`, `dioxus-vdom-macro`, `dioxus-vdom-template`, `generational-box`, `manganis-core`, `subsecond`

## Public items by file

- `third_party/dioxus/packages/vdom/src/arena.rs`: struct ElementId, struct RenderTargetId
- `third_party/dioxus/packages/vdom/src/diff/template.rs`: enum VNodeChild, struct StaticElement, struct StaticText, struct DynamicAnchor, struct DynamicNodeSlot, struct DynamicAttrSlot
- `third_party/dioxus/packages/vdom/src/error_boundary.rs`: fn provide_create_error_boundary, fn use_error_boundary_provider, struct ErrorContext, struct ErrorHandler, struct ErrorBoundaryProps, fn ErrorBoundary, macro bail!
- `third_party/dioxus/packages/vdom/src/events.rs`: struct Event, type EventHandler, struct Callback, trait SpawnIfAsync, struct AsyncMarker, struct AsyncResultMarker, struct MarkerWrapper, struct UnitClosure, struct ListenerCallback
- `third_party/dioxus/packages/vdom/src/fragment.rs`: fn Fragment, struct FragmentProps
- `third_party/dioxus/packages/vdom/src/generational_box.rs`: fn with_owner, fn current_owner
- `third_party/dioxus/packages/vdom/src/global_context.rs`: fn current_scope_id, fn throw_error, fn try_consume_context, fn consume_context, fn consume_context_from_scope, fn has_context, fn provide_context, fn provide_root_context, fn suspend, fn spawn_isomorphic, fn spawn, fn queue_effect, fn spawn_forever, fn remove_future, fn use_hook, fn generation, fn parent_scope, fn needs_update, fn needs_update_any, fn schedule_update, fn schedule_update_any, fn use_drop, fn use_before_render, fn use_after_render, ...
- `third_party/dioxus/packages/vdom/src/hotreload_utils.rs`: enum HotReloadLiteral, struct FmtedSegments, enum FmtSegment, struct DynamicLiteralPool, struct DynamicValuePool, struct HotReloadTemplateWithLocation, struct TemplateGlobalKey, struct HotReloadedTemplate, enum HotReloadDynamicNode, enum HotReloadDynamicAttribute, struct NamedAttribute, enum HotReloadAttributeValue
- `third_party/dioxus/packages/vdom/src/launch.rs`: trait LaunchConfig
- `third_party/dioxus/packages/vdom/src/lib.rs`: mod view, mod internal, fn Ok
- `third_party/dioxus/packages/vdom/src/mutations.rs`: trait WriteMutations, trait MultiWriter, enum Mutation, struct Mutations, struct NoOpMutations
- `third_party/dioxus/packages/vdom/src/nodes.rs`: struct DynamicValues, struct VNode, struct MountedVNode, enum DynamicNode, struct VComponent, struct VText, struct Attribute, enum AttributeValue, enum BorrowedAttributeValue, trait AnyValue, trait IntoDynNode, trait IntoVNode, struct FromNodeIterator, trait IntoAttributeValue, trait HasAttributes
- `third_party/dioxus/packages/vdom/src/portal.rs`: struct PortalProps, struct PortalPropsBuilder, struct PortalComponentBuilder, fn Portal
- `third_party/dioxus/packages/vdom/src/properties.rs`: trait Properties, struct EmptyBuilder, trait ComponentFunction, trait ComponentFunctionExt, struct ComponentBuilder, struct ComponentBuilderOutput, trait ComponentBuilderRender, struct EmptyMarker, trait SuperInto, trait SuperFrom, struct OptionStringFromMarker, struct OptionArgumentsFromMarker, struct OptionCallbackMarker
- `third_party/dioxus/packages/vdom/src/reactive_context.rs`: struct ReactiveContext, struct Subscribers, trait SubscriberList
- `third_party/dioxus/packages/vdom/src/render_error.rs`: enum RenderError, struct CapturedError
- `third_party/dioxus/packages/vdom/src/runtime.rs`: struct Runtime, struct RuntimeGuard
- `third_party/dioxus/packages/vdom/src/scheduler.rs`: struct ScopeOrder
- `third_party/dioxus/packages/vdom/src/scopes.rs`: struct ScopeId, struct ScopeState
- `third_party/dioxus/packages/vdom/src/suspense/component.rs`: struct SuspenseBoundaryProps, struct SuspenseBoundaryPropsBuilder, struct SuspenseBoundaryComponentBuilder, trait SuspenseBoundaryPropsBuilder_Optional, enum SuspenseBoundaryPropsBuilder_Error_Repeated_field_fallback, enum SuspenseBoundaryPropsBuilder_Error_Repeated_field_children, enum SuspenseBoundaryPropsBuilder_Error_Missing_required_field_fallback, struct SuspenseBoundaryPropsWithOwner, fn SuspenseBoundary
- `third_party/dioxus/packages/vdom/src/suspense/mod.rs`: struct SuspendedFuture, struct SuspenseContext
- `third_party/dioxus/packages/vdom/src/tasks.rs`: struct Task
- `third_party/dioxus/packages/vdom/src/view/attribute.rs`: trait AttributeDescriptor, struct StaticAttributeBuilder, fn static_attribute, trait StaticAttributeValue, struct StaticAttributeValueBuilder, struct StaticAttributeValueBuilderMarker, trait IntoAttributeBuilderValue, struct DynamicAttributesBuilder, fn dynamic_attributes_builder, trait AttributeBuilderTarget
- `third_party/dioxus/packages/vdom/src/view/child.rs`: struct DynamicViewChildMarker, struct DynamicNodeBuilder, fn dynamic_node_builder, struct ViewChildMarker, trait IntoViewChild
- `third_party/dioxus/packages/vdom/src/view/element.rs`: trait ElementTag, struct ElementBuilder, fn element_builder
- `third_party/dioxus/packages/vdom/src/view/fragment.rs`: struct FragmentBuilder, fn fragment
- `third_party/dioxus/packages/vdom/src/view/text.rs`: trait StaticText, struct StaticTextBuilder
- `third_party/dioxus/packages/vdom/src/view/traits.rs`: trait ViewTemplate, trait View, struct KeyedView, trait ViewKeyExt, trait ViewExt, fn into_vnode_cached, fn into_vnode_with_capacity
- `third_party/dioxus/packages/vdom/src/virtual_dom.rs`: struct VirtualDom

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
