---
name: bevy-feathers
description: "Use when working with the `bevy_feathers` crate of bevy: A collection of UI widgets for building editors and utilities in Bevy."
---

# bevy_feathers

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_feathers/` (manifest `third_party/bevy/crates/bevy_feathers/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_feathers
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

A collection of UI widgets for building editors and utilities in Bevy

## Crate docs (`third_party/bevy/crates/bevy_feathers/src/lib.rs`)

`bevy_feathers` is a collection of styled and themed widgets for building editors and
inspectors.

The aesthetic choices made here are designed with a future Bevy Editor in mind,
but this crate is deliberately exposed to the public to allow the broader ecosystem to easily create
tooling for themselves and others that fits cohesively together.

While it may be tempting to use this crate for your game's UI, it's deliberately not intended for that.
We've opted for a clean, functional style, and prioritized consistency over customization.
That said, if you like what you see, it can be a helpful learning tool.
Consider copying this code into your own project,
and refining the styles and abstractions provided to meet your needs.

## Best practices for event propagation

Generally, when a widget handles an event,
propagation of that event to parent entities should be stopped.
This is important when writing your custom widgets, and understanding the behavior of existing widgets.

For more guidance on this, see the documentation for [`EntityEvent`](bevy_ecs::event::EntityEvent).

## Cargo features (from the manifest)

- `default`
- `bevy_settings` (enables 1)
- `webgl`
- `webgpu`

## Depends on (same ecosystem)

`bevy-settings`, `bevy_a11y`, `bevy_app`, `bevy_asset`, `bevy_camera`, `bevy_color`, `bevy_derive`, `bevy_ecs`, `bevy_input`, `bevy_input_focus`, `bevy_log`, `bevy_math`, `bevy_picking`, `bevy_platform`, `bevy_reflect`, `bevy_render`, `bevy_scene`, `bevy_shader`, `bevy_text`, `bevy_ui`, `bevy_ui_render`, `bevy_ui_widgets`, `bevy_window`

## Public items by file

