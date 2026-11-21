# bevy_sprite, bevy_sprite_render, bevy_text, bevy_ui (Bevy 0.20.1)

Pin: bevyengine/bevy commit `a0577b5`; `path:line (a0577b5)` relative to repo root. Snippets are "not compiled".
Web: learn hub https://bevy.org/learn/ (quick start, migration guides, API docs https://docs.rs/bevy), examples index https://bevy.org/examples/
(2D Rendering, UI categories; e.g. https://bevy.org/examples/2d-rendering/sprite-sheet/), release post https://bevy.org/news/bevy-0-20/ (dated 2026-10-08 on https://bevy.org/news/).

## Crate map
| Crate | Role (Cargo.toml `description` / source) |
|---|---|
| `bevy_sprite` | `Sprite`, `Anchor`, `SpriteImageMode`, `TextureSlicer`, sprite picking backend, `Text2d` component and layout (`crates/bevy_sprite/src/lib.rs:64`, `sprite.rs:17`, `text2d.rs:102`, `picking_backend.rs:78`) |
| `bevy_sprite_render` | renders it: `Sprite` becomes a `Mesh2d` quad + `SpriteMeshMaterial` (`sprite_mesh/mod.rs:37-60`), plus `Mesh2d`/`ColorMaterial`/`MeshMaterial2d`/`ExtendedMaterial2d`, tilemap chunks, wireframe2d, `Text2d` extraction (`crates/bevy_sprite_render/src/lib.rs:23-80`) |
| `bevy_image` | `TextureAtlasLayout`, `TextureAtlas`, `TextureAtlasBuilder`, `TextureAtlasSources` (`crates/bevy_image/src/texture_atlas.rs:33,104,214`, `texture_atlas_builder.rs:39`) |
| `bevy_text` | text engine on `parley` 0.11 (`crates/bevy_text/Cargo.toml:45`; no cosmic-text in `crates/*/Cargo.toml`), fonts, font atlases, `EditableText` (`crates/bevy_text/src/editing.rs:109`) |
| `bevy_ui` | ECS UI on taffy: `Node`, `Text`, `ImageNode`, layout, focus/interaction states (`crates/bevy_ui/src/ui_node.rs:485`, `widget/text.rs:128`, `widget/image.rs:18`) |
| `bevy_ui_render` | UI render pass (retained since 0.20, see below), box shadow, gradients, `UiMaterial`, texture slice pipeline |
| `bevy_ui_widgets` | headless, unstyled widgets: Button, Checkbox, Radio, Slider, Scrollbar, Menu, Popover, Tabs, text input (`crates/bevy_ui_widgets/src/lib.rs:1-3`, files `button.rs` ... `text_input.rs`) |
| `bevy_feathers` | styled widget toolkit for editors/tools, theme tokens; add `FeathersPlugins` (`crates/bevy_feathers/src/lib.rs:130-139`) |
| `bevy_input_focus` | `InputFocus`, `AutoFocus`, `TabIndex`/`TabGroup`, `TabNavigationPlugin`, directional navigation (`crates/bevy_input_focus/src/lib.rs:109,320,340`, `tab_navigation.rs:64,73,406`, `autofocus.rs:24`) |
| `bevy_picking` | `Pickable`, pointer events (`crates/bevy_picking/src/lib.rs:199`, `events.rs:192-413`) |

## Sprites
- `Sprite{image, texture_atlas: Option<TextureAtlas>, color, flip_x, flip_y, custom_size, rect, image_mode, alpha_mode}`; requires `Transform, Visibility, VisibilityClass, Anchor` (`sprite.rs:13-44`). Constructors `from_image`, `from_atlas_image(image, atlas)`, `from_color(color, size)`, `sized` (`:48-73`).
- `alpha_mode: SpriteAlphaMode::{Opaque, Mask(f32), Blend (default)}` is new in 0.20 (`sprite.rs:180-197`). Set `Opaque` or `Mask` for toon/pixel assets to avoid blend sorting.
- `Anchor(Vec2)` normalized pivot, constants `CENTER`, `BOTTOM_CENTER`, ... (`sprite.rs:303-323`); it is a required component, set it as a component, not a `Sprite` field. Demo: `examples/picking/sprite_picking.rs:30-60`.
- `SpriteImageMode::{Auto, Scale(SpriteScalingMode), Sliced(TextureSlicer), Tiled{tile_x,tile_y,stretch_value}}` (`sprite.rs:202-216`). 9-slice: `TextureSlicer{border: BorderRect, center_scale_mode, sides_scale_mode: SliceScaleMode::{Stretch,Tile{stretch_value}}, max_corner_scale}` (`texture_slice/slicer.rs:14-28`; `examples/2d/sprite_slice.rs:40-80`).
- Pixel art: `DefaultPlugins.set(ImagePlugin::default_nearest())` (`examples/2d/sprite_sheet.rs:8`). Per-image: `image.sampler = ImageSampler::nearest()`.
- Atlas: `TextureAtlasLayout::from_grid(tile_size: UVec2, columns, rows, padding: Option<UVec2>, offset: Option<UVec2>)` (`texture_atlas.rs:133`), `add_texture(URect) -> usize` (`:180`), `TextureAtlas{layout, index}` with `with_index` (`:214-235`). Packing loose images: `TextureAtlasBuilder::default().padding(..)`, `.add_texture(Some(id), &image)`, `.build() -> (layout, sources, Image)` (`examples/2d/texture_atlas.rs:224-242`; padding avoids bleeding with linear sampling); look up a sub-image with `TextureAtlasSources::texture_index(id)` (`texture_atlas.rs:40`).
- Sprite sheet animation = tick a `Timer` and set `sprite.texture_atlas.as_mut().unwrap().index` (`examples/2d/sprite_sheet.rs:19-39`, keyboard-driven variant `examples/2d/sprite_animation.rs`). No built-in sprite animation asset in `bevy_sprite`.
- Picking: `SpritePickingPlugin` is added by `SpritePlugin` when `bevy_picking` is on (`crates/bevy_sprite/src/lib.rs:97-98`). Only entities with `Pickable` are considered (`picking_backend.rs:53-60`); default mode `AlphaThreshold(0.1)` ignores transparent pixels, `BoundingBox` uses the rect (`:41-50,66-75`). Observers: `.observe(|over: On<PointerOver>| ..)` using flat events `PointerOver/PointerOut/PointerPress/PointerRelease/PointerClick/PointerDrag` (`examples/picking/sprite_picking.rs:62-67`; `crates/bevy_picking/src/events.rs`).
- 2D rendering features: custom sprite shaders via `SpriteMaterial<M>`/`MaterialExtension2d`, `ExtendedMaterial2d` (2D counterpart of `ExtendedMaterial`); see https://bevy.org/news/bevy-0-20/#sprite-materials and `crates/bevy_sprite_render/src/sprite_mesh/sprite_extended_material.rs:43,218`. Shader sources in this crate are `.wesl` (WESL), not `.wgsl`.

## Text
- Two entry points: `Text2d(String)` (world space, `bevy_sprite/src/text2d.rs:102`, requires `FontHinting::Disabled`; extras `Text2dShadow`, `TextBounds`, `examples/2d/text2d.rs`) and UI `Text(String)` (`bevy_ui/src/widget/text.rs:128`). Both use `TextFont`, `TextLayout{justify, linebreak}` (`bevy_text/src/text.rs:134`), `TextColor` (`:1405`), `TextBackgroundColor`, `Underline`/`Strikethrough`, `LineHeight`, `LetterSpacing`.
- Sections = child entities with `TextSpan(String)` (own `TextFont`/`TextColor`), valid only under a `Text`/`Text2d` parent (`text.rs:185-200`; `examples/ui/text/text.rs:70-136`). Inline content: `InlineBox` and `InlineImage` (`crates/bevy_text/src/inline_box.rs:9`, `examples/ui/text/inline_image.rs`).
- `TextFont{font: FontSource, font_size: FontSize, weight, width, style, font_smoothing, ..}` (`text.rs:671-700`). `FontSource::{Handle(Handle<Font>), Family(name), Families(css list), generic ...}` (`:284-300`); `.into()` from a handle works (`examples/ui/text/text.rs:39`). Load fonts as normal assets (`asset_server.load("fonts/X.ttf")`), `Font::from_bytes(Vec<u8>)` (`font.rs:30-47`). The built-in `FiraMono-subset.ttf` is used if the `default_font` feature is on (`text.rs:291-296`). System fonts need feature `parley/system` -> `system_font_discovery` (`bevy_text/Cargo.toml:13`; `examples/ui/text/system_fonts.rs`).
- A new glyph atlas is generated per (font, scaled size) (`text.rs:686-688,780-781`; `font_atlas.rs:32`; debug `examples/ui/text/font_atlas_debug.rs`): keep few sizes.
- 0.20 changes (https://bevy.org/learn/migration-guides/0-19-to-0-20/, files in `_release-content/migration-guides/`): `TextFont::default()` size is `FontSize::Rem(1.0)` (`default_font_size_is_rem.md`; `FontSize::{Px,Vw,Vh,VMin,VMax,Rem}` at `text.rs:783-796`, `RemSize` `:875`), so set `FontSize::Px(..)` for fixed size (`text.rs:762`); `FontSource` generic variants replaced by `GenericFontFamily` constructors (`font_source_generic_families.md`); `Font::from_bytes` drops the family-name arg (`font_from_bytes.md`); `resolve_font_source` removed (`resolve_font_source.md`); `EditableText` and `TextInput` are separate components (`text_input.md`); `TextReader` returns `(Entity, usize, TextLayoutItem)` (`text_reader_inline_box_changes.md`); `TextScroll` replaced by `EditableText::viewport` (`editable_text_scrolling.md`).

## UI
- `Node` is the layout component (`ui_node.rs:485`; workspace edition 2024, `Cargo.toml:4`); `Display::{Flex, Grid, Block, None}` (`:1457-1469`); grid example `examples/ui/layout/grid.rs:27-42`, flex `examples/ui/layout/flex_layout.rs`. Length helpers `px(..)`, `percent`, `vw`, `vh`, `em`, `rem` (`geometry.rs:592`); `Val::Em/Rem` are new and `Node` now requires `EmSize` (`val_em_and_rem.md`).
- `ImageNode{color, image, texture_atlas, flip_x, ..}` requires `Node, ImageNodeSize` (`widget/image.rs:15-31`); atlas in UI: `ImageNode::from_atlas_image(tex, TextureAtlas::from(layout))` (`examples/ui/images/ui_texture_atlas.rs:46-49`); 9-slice UI: `examples/ui/images/ui_texture_slice.rs`.
- `UiScale(f32)` multiplies fixed `Val::Px` (`bevy_ui/src/lib.rs:127-132`; `examples/ui/ui_scaling.rs`). `UiTargetCamera(Entity)` on a ROOT node picks the camera; without it the `IsDefaultUiCamera` camera or highest-order primary-window camera is used; `RenderLayers` do not apply to UI (`ui_node.rs:3395-3450`; `examples/ui/ui_target_camera.rs:20,70`). Render UI into a texture: `examples/ui/render_ui_to_texture.rs`, `ViewportNode` (`widget/viewport.rs:43`).
- Buttons: `bevy_ui::widget::Button` and `Interaction` are deprecated since 0.20.0 (`widget/button.rs:24`; `deprecate_button_and_interaction.md`). Use `bevy::ui_widgets::Button` (headless, emits `Activate` on release, keeps `Pressed`; `bevy_ui_widgets/src/button.rs:20-40`) + `picking::hover::Hovered` + `ui::Pressed` (`examples/ui/widgets/button.rs:27-30,78-80,109-145`); disable with `InteractionDisabled`; `ActivateOnPress` for menus. DefaultPlugins already include the widget plugins (`button.rs` example `:35`).
- Focus: add `bevy::input_focus::tab_navigation::TabNavigationPlugin`, wrap in `TabGroup`, give `TabIndex` (`examples/ui/widgets/button.rs:8-12`, `examples/ui/widgets/tab_navigation.rs`); `FocusCause::Auto` is new (`focus_cause_auto.md`); Escape in a text input releases focus (`escape_in_a_text_input_releases_focus_and_propagates.md`). Gamepad/keyboard navigation: `examples/ui/navigation/directional_navigation.rs`.
- Rendering is retained since 0.20 (only transparent UI is still re-extracted every frame): custom UI render code must read `ExtractedUiNodes::changed`; `ExtractedUiNode.clip` is `Option<CalculatedClip>`; `CalculatedClip` stores transformed rects (https://bevy.org/learn/migration-guides/0-19-to-0-20/, "UI rendering is now retained"; `calculated_clip_rects.md`). `BorderRadius` fields are 2D (`border_radius.md`). UI system sets in `PostUpdate`/`ExtractSchedule` use weak ordering (`weak_system_set_ordering.md`).
- Feathers: `app.add_plugins(FeathersPlugins)`; theme restructured around semantic tokens (`contextual_theming.md`); new widgets color input, list view/scrollbar, dropdown, lazy menu (https://bevy.org/news/bevy-0-20/#more-feathers-widgets); gallery `examples/ui/widgets/feathers_gallery.rs`. `cursor` module moved from feathers to `bevy_picking` (`cursor_module_to_bevy_picking.md`).

## 0.20 sprite migration summary
`Sprite` now renders through `Mesh2d` + a `Material2d` (new `alpha_mode`; same-Z draw order may differ; old backend kept only for `Text2d`) (`sprite_mesh_migration.md`). `SpriteMaterial`/`SpriteMaterialPlugin` internals renamed `SpriteMeshMaterial`/`SpriteMeshMaterialPlugin`, while the new public `SpriteMaterial<M>` wraps custom materials (`sprite_material_rename.md`, `sprite_extended_material.rs:218`). Give overlapping sprites explicit Z.

## Recipes for our GLB pipeline (not compiled)
Preview of a rendered sprite sheet (frames in columns, one direction per row, produced by the 8-direction offscreen capture in render.md). One sprite per direction animates its row:

```rust
use bevy::prelude::*;

#[derive(Component)] struct Row { first: usize, frames: usize }
#[derive(Component, Deref, DerefMut)] struct Tick(Timer);

fn setup(mut commands: Commands, assets: Res<AssetServer>, mut layouts: ResMut<Assets<TextureAtlasLayout>>) {
    let (tile, columns, rows) = (UVec2::splat(128), 8u32, 8u32); // read from the manifest the exporter writes
    let layout = layouts.add(TextureAtlasLayout::from_grid(tile, columns, rows, None, None));
    let sheet = assets.load("sheets/hero.png");
    commands.spawn(Camera2d);
    for row in 0..rows {
        commands.spawn((
            Sprite::from_atlas_image(sheet.clone(), TextureAtlas { layout: layout.clone(), index: (row * columns) as usize }),
            Transform::from_xyz((row as f32 - 3.5) * 140.0, 0.0, 0.0),
            Row { first: (row * columns) as usize, frames: columns as usize },
            Tick(Timer::from_seconds(1.0 / 12.0, TimerMode::Repeating)),
        ));
    }
}

fn animate(time: Res<Time>, mut q: Query<(&Row, &mut Tick, &mut Sprite)>) {
    for (row, mut tick, mut sprite) in &mut q {
        if tick.tick(time.delta()).just_finished() && let Some(atlas) = &mut sprite.texture_atlas {
            atlas.index = row.first + (atlas.index - row.first + 1) % row.frames;
        }
    }
}
// App: DefaultPlugins.set(ImagePlugin::default_nearest()); Startup setup; Update animate.
```
Uncertainty: `let` chains in `if` need edition 2024 (the checkout example does the same at `examples/2d/sprite_sheet.rs:29-31`); API for `Sprite::from_atlas_image`/`TextureAtlas{layout,index}` verified against the checkout, whole snippet not compiled.
