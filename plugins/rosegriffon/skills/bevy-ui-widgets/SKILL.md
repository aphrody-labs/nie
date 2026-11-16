---
name: bevy-ui-widgets
description: "Use when working with the `bevy_ui_widgets` crate of bevy: Unstyled common widgets for Bevy Engine."
---

# bevy_ui_widgets

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_ui_widgets/` (manifest `third_party/bevy/crates/bevy_ui_widgets/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_ui_widgets
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Unstyled common widgets for Bevy Engine

## Crate docs (`third_party/bevy/crates/bevy_ui_widgets/src/lib.rs`)

This crate provides a set of standard widgets for Bevy UI, such as buttons, checkboxes, and sliders.
These widgets have no inherent styling, it's the responsibility of the user to add styling
appropriate for their game or application.

## State Management

The typical way that UI widgets are used in games is that they often display a view of live
data in the engine - they aren't just passive form fields to fill in and then submit.
For example, a "color picker" might have multiple ways to edit an RGB value, including
sliders, a color plane, a color wheel, hex input, or swatches of recent colors. Interacting with
any of these widgets not only updates the RGB model, but also updates all of the other widgets.

For this reason, almost all of the widgets use **external** state management: this means that
the widgets do not automatically update their own internal state, but instead rely on the app
to update the widget state (as well as any other related game state) in response to a change
event emitted by the widget.

These widgets emit a [`ValueChange`] event which carries the proposed new state; the app should respond
to this event by updating its own internal model, and by updating the widget state. The general
convention is that the widget state is contained in a specific state-bearing component
(like [`SliderValue`]), and the widget detects when this component is inserted or replaced.

