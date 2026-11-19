//! A virtualised tree: flat rows with a depth, expandable branches, keyboard navigation and the
//! `Tree` and `TreeItem` roles with level, position in set and expanded state.

use gpui::{
    Context, EventEmitter, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    Render, Role, ScrollStrategy, SharedString, StatefulInteractiveElement, Styled, UniformListScrollHandle,
    Window, div, prelude::*, transparent_black, uniform_list,
};

use crate::a11y::Settings;

/// One visible row of the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRow {
    /// A stable identifier of the node, for the host to map events back.
    pub id: SharedString,
    /// The text of the row, also its accessible name.
    pub label: SharedString,
    /// Nesting level, 0 for the roots.
    pub depth: usize,
    /// Whether the node can be expanded.
    pub branch: bool,
    /// Whether a branch is open.
    pub expanded: bool,
}

/// What the tree tells its host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeEvent {
    /// The selection moved to the row at this index.
    Selected(usize),
    /// A branch was opened or closed; the host rebuilds the rows.
    Toggled(usize),
    /// Enter or double activation on the row.
    Activated(usize),
}

/// The tree view.
pub struct Tree {
    label: SharedString,
    rows: Vec<TreeRow>,
    selected: usize,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
}

impl EventEmitter<TreeEvent> for Tree {}

impl Tree {
    /// An empty tree; `label` names it for assistive technology.
    pub fn new(label: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            label: label.into(),
            rows: Vec::new(),
            selected: 0,
            focus: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
        }
    }

    /// Replaces the visible rows, keeping the selection in range.
    pub fn set_rows(&mut self, rows: Vec<TreeRow>, cx: &mut Context<Self>) {
        self.rows = rows;
        self.selected = self.selected.min(self.rows.len().saturating_sub(1));
        cx.notify();
    }

    /// The focus handle, to give the tree the keyboard focus.
    #[must_use]
    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }

    /// The selected row index.
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The rows being shown.
    #[must_use]
    pub fn rows(&self) -> &[TreeRow] {
        &self.rows
    }

    /// Moves the selection to `ix`.
    pub fn select(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.rows.is_empty() {
            return;
        }
        let ix = ix.min(self.rows.len() - 1);
        if ix != self.selected {
            self.selected = ix;
            self.scroll.scroll_to_item(ix, ScrollStrategy::Nearest);
            cx.emit(TreeEvent::Selected(ix));
        }
        cx.notify();
    }

    fn on_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let last = self.rows.len().saturating_sub(1);
        match event.keystroke.key.as_str() {
            "up" => self.select(self.selected.saturating_sub(1), cx),
            "down" => self.select((self.selected + 1).min(last), cx),
            "home" => self.select(0, cx),
            "end" => self.select(last, cx),
            "pageup" => self.select(self.selected.saturating_sub(10), cx),
            "pagedown" => self.select((self.selected + 10).min(last), cx),
            "right" => {
                if self.rows.get(self.selected).is_some_and(|r| r.branch && !r.expanded) {
                    cx.emit(TreeEvent::Toggled(self.selected));
                } else {
                    self.select((self.selected + 1).min(last), cx);
                }
            }
            "left" => {
                if let Some(row) = self.rows.get(self.selected) {
                    if row.branch && row.expanded {
                        cx.emit(TreeEvent::Toggled(self.selected));
                    } else if let Some(parent) = (0..self.selected).rev().find(|i| self.rows[*i].depth < row.depth) {
                        self.select(parent, cx);
                    }
                }
            }
            "enter" => cx.emit(TreeEvent::Activated(self.selected)),
            _ => {}
        }
    }
}

impl Render for Tree {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Settings::theme(cx).clone();
        let total = self.rows.len();
        let row_height = theme.px(26.0);
        div()
            .id("nie-tree")
            .role(Role::Tree)
            .aria_label(self.label.clone())
            .track_focus(&self.focus)
            .tab_stop(true)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| this.on_key(event, cx)))
            .size_full()
            .bg(theme.surface.hsla())
            .text_color(theme.text.hsla())
            .font_family(theme.ui_font)
            .text_size(theme.px(theme.text_size))
            .border_2()
            .border_color(transparent_black())
            .focus_visible(|s| s.border_color(theme.focus.hsla()))
            .child(
                uniform_list(
                    "nie-tree-rows",
                    total,
                    cx.processor(move |this, range: std::ops::Range<usize>, window, cx| {
                        let theme = Settings::theme(cx).clone();
                        let width = this.scroll.0.borrow().last_item_size.map(|s| s.item.width);
                        if width.is_none() {
                            // The list reports its width after its first layout: draw once more.
                            window.refresh();
                        }
                        range
                            .map(|ix| {
                                let row = &this.rows[ix];
                                let selected = ix == this.selected;
                                let marker = match (row.branch, row.expanded) {
                                    (true, true) => "v",
                                    (true, false) => ">",
                                    (false, _) => " ",
                                };
                                div()
                                    .id(("nie-tree-row", ix))
                                    .role(Role::TreeItem)
                                    .aria_label(row.label.clone())
                                    .aria_level(row.depth + 1)
                                    .aria_position_in_set(ix + 1)
                                    .aria_size_of_set(total)
                                    .aria_selected(selected)
                                    .when(row.branch, |d| d.aria_expanded(row.expanded))
                                    .when_some(width, |d, w| d.w(w)).h(row_height)
                                    .pl(theme.px(8.0 + 16.0 * row.depth as f32))
                                    .flex()
                                    .items_center()
                                    .gap(theme.px(6.0))
                                    .cursor_pointer()
                                    .when(selected, |d| d.bg(theme.selection.hsla()).text_color(theme.selection_text.hsla()))
                                    .hover(|d| d.bg(theme.surface_hover.hsla()))
                                    .on_click(cx.listener(move |this, _, _window, cx| {
                                        this.select(ix, cx);
                                        if this.rows.get(ix).is_some_and(|r| r.branch) {
                                            cx.emit(TreeEvent::Toggled(ix));
                                        }
                                    }))
                                    .child(marker)
                                    .child(row.label.clone())
                            })
                            .collect()
                    }),
                )
                .track_scroll(&self.scroll)
                .size_full(),
            )
    }
}
