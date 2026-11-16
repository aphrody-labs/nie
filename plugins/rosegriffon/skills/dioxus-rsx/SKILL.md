---
name: dioxus-rsx
description: "Use when working with the `dioxus-rsx` crate of dioxus: Core functionality for Dioxus - a concurrent renderer-agnostic Virtual DOM for interactive user experiences."
---

# dioxus-rsx

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/rsx/` (manifest `third_party/dioxus/packages/rsx/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/rsx
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Core functionality for Dioxus - a concurrent renderer-agnostic Virtual DOM for interactive user experiences

## README (`third_party/dioxus/packages/rsx/README.md`)

# Dioxus-RSX

[Website](https://dioxuslabs.com) |
[Guides](https://dioxuslabs.com/learn/0.7/) |
[API Docs](https://docs.rs/dioxus-rsx/latest/dioxus_rsx) |
[Chat](https://discord.gg/XgGxMSkvUM)

## Overview

This crate provides the actual DSL that Dioxus uses in the `rsx!` macro. This crate is separate from the macro crate to enable tooling like autoformat, translation, and AST manipulation (extract to component).

## Contributing

- Report issues on our [issue tracker](https://github.com/dioxuslabs/dioxus/issues).
- Join the discord and ask questions!

## Design

The architecture of this crate has gone through several redesigns and we're hoping that we've finally landed on a final design that we're happy with.

There is still some thinking that we could approach this by flattening the AST to have better incremental parsing and performance without having to add metadata to the data structures.

For now, however, the design we've settled on:

- A top-level CallBody containing a single TemplateBody
- Each TemplateBody has its dynamic mapping on it, filled in after parsing (...not during)
- Hotreload Information for things like literals is propagated as a final pass into nodes on the tree

This lets us incrementally build up the tree to make it modular/easier to test in exchange for extra "bookkeeping passes".

The good part of bookkeeping passes is:
- They're optional
- They're generally pretty quick
- They can be enforced as part of tree construction in the various levels

Of course the major downside is they're 1) slower and 2) need a way to store metadata, which they do on the nodes themselves. Sometimes you might not even want to collect metadata which we currently have no way of opting-out of.

This can make global reasoning about the tree a bit harder since you need to dig into each node for its metadata, but the local reasoning is much better. Previously we stored the metadata on a secondary "view" type item - and this was fine... but it's not ideal for long-lived programs that persist a queryable tree-object.

We want to query the tree:
- For its dynamic mappings
- For its hot-reloadable contents

An alternative approach would be
- flattened AST
- single "parser" IE `self.parse_element` / `self.parse_component`

I've done a bunch of research on parsers and cannot find many (any?) parsers that are implemented statefully.

We *do* want a reference to these items - currently its their path, but it could be done via just an ID for each node. Unfortunately there's no easy way to "pool" these up other than Arc/Rc.

Anyways, until our applications are limited by query performance, keeping the tree unflattened seems fine. The JS DOM has a similar API where it flushes changes after modifications, but also has an ID system via pointers. We could try adding IDs with something like generational-box or just Arc/Rc.

Note that we still don't get the "goodies" of a proper tree-sitter type parser (skipping tokenization incrementally) but that really isn't achievable without being a part of the LSP. In theory, we can eventually use the LSP to help modify the tree in place and then "fix" it if you wanted to modify trees in place. That being said, we still need to perform diffing of two rsx! trees even when we know their shape has changed.

## License

This project is licensed under the [MIT license].

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Dioxus by you shall be licensed as MIT, without any additional
terms or conditions.

## Cargo features (from the manifest)

- `default`

## Depends on (same ecosystem)

`dioxus-vdom-template`

## Public items by file

- `third_party/dioxus/packages/rsx/src/attribute.rs`: struct Attribute, enum AttributeName, struct Spread, enum AttributeValue, struct IfAttributeValue
- `third_party/dioxus/packages/rsx/src/component.rs`: struct Component
- `third_party/dioxus/packages/rsx/src/diagnostics.rs`: struct Diagnostics
- `third_party/dioxus/packages/rsx/src/element.rs`: struct Element, enum ElementName
- `third_party/dioxus/packages/rsx/src/expr_node.rs`: struct ExprNode
- `third_party/dioxus/packages/rsx/src/fill_order.rs`: trait FillOrderVisitor, fn visit_roots, fn visit_element, fn siblings_have_static_node
- `third_party/dioxus/packages/rsx/src/forloop.rs`: struct ForLoop
- `third_party/dioxus/packages/rsx/src/ifchain.rs`: struct IfChain
- `third_party/dioxus/packages/rsx/src/ifmt.rs`: struct IfmtInput, enum Segment, struct FormattedSegment
- `third_party/dioxus/packages/rsx/src/literal.rs`: enum HotLiteral, struct HotReloadFormattedSegment
- `third_party/dioxus/packages/rsx/src/location.rs`: struct DynIdx
- `third_party/dioxus/packages/rsx/src/node.rs`: enum BodyNode
- `third_party/dioxus/packages/rsx/src/partial_closure.rs`: struct PartialClosure
- `third_party/dioxus/packages/rsx/src/raw_expr.rs`: struct PartialExpr
- `third_party/dioxus/packages/rsx/src/rsx_block.rs`: struct RsxBlock, enum RsxItem
- `third_party/dioxus/packages/rsx/src/rsx_call.rs`: struct CallBody
- `third_party/dioxus/packages/rsx/src/template_body.rs`: struct TemplateBody
- `third_party/dioxus/packages/rsx/src/text_node.rs`: struct TextNode
- `third_party/dioxus/packages/rsx/src/util.rs`: fn parse_raw_ident

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
