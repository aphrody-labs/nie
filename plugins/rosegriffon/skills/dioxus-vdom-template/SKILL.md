---
name: dioxus-vdom-template
description: "Use when working with the `dioxus-vdom-template` crate of dioxus: Static template representation and const builder for Dioxus."
---

# dioxus-vdom-template

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/vdom-template/` (manifest `third_party/dioxus/packages/vdom-template/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/vdom-template
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Static template representation and const builder for Dioxus.

## Crate docs (`third_party/dioxus/packages/vdom-template/src/lib.rs`)

Static template storage for Dioxus views.

The `rsx!` macro lowers each view into two pieces:

- a static [`Template`] describing the stable element, text, and static
  attribute structure
- runtime dynamic node and dynamic attribute arrays for one render

The template uses compact paths and anchors to connect those runtime values
back to stable locations in the static tree.

## Annotated RSX lowering

Consider this `rsx!` block:

```rust,ignore
rsx! {
    {before}
    div { class: "{class_name}",
        "Hello "
        strong { "{name}" }
        {badge}
        "{count}"
        span { "!" }
    }
    {after}
}
```

Dynamic nodes do not receive their own paths. Paths are only stored when an
emitted anchor needs to point to a static node. The path
notation below is a tree walk: `0` means "next child", and `1` means "next
sibling". The empty path is the vnode render-parent site; it is not a real
static node.

This table includes only paths that are actually emitted in anchor slot
targets for the example above:

```text
Path walk    Last static node?    Why it is emitted
0            false                root anchor for div / node before div
0            false                dynamic class attrs on div
001          true                 dynamic text inside strong
0011         false                badge and count before span
0            true                 trailing root dynamic node after div
```

There is no emitted path for `"Hello "` because no dynamic slot is anchored
to that static text node. The same path can appear with different parent
ownership: a root-level anchor at `0` can insert before `div`, while an
element-owned anchor at `0` applies dynamic attributes to `div`.

The low tag bit on an anchor path records whether the path is the last
static node before the dynamic slot. When it is false, dynamic nodes insert
before the static node at `path`. When it is true, dynamic nodes insert
after that static node, or append to it when the path is also the anchor's
owning element.

Anchors are created for dynamic node or attribute groups and for every root static node. A
non-root static element appears as an anchor parent only when it owns dynamic attributes or a

## Cargo features (from the manifest)

- `serialize` (enables 1)

## Depends on (same ecosystem)

`dioxus-const-vec`

## Public items by file

- `third_party/dioxus/packages/vdom-template/src/anchor.rs`: struct TemplateAnchor
- `third_party/dioxus/packages/vdom-template/src/data.rs`: struct Template, enum StaticTemplateNode, struct StaticTemplateElement, struct StaticTemplateText, struct StaticTemplateAttribute, struct StaticTemplateNodeIter, struct StaticTemplateAttributeIter
- `third_party/dioxus/packages/vdom-template/src/op.rs`: enum DecodedTemplateOp
- `third_party/dioxus/packages/vdom-template/src/path.rs`: struct TemplatePath, struct TemplatePathSegments, struct TemplateSlotPath, struct TemplateSlotTarget
- `third_party/dioxus/packages/vdom-template/src/raw.rs`: enum TemplateRawTree
- `third_party/dioxus/packages/vdom-template/src/serialization.rs`: fn deserialize_string_leaky, fn deserialize_option_leaky
- `third_party/dioxus/packages/vdom-template/src/storage.rs`: struct TemplateStorage, fn build_runtime_template, struct TemplateElementFrame, struct TemplateLoweringCursor, struct RuntimeTemplateBuilder

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