This design pattern draws from the classic "MVC" (Model / View / Controller) style of
widget design, which is also used by React.js for
[controlled](https://medium.com/@rupalsinghal/controlled-vs-uncontrolled-components-in-react-the-complete-guide-you-cant-afford-to-miss-fbf6ea28b0fd)
widgets.

There are a few exceptions to this rule:
* Buttons and menu items don't have any state, so they emit an [`Activate`] event instead.
  (The hover and pressed states don't count, because they are purely visual / stylistic).
* Text input widgets have a large and expensive state, so they handle their own state updates.
* Scrollbars don't emit events, they modify the scroll position of the target entity
  directly.

For users who don't want to bother with writing a state update handler, these widgets provide
a "self update" observer function: for example, the [`checkbox_self_update`] observer will
listen for the [`ValueChange`] event and update the checkbox state. This effectively converts
it into an "uncontrolled" widget, in React.js terms.

## Best practices for event propagation

Generally, when a widget handles an event,
propagation of that event to parent entities should be stopped. Events which are not of
interest to the widget should be allowed to propagate.
This "consume what you use" principle is important when writing your custom widgets, and
understanding the behavior of existing widgets.

For more guidance on this, see the documentation for [`EntityEvent`].

## Cargo features (from the manifest)

- `default`
- `serialize` (enables 1)

## Depends on (same ecosystem)

`bevy_a11y`, `bevy_app`, `bevy_camera`, `bevy_ecs`, `bevy_input`, `bevy_input_focus`, `bevy_log`, `bevy_math`, `bevy_picking`, `bevy_platform`, `bevy_reflect`, `bevy_text`, `bevy_time`, `bevy_ui`, `bevy_window`

## Public items by file

- `third_party/bevy/crates/bevy_ui_widgets/src/button.rs`: struct Button, struct ActivateOnPress, struct ButtonPlugin
- `third_party/bevy/crates/bevy_ui_widgets/src/checkbox.rs`: struct Checkbox, struct SetChecked, struct ToggleChecked, struct CheckboxPlugin, fn checkbox_self_update
- `third_party/bevy/crates/bevy_ui_widgets/src/dialog.rs`: struct Dialog, struct RequestClose, struct DialogDragHandle, struct DialogDragState, struct DialogStack, struct DialogPlugin
- `third_party/bevy/crates/bevy_ui_widgets/src/dock/tree.rs`: struct DockNodeId, struct DockTabId, struct DockPanelKey, enum DockEdge, struct DockTab, struct DockLeaf, struct DockSplitChild, struct DockSplit, enum DockNode, enum DockError, struct DockTree
- `third_party/bevy/crates/bevy_ui_widgets/src/dock/ui.rs`: struct DockPlugin, enum DockSystems, struct DockNodeBinding, struct DockTabBinding, struct DockPanelContent, struct DockPanelTitle, struct DockTabTitle, struct DockChange, enum DockEdit, fn dock_self_update
- `third_party/bevy/crates/bevy_ui_widgets/src/drag_proxy.rs`: struct DragOverlayRoot, struct DragProxy, struct DragProxyPlugin
- `third_party/bevy/crates/bevy_ui_widgets/src/lib.rs`: mod popover, struct UiWidgetsPlugins, struct Activate, struct ValueChange
- `third_party/bevy/crates/bevy_ui_widgets/src/list.rs`: struct ListBox, struct ListBoxMultiSelect, struct ListItem, struct ActiveDescendant, struct ReselectListRow, struct SetSelected, struct ListBoxPlugin, fn listbox_update_selection
- `third_party/bevy/crates/bevy_ui_widgets/src/menu.rs`: enum MenuAction, struct MenuEvent, enum MenuLayout, struct MenuPopup, struct MenuItem, enum MenuFocusState, struct MenuFocusSystem, struct MenuButton, struct MenuPlugin
- `third_party/bevy/crates/bevy_ui_widgets/src/modal.rs`: struct ModalDialog, struct ModalDialogBarrier, struct ModalDialogPlugin
- `third_party/bevy/crates/bevy_ui_widgets/src/numeric.rs`: enum NumericFormat, enum NumericValue, enum NumericRange
- `third_party/bevy/crates/bevy_ui_widgets/src/observe.rs`: struct AddObserver, fn observe
- `third_party/bevy/crates/bevy_ui_widgets/src/popover.rs`: enum PopoverSide, enum PopoverAlign, struct PopoverPlacement, struct Popover, struct PopoverPlugin
- `third_party/bevy/crates/bevy_ui_widgets/src/radio.rs`: struct RadioGroup, struct RadioButton, struct RadioGroupPlugin, fn radio_self_update
- `third_party/bevy/crates/bevy_ui_widgets/src/scrollarea.rs`: struct ScrollArea, struct ScrollAreaPlugin
- `third_party/bevy/crates/bevy_ui_widgets/src/scrollbar.rs`: enum ControlOrientation, struct Scrollbar, struct ScrollbarThumb, struct ScrollbarDragState, struct ScrollbarPlugin
- `third_party/bevy/crates/bevy_ui_widgets/src/slider.rs`: enum SliderOrientation, enum TrackClick, struct Slider, struct SliderThumb, struct SliderValue, struct SliderRange, struct SliderStep, struct SliderPrecision, struct SliderDragState, struct SetSliderValue, enum SliderValueChange, fn slider_self_update, struct SliderPlugin
- `third_party/bevy/crates/bevy_ui_widgets/src/split_pane.rs`: struct SplitPane, struct Pane, struct SplitPaneHandle, struct SplitPaneDragState, fn split_pane_self_update, struct SplitPanePlugin
- `third_party/bevy/crates/bevy_ui_widgets/src/tabs.rs`: enum TabActivation, enum TabDragMode, struct TabList, struct SelectedTab, struct Tab, struct TabLocked, struct TabDragging, struct TabInsertionPoint, struct TabInsertionPreview, struct TabMoved, struct TabDrop, enum TabDragPhase, struct TabDragLifecycle, struct TabPlugin, fn tablist_self_update
- `third_party/bevy/crates/bevy_ui_widgets/src/text_input.rs`: struct TextInput, struct SelectAllOnFocus, enum ImeSystems, struct TextInputPlugin
- `third_party/bevy/crates/bevy_ui_widgets/src/tree.rs`: enum TreeSelectionMode, struct TreeView, struct SelectedTreeItem, struct TreeItem, struct TreeItemChildren, struct TreeItemToggle, struct TreeItemActivate, struct TreeItemExpandChange, struct TreePlugin, fn tree_view_self_update, fn tree_view_expand_self_update

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
