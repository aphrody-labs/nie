---
name: blitz-dom
description: "Use when working with the `blitz-dom` crate of blitz: Blitz DOM implementation."
---

# blitz-dom

- Version: 0.3.0-beta.4 (workspace member of `third_party/blitz`, vendored commit `a84a3c51b562`)
- Source: `third_party/blitz/packages/blitz-dom/` (manifest `third_party/blitz/packages/blitz-dom/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/blitz/tree/main/packages/blitz-dom
- License: [object Object]
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Blitz DOM implementation

## Crate docs (`third_party/blitz/packages/blitz-dom/src/lib.rs`)

The core DOM abstraction in Blitz

This crate implements a flexible headless DOM ([`BaseDocument`]), which is designed to emebedded in and "driven" by external code. Most users will want
to use a wrapper:

 - [`HtmlDocument`](https://docs.rs/blitz-html/latest/blitz_html/struct.HtmlDocument.html) from the [blitz-html](https://docs.rs/blitz-html) crate.
   Allows you to parse HTML (or XHTML) into a Blitz [`BaseDocument`], and can be combined with a markdown-to-html converter like [comrak](https://docs.rs/comrak)
   or [pulldown-cmark](https://docs.rs/pulldown-cmark) to render/process markdown.
 - [`DioxusDocument`](https://docs.rs/dioxus-native/latest/dioxus_native/struct.DioxusDocument.html) from the [dioxus-native](https://docs.rs/dioxus-native) crate.
   Combines a [`BaseDocument`] with a Dioxus `VirtualDom` to enable dynamic rendering and event handling.

It includes: A DOM tree respresentation, CSS parsing and resolution, layout and event handling. Additional functionality is available in
separate crates, including html parsing ([blitz-html](https://docs.rs/blitz-html)), networking ([blitz-net](https://docs.rs/blitz-html)),
rendering ([blitz-paint](https://docs.rs/blitz-paint)) and windowing ([blitz-shell](https://docs.rs/blitz-shell)).

Most of the functionality in this crates is provided through the  struct.

`blitz-dom` has a native Rust API that is designed for higher-level abstractions to be built on top (although it can also be used directly).

The goal behind this crate is that any implementor can interact with the DOM and render it out using any renderer
they want.

## Cargo features (from the manifest)

- `default` (enables 7)
- `custom-widget` (enables 2)
- `tracing` (enables 1)
- `svg` (enables 2)
- `woff` (enables 1): WOFF decoding (using the "wuff" crate which is pure Rust)
- `accessibility` (enables 1)
- `system-fonts` (enables 1)
- `complex-scripts` (enables 1): complex-scripts: enable dictionary-based line-breaking
- `text-transform-icu` (enables 2): text-transform-icu: language-aware, Unicode-complete case mapping for `text-transform` (otherwise falls back to the standard library's case mapping)
- `autofocus`
- `floats` (enables 2)
- `writing-mode` (enables 1): CSS vertical `writing-mode` support (layout runs in each box's writing-mode frame; see layout/writing_mode.rs)
- `file-input`
- `scrollbars`: Overlay-scrollbar thumb interaction (drag + hover tracking). Normally enabled through `blitz-paint/scrollbars`, which paints the thumbs.
- `parallel-construct`
- `log-phase-times` (enables 1)
- `devtools`: Report DOM mutations to a `DevtoolsProvider` (e.g. for a remote inspector)

## Depends on (same ecosystem)

`blitz-traits`, `debug_timer`, `stylo_taffy`, `taffy`

## Public items by file

- `third_party/blitz/packages/blitz-dom/src/config.rs`: enum StyleThreading, struct DocumentConfig
- `third_party/blitz/packages/blitz-dom/src/cssom.rs`: enum CssomError, struct CssRuleInfo
- `third_party/blitz/packages/blitz-dom/src/document.rs`: enum DocGuard, enum DocGuardMut, trait Document, struct PlainDocument, enum DocumentEvent, struct BaseDocument, struct BoundingRect
- `third_party/blitz/packages/blitz-dom/src/events/driver.rs`: trait EventHandler, struct NoopEventHandler, struct EventDriver
- `third_party/blitz/packages/blitz-dom/src/form.rs`: enum RequestContentType
- `third_party/blitz/packages/blitz-dom/src/html.rs`: trait HtmlParserProvider, struct DummyHtmlParserProvider
- `third_party/blitz/packages/blitz-dom/src/layout/mod.rs`: struct RefCellChildIter
- `third_party/blitz/packages/blitz-dom/src/layout/paint_tree.rs`: fn hoisted_child_position, struct HoistedPaintChild, struct StackingContext
- `third_party/blitz/packages/blitz-dom/src/layout/replaced.rs`: struct IntrinsicSizes, struct ReplacedContext, fn compute_replaced_layout
- `third_party/blitz/packages/blitz-dom/src/layout/table.rs`: struct TableTreeWrapper, struct TableContext, struct TableCell, struct TableColumn, struct TableRow, struct RangeIter
- `third_party/blitz/packages/blitz-dom/src/layout/text_transform.rs`: fn full_width, fn full_size_kana
- `third_party/blitz/packages/blitz-dom/src/lib.rs`: mod node, mod traversal, mod net, mod util, fn taffy_node_id, fn dom_node_id, type SelectorList, fn build_single_font_ctx
- `third_party/blitz/packages/blitz-dom/src/mutator.rs`: enum AppendTextErr, struct DocumentMutator, struct ViewportMut
- `third_party/blitz/packages/blitz-dom/src/net.rs`: struct FontFaceOverrides, enum Resource, struct ResourceLoadResponse, struct StylesheetHandler, struct ImageHandler
- `third_party/blitz/packages/blitz-dom/src/node/attributes.rs`: struct Attribute, struct Attributes
- `third_party/blitz/packages/blitz-dom/src/node/custom_widget.rs`: struct ProxyRenderContext, trait Widget, enum CustomWidgetStatus, struct CustomWidgetData
- `third_party/blitz/packages/blitz-dom/src/node/element.rs`: struct ElementData, struct LayoutData, struct DocumentData, enum SpecialElementType, enum SpecialElementData, struct RasterImageData, enum ImageData, enum Status, struct ImageResourceData, struct CanvasData, struct ListItemLayout, enum Marker, enum ListItemLayoutPosition
- `third_party/blitz/packages/blitz-dom/src/node/node.rs`: enum DisplayOuter, struct Node, enum NodeKind, enum NodeData, struct TextNodeData
- `third_party/blitz/packages/blitz-dom/src/node/scrollbar.rs`: struct ScrollbarRef, enum ScrollbarColor
- `third_party/blitz/packages/blitz-dom/src/node/stylo_data.rs`: struct StyloData, struct StyleDataRef, struct ComputedStyleRef
- `third_party/blitz/packages/blitz-dom/src/node/svg.rs`: struct SvgIntrinsicDimensions, struct SvgImageData
- `third_party/blitz/packages/blitz-dom/src/node/text.rs`: struct TextBrush, struct TextLayout, enum GeneratedTextInputEvent, struct TextInputData
- `third_party/blitz/packages/blitz-dom/src/resolved_style.rs`: fn css_property_is_supported, fn parse_transform_matrix, fn resolved_style_property_names
- `third_party/blitz/packages/blitz-dom/src/scrolling.rs`: enum ScrollBehavior, enum ScrollLogicalPosition
- `third_party/blitz/packages/blitz-dom/src/selection.rs`: struct SelectionEndpoint, struct TextSelection
- `third_party/blitz/packages/blitz-dom/src/stylo/traversal.rs`: struct RegisteredPaintersImpl, struct RecalcStyle
- `third_party/blitz/packages/blitz-dom/src/stylo/user_agent.rs`: struct UserAgentStylesheet
- `third_party/blitz/packages/blitz-dom/src/stylo_to_kurbo.rs`: fn resolve_2d_transform
- `third_party/blitz/packages/blitz-dom/src/traversal.rs`: struct TreeTraverser, struct AncestorTraverser
- `third_party/blitz/packages/blitz-dom/src/tree.rs`: struct NodeTree
- `third_party/blitz/packages/blitz-dom/src/util.rs`: type Color, fn decode_font_bytes, enum ImageLayerKind, enum ImageType, struct Point, fn walk_tree, trait ToColorColor, macro qual_name!

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
