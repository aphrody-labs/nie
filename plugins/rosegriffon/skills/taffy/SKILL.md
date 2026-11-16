---
name: taffy
description: "Use when working with the `taffy` crate of taffy: A flexible UI layout library ."
---

# taffy

- Version: 0.15.1 (workspace member of `third_party/taffy`, vendored commit `49124638d1ca`)
- Source: `third_party/taffy/` (manifest `third_party/taffy/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/taffy/tree/main/
- License: MIT
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Orientation

Layout engine shared by Bevy UI, Blitz and Dioxus native: flexbox, grid, block, absolute positioning, with the high-level `TaffyTree` and the low-level traits for custom trees (`third_party/taffy/README.md`, `src/`, `examples/`, `benches/`).

Monorepo rules: the fork carries version `0.15.1` (the registry release) so `[patch.crates-io]` replaces it everywhere and every consumer compiles one copy; the root `Cargo.toml` requires `taffy = "0.15.1"`. The fork adds `[profile.rg-wasm]` (`opt-level = "z"`, fat LTO). A `taffy` bug found while porting UI belongs upstream in DioxusLabs/taffy (one issue or PR per change).

## Purpose

A flexible UI layout library 

## README (`third_party/taffy/README.md`)

Taffy is a flexible, high-performance, cross-platform UI layout library written in [Rust](https://www.rust-lang.org).

It currently implements the CSS **Block**, **Flexbox** and **CSS Grid** layout algorithms. Support for other paradigms is planned. For more information on this and other future development plans see the [roadmap issue](https://github.com/DioxusLabs/taffy/issues/345).

This crate is a collaborative, cross-team project, and is designed to be used as a dependency for other UI and GUI libraries.
Right now, it powers:

- [Servo](https://github.com/servo/servo): an alternative web browser
- [Blitz](https://github.com/DioxusLabs/blitz): a radically modular web engine
- [Bevy](https://bevyengine.org/): an ergonomic, ECS-first Rust game engine
- [Takumi](https://github.com/kane50613/takumi): Renders your React components to images
- [iocraft](https://github.com/ccbrown/iocraft): crafting beautiful interfaces for the terminal
- [Slint](https://github.com/slint-ui/slint): a declarative GUI toolkit for building native user interfaces
- The [Lapce](https://lapce.dev/) text editor via the [Floem](https://github.com/lapce/floem) UI framework
- The [Zed](https://zed.dev/) text editor via the [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) UI framework

## Usage

```rust
use taffy::prelude::*;

// First create an instance of TaffyTree
let mut tree : TaffyTree = TaffyTree::new();

// Create a tree of nodes using `TaffyTree.new_leaf` and `TaffyTree.new_with_children`.
// These functions both return a node id which can be used to refer to that node
// The Style struct is used to specify styling information
let header_node = tree
    .new_leaf(
        Style {
            size: Size { width: length(800.0), height: length(100.0) },
            ..Default::default()
        },
    ).unwrap();

let body_node = tree
    .new_leaf(
        Style {
            size: Size { width: length(800.0), height: auto() },
            flex_grow: 1.0,
            ..Default::default()
        },
    ).unwrap();

let root_node = tree
    .new_with_children(
        Style {
            flex_direction: FlexDirection::Column,
            size: Size { width: length(800.0), height: length(600.0) },
            ..Default::default()
        },
        &[header_node, body_node],
    )
    .unwrap();

// Call compute_layout on the root of your tree to run the layout algorithm
tree.compute_layout(root_node, Size::MAX_CONTENT).unwrap();

// Inspect the computed layout using `TaffyTree.layout`
assert_eq!(tree.layout(root_node).unwrap().size.width, 800.0);
assert_eq!(tree.layout(root_node).unwrap().size.height, 600.0);
assert_eq!(tree.layout(header_node).unwrap().size.width, 800.0);
assert_eq!(tree.layout(header_node).unwrap().size.height, 100.0);
assert_eq!(tree.layout(body_node).unwrap().size.width, 800.0);
assert_eq!(tree.layout(body_node).unwrap().size.height, 500.0); // This value was not set explicitly, but was computed by Taffy

```

## Bindings to other languages


## Cargo features (from the manifest)

- `default` (enables 10)
- `block_layout`: Enables the Block layout algorithm. See [`compute_block_layout`](crate::compute_block_layout).
- `float_layout`: Enables the Float layout algorithm. This is a sub-feature of block layout.
- `flexbox`: Enables the Flexbox layout algorithm. See [`compute_flexbox_layout`](crate::compute_flexbox_layout).
- `flexbox_balance` (enables 1): Enables support for `flex-wrap: balance` and `flex-line-count` from [CSS Flexbox Level 2](https://drafts.csswg.org/css-flexbox-2/)
- `grid` (enables 1): Enables the CSS Grid layout algorithm. See [`compute_grid_layout`](crate::compute_grid_layout).
- `calc`: Enables calc() values for all layout algorithms
- `content_size`: Causes all algorithms to compute and output a content size for each node
- `detailed_layout_info`: Causes algorithms to stores detailed information of the nodes in TaffyTree, with only CSS Grid supporting this.
- `strict_provenance`: Use strict provenance APIs for pointer manipulation. Using this feature requires Rust 1.84 or higher.
- `taffy_tree` (enables 2): Enable the built-in Taffy node tree. See [`TaffyTree`](crate::TaffyTree).
- `serde` (enables 2): Add [`serde`] derives to Style structs
- `parse` (enables 1): Implement `FromStr` trait for Taffy style types
- `parse_faster` (enables 2): Enable the `parse` feature with proc-macro dependent optimisations
- `std` (enables 2): Allow Taffy to depend on the [`Rust Standard Library`](std)
- `alloc`: Deprecated no-op: Taffy always depends on the `alloc` library. Retained so that existing `features = ["alloc"]` configurations keep compiling.
- `debug` (enables 1): Internal feature for debugging
- `profile` (enables 1): Internal feature for profiling

## Public items by file

- `third_party/taffy/src/compute/block.rs`: fn compute_block_align_content_offset, struct BlockFormattingContext, struct FloatMark, struct BlockContext, fn compute_block_layout
- `third_party/taffy/src/compute/flexbox.rs`: fn compute_flexbox_layout
- `third_party/taffy/src/compute/float.rs`: struct ContentSlot, struct BfcSlot, struct PlacedFloatedBox, struct FloatContext, struct FloatIntrinsicWidthCalculator
- `third_party/taffy/src/compute/grid/mod.rs`: fn compute_grid_layout, struct DetailedGridInfo, struct DetailedGridTracksInfo, struct DetailedGridItemsInfo
- `third_party/taffy/src/compute/grid/types/coordinates.rs`: struct GridLine, struct OriginZeroLine, trait GridCoordinate
- `third_party/taffy/src/compute/grid/types/named.rs`: struct GridLineNames, struct GridLineNamesIter
- `third_party/taffy/src/compute/leaf.rs`: fn compute_leaf_layout
- `third_party/taffy/src/compute/mod.rs`: fn compute_root_layout, fn compute_cached_layout, fn round_layout, fn compute_hidden_layout, mod detailed_info
- `third_party/taffy/src/compute/oof.rs`: fn resolve_static_offset, fn compute_oof_layout, struct OofLayoutResult, fn compute_oof_layout_for_area
- `third_party/taffy/src/geometry.rs`: enum AbsoluteAxis, enum AbstractAxis, struct Rect, struct Line, struct FloatInsets, struct Size, struct Point, struct MinMax
- `third_party/taffy/src/lib.rs`: mod compute, mod geometry, mod prelude, mod style, mod style_helpers, mod tree, mod util
- `third_party/taffy/src/style/alignment.rs`: enum AlignItemsKeyword, enum AlignContentKeyword, enum AlignmentSafety, struct AlignItems, type JustifyItems, type AlignSelf, type JustifySelf, struct AlignContent, type JustifyContent
- `third_party/taffy/src/style/available_space.rs`: enum AvailableSpace
- `third_party/taffy/src/style/block.rs`: trait BlockContainerStyle, trait BlockItemStyle, enum TextAlign
- `third_party/taffy/src/style/compact_length.rs`: struct CompactLength
- `third_party/taffy/src/style/dimension.rs`: struct LengthPercentage, enum ExpandedLengthPercentage, struct LengthPercentageAuto, enum ExpandedLengthPercentageAuto, struct Dimension, enum ExpandedDimension
- `third_party/taffy/src/style/flex.rs`: trait FlexboxContainerStyle, trait FlexboxItemStyle, enum FlexWrap, enum FlexDirection
- `third_party/taffy/src/style/float.rs`: enum Float, enum FloatDirection, enum Clear
- `third_party/taffy/src/style/grid.rs`: struct GridTemplateAreas, struct GridTemplateArea, struct NamedGridLine, trait GenericRepetition, trait TemplateLineNames, enum GenericGridTemplateComponent, trait GridContainerStyle, trait GridItemStyle, enum GridAutoFlow, enum GenericGridPlacement, enum GridPlacement, struct MaxTrackSizingFunction, enum ExpandedMaxTrackSizingFunction, struct MinTrackSizingFunction, enum ExpandedMinTrackSizingFunction, type TrackSizingFunction, enum RepetitionCount, struct InvalidStringRepetitionValue, struct GridTemplateRepetition, enum GridTemplateComponent, struct GridTemplateTracks, struct GridAutoTracks
- `third_party/taffy/src/style/mod.rs`: trait CheapCloneStr, trait CoreStyle, trait OofItemStyle, enum Display, enum BoxGenerationMode, enum Position, struct ContainingBlockClaims, enum BoxSizing, enum Overflow, struct Contain, enum Direction, struct Style
- `third_party/taffy/src/style_helpers.rs`: fn repeat, fn evenly_sized_tracks, fn line, trait TaffyGridLine, fn span, trait TaffyGridSpan, fn minmax, fn flex, fn zero, trait TaffyZero, fn auto, trait TaffyAuto, fn min_content, trait TaffyMinContent, fn max_content, trait TaffyMaxContent, fn fit_content, trait TaffyFitContent, fn length, trait FromLength, fn percent, trait FromPercent, fn fr, trait FromFr
- `third_party/taffy/src/tree/cache.rs`: fn cache_mode_change_evictions, struct Cache, enum ClearState
- `third_party/taffy/src/tree/layout.rs`: struct ScrollableOverflow, enum RunMode, enum SizingMode, struct CollapsibleMarginSet, enum RequestedAxis, struct LayoutInput, struct Baselines, enum AxisStaticEdge, struct AxisStaticAlign, struct AxisStaticPosition, struct OofCandidate, struct OofCandidates, struct OofPositioningArea, struct LayoutOutput, struct Layout, enum DetailedLayoutInfo
- `third_party/taffy/src/tree/mod.rs`: mod traits
- `third_party/taffy/src/tree/node.rs`: struct NodeId
- `third_party/taffy/src/tree/taffy_tree.rs`: type TaffyResult, enum TaffyError, struct TaffyTree, struct TaffyTreeChildIter
- `third_party/taffy/src/tree/traits.rs`: trait TraversePartialTree, trait TraverseTree, trait LayoutPartialTree, trait LayoutContainingBlock, trait CacheTree, trait RoundTree, trait PrintTree, trait LayoutFlexboxContainer, trait LayoutGridContainer, trait LayoutBlockContainer
- `third_party/taffy/src/util/debug.rs`: struct DebugLogger
- `third_party/taffy/src/util/math.rs`: trait MaybeMath
- `third_party/taffy/src/util/parse.rs`: struct ParseError, type ParseResult
- `third_party/taffy/src/util/print.rs`: fn print_tree, fn write_tree
- `third_party/taffy/src/util/resolve.rs`: trait MaybeResolve, trait ResolveOrZero

## Examples in the source tree

- `third_party/taffy/examples/basic.rs`
- `third_party/taffy/examples/common/image.rs`
- `third_party/taffy/examples/common/text.rs`
- `third_party/taffy/examples/custom_tree_owned_partial.rs`
- `third_party/taffy/examples/custom_tree_owned_unsafe.rs`
- `third_party/taffy/examples/custom_tree_vec.rs`
- `third_party/taffy/examples/flexbox_gap.rs`
- `third_party/taffy/examples/grid_holy_grail.rs`

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
