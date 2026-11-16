---
name: bevy-text
description: "Use when working with the `bevy_text` crate of bevy: Provides text functionality for Bevy Engine."
---

# bevy_text

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_text/` (manifest `third_party/bevy/crates/bevy_text/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_text
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Provides text functionality for Bevy Engine

## README (`third_party/bevy/crates/bevy_text/README.md`)

# Bevy Text

## Cargo features (from the manifest)

- `default_font`
- `system_font_discovery` (enables 1)
- `system_clipboard` (enables 1)
- `complex_script_segmentation` (enables 1)

## Depends on (same ecosystem)

`bevy_app`, `bevy_asset`, `bevy_clipboard`, `bevy_color`, `bevy_derive`, `bevy_ecs`, `bevy_image`, `bevy_log`, `bevy_math`, `bevy_platform`, `bevy_reflect`, `bevy_utils`

## Public items by file

- `third_party/bevy/crates/bevy_text/src/bounds.rs`: struct TextBounds
- `third_party/bevy/crates/bevy_text/src/cursor.rs`: struct TextCursorStyle
- `third_party/bevy/crates/bevy_text/src/editing.rs`: struct EditableText, struct MaskState, struct Placeholder, enum PlaceholderMode, struct EditableTextGeneration, struct EditableTextFilter, enum TextReadWriteMode, fn apply_text_edits, struct TextEditChange
- `third_party/bevy/crates/bevy_text/src/error.rs`: enum TextError
- `third_party/bevy/crates/bevy_text/src/font.rs`: struct Font, fn load_font_assets_into_font_collection
- `third_party/bevy/crates/bevy_text/src/font_atlas.rs`: struct GlyphCacheKey, struct FontAtlas, fn add_glyph_to_atlas, fn get_outlined_glyph_texture, fn get_glyph_atlas_info
- `third_party/bevy/crates/bevy_text/src/font_atlas_set.rs`: struct FontAtlasKey, struct FontAtlasSet
- `third_party/bevy/crates/bevy_text/src/font_loader.rs`: struct FontLoader, enum FontLoaderError
- `third_party/bevy/crates/bevy_text/src/glyph.rs`: struct PositionedGlyph, struct GlyphAtlasInfo, struct GlyphAtlasLocation
- `third_party/bevy/crates/bevy_text/src/inline_box.rs`: struct InlineBox, enum InlineBoxKind
- `third_party/bevy/crates/bevy_text/src/lib.rs`: mod prelude, struct TextPlugin, struct Text2dUpdateSystems, struct EditableTextSystems
- `third_party/bevy/crates/bevy_text/src/parley_context.rs`: struct TextBrush, struct FontCx, struct LayoutCx, struct ScaleCx
- `third_party/bevy/crates/bevy_text/src/pipeline.rs`: struct TextPipeline, enum TextElement, struct TextLayoutInfo, struct RunGeometry, struct TextMeasureInfo
- `third_party/bevy/crates/bevy_text/src/scroll.rs`: struct TextViewport, struct TextLineYBounds, fn scrollable_text_layout_width
- `third_party/bevy/crates/bevy_text/src/text.rs`: struct TextEntity, struct ComputedTextBlock, struct TextLayout, struct TextSpan, enum Justify, enum FontSource, struct DefaultFontSource, enum GenericFontFamily, struct TextFont, enum FontSize, struct RemSize, struct EmSize, struct FontWeight, struct FontWidth, enum FontStyle, struct FontFeatureTag, struct FontFeatures, struct FontFeaturesBuilder, struct FontVariationTag, struct FontVariations, struct FontVariationsBuilder, enum LineHeight, enum LetterSpacing, struct TextColor, ...
- `third_party/bevy/crates/bevy_text/src/text_access.rs`: trait TextSection, struct TextIterScratch, struct TextReader, struct TextSpanIter, struct TextWriter
- `third_party/bevy/crates/bevy_text/src/text_edit.rs`: struct PreeditCursor, enum TextEdit, fn cursor_reveal_rect

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