- `third_party/bevy/crates/bevy_feathers/src/alpha_pattern.rs`: struct AlphaPatternPlugin
- `third_party/bevy/crates/bevy_feathers/src/constants.rs`: mod fonts, mod icons, mod size
- `third_party/bevy/crates/bevy_feathers/src/containers/flex_spacer.rs`: fn flex_spacer
- `third_party/bevy/crates/bevy_feathers/src/containers/group.rs`: fn group, fn group_header, fn group_body
- `third_party/bevy/crates/bevy_feathers/src/containers/pane.rs`: fn pane, fn pane_header, fn pane_header_divider, fn pane_body
- `third_party/bevy/crates/bevy_feathers/src/containers/subpane.rs`: fn subpane, fn subpane_header, fn subpane_body
- `third_party/bevy/crates/bevy_feathers/src/controls/button.rs`: enum ButtonVariant, struct FeathersButton, struct FeathersButtonProps, struct FeathersToolButton, struct ButtonPlugin
- `third_party/bevy/crates/bevy_feathers/src/controls/checkbox.rs`: struct FeathersCheckbox, struct FeathersCheckboxProps, struct CheckboxPlugin
- `third_party/bevy/crates/bevy_feathers/src/controls/color_input.rs`: struct ColorInputValue, enum ColorInputMode, struct ColorInputSettings, struct FeathersColorInput, struct FeathersColorInputProps, struct ColorInputPlugin, fn color_input_self_update
- `third_party/bevy/crates/bevy_feathers/src/controls/color_plane.rs`: enum FeathersColorPlane, struct ColorPlaneValue, struct ColorPlanePlugin
- `third_party/bevy/crates/bevy_feathers/src/controls/color_slider.rs`: enum ColorChannel, struct SliderBaseColor, struct FeathersColorSlider, struct FeathersColorSliderProps, struct ColorSlider, struct ColorSliderPlugin
- `third_party/bevy/crates/bevy_feathers/src/controls/color_swatch.rs`: struct FeathersColorSwatch, struct FeathersColorSwatchProps, struct ColorSwatchValue, struct ColorSwatchFg, struct ColorSwatchPlugin
- `third_party/bevy/crates/bevy_feathers/src/controls/color_swatch_grid.rs`: struct FeathersColorSwatchGrid, struct FeathersColorSwatchGridCell, struct ColorSwatchGridCellRing, struct FeathersColorSwatchEmptyCell, struct ColorSwatchGridUpdate, struct ColorSwatchGridPlugin
- `third_party/bevy/crates/bevy_feathers/src/controls/color_wheel.rs`: struct FeathersColorWheel, struct ColorWheelValue, struct ColorWheelPlugin
- `third_party/bevy/crates/bevy_feathers/src/controls/dialog.rs`: struct FeathersDialogProps, struct FeathersDialog, struct FeathersFloatingDialogProps, struct FeathersFloatingDialog, struct FeathersDialogHeader, struct FeathersDialogClose, struct FeathersDialogBody, struct FeathersDialogFooter
- `third_party/bevy/crates/bevy_feathers/src/controls/disclosure_toggle.rs`: struct FeathersDisclosureToggle, struct DisclosureTogglePlugin
- `third_party/bevy/crates/bevy_feathers/src/controls/listview.rs`: struct FeathersListView, struct FeathersListViewProps, struct FeathersListRow, struct ListViewPlugin
- `third_party/bevy/crates/bevy_feathers/src/controls/menu.rs`: struct FeathersMenu, struct FeathersLazyMenu, struct FeathersMenuButton, struct FeathersMenuButtonProps, struct FeathersMenuToolButton, struct FeathersMenuPopup, struct FeathersMenuItem, struct FeathersMenuItemProps, struct FeathersMenuDivider, struct MenuPlugin
- `third_party/bevy/crates/bevy_feathers/src/controls/mod.rs`: struct ControlsPlugin
- `third_party/bevy/crates/bevy_feathers/src/controls/number_input.rs`: struct FeathersNumberInput, struct FeathersNumberInputProps, struct SoftLimit, struct HardLimit, struct NumberInputPrecision, struct NumberInputStep, struct NumberInputSnapToStep, enum NumberInputWrap, struct NumberInputUnits, trait UnitsFormat, fn format_display_value, struct Dimensionless, struct StandardUnitItem, trait StandardUnitKind, struct LengthMeters, struct TimeSeconds, struct AngleDegrees, struct RatioPercent, struct UnitsRegistry, struct NumberInputPlugin
- `third_party/bevy/crates/bevy_feathers/src/controls/radio.rs`: struct FeathersRadio, struct FeathersRadioProps, struct RadioPlugin
- `third_party/bevy/crates/bevy_feathers/src/controls/scrollbar.rs`: struct FeathersScrollbar, struct FeathersScrollbarProps, struct ScrollbarGutter, struct ScrollbarPlugin
- `third_party/bevy/crates/bevy_feathers/src/controls/select.rs`: struct FeathersSelect, struct OptionIndex, fn list_rows_from_strings, struct FeathersSelectProps, struct SelectPlugin
- `third_party/bevy/crates/bevy_feathers/src/controls/slider.rs`: struct FeathersSlider, struct FeathersSliderProps, struct SliderPlugin
- `third_party/bevy/crates/bevy_feathers/src/controls/split_pane.rs`: struct FeathersSplitPane, struct FeathersSplitPaneProps, struct FeathersPane, struct FeathersPaneProps, struct FeathersSplitPaneHandle, struct SplitPanePlugin
- `third_party/bevy/crates/bevy_feathers/src/controls/tabs.rs`: struct FeathersTabList, struct FeathersTabListProps, struct TabFillets, struct FeathersTabListStartAdornment, struct FeathersTabListEndAdornment, struct FeathersTab, struct FeathersTabProps, enum TabFilletEdge, struct FeathersTabInsertionIndicator, struct FeathersTabDragProxy, struct FeathersTabsPlugin
- `third_party/bevy/crates/bevy_feathers/src/controls/text_input.rs`: struct FeathersTextInputContainer, struct FeathersTextInput, struct FeathersTextInputProps, struct TextInputPlugin
- `third_party/bevy/crates/bevy_feathers/src/controls/toggle_switch.rs`: struct FeathersToggleSwitch, struct ToggleSwitchPlugin
- `third_party/bevy/crates/bevy_feathers/src/controls/tree_view.rs`: struct FeathersTreeView, struct FeathersTreeViewProps, struct FeathersTreeItem, struct FeathersTreeItemProps, struct FeathersTreeItemHeader, struct FeathersTreeItemToggle, struct FeathersTreeItemChevron, struct FeathersTreeItemChildren, struct TreeViewPlugin
- `third_party/bevy/crates/bevy_feathers/src/controls/virtual_keyboard.rs`: struct VirtualKeyboard, struct VirtualKeyboardProps, struct VirtualKeyPressed
- `third_party/bevy/crates/bevy_feathers/src/dark_theme.rs`: fn create_dark_theme
- `third_party/bevy/crates/bevy_feathers/src/display/icon.rs`: struct ThemedIcon, fn icon, fn icon_untinted
- `third_party/bevy/crates/bevy_feathers/src/display/label.rs`: fn caption, fn label, fn label_dim, fn label_small
- `third_party/bevy/crates/bevy_feathers/src/focus.rs`: struct FocusIndicator, struct FocusWithinIndicator, struct FocusOutlinesPlugin
- `third_party/bevy/crates/bevy_feathers/src/font_styles.rs`: struct InheritableFont
- `third_party/bevy/crates/bevy_feathers/src/lib.rs`: mod constants, mod containers, mod controls, mod dark_theme, mod display, mod focus, mod font_styles, mod palette, mod rounded_corners, mod theme, mod tokens, struct FeathersSystems, struct FeathersCorePlugin, struct FeathersPlugins
- `third_party/bevy/crates/bevy_feathers/src/rounded_corners.rs`: enum RoundedCorners
- `third_party/bevy/crates/bevy_feathers/src/theme.rs`: enum SurfaceLevel, struct ThemeContext, struct ThemeToken, struct SemanticToken, struct ThemeProps, struct UiTheme, struct ThemeBackgroundColor, struct ThemeBorderColor, struct InheritableThemeTextColor, struct ThemeTextColor, struct ThemedText
- `third_party/bevy/crates/bevy_feathers/src/tokens.rs`: mod semantic

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
