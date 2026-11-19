//! A virtualised table of text cells, with the `Table`, `Row`, `ColumnHeader` and `Cell` roles
//! and row and column counts (the cfg.bin record view of the explorer).

use gpui::{
    Context, EventEmitter, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, Role,
    ScrollStrategy, SharedString, StatefulInteractiveElement, Styled, UniformListScrollHandle, Window, div,
    prelude::*, transparent_black, uniform_list,
};

use crate::a11y::Settings;

/// What the table tells its host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableEvent {
    /// The selection moved to this row.
    Selected(usize),
}

/// A table: columns and rows of text.
pub struct Table {
    label: SharedString,
    columns: Vec<SharedString>,
    rows: Vec<Vec<SharedString>>,
    selected: usize,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
}

impl EventEmitter<TableEvent> for Table {}

impl Table {
    /// An empty table; `label` names it for assistive technology.
    pub fn new(label: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            label: label.into(),
            columns: Vec::new(),
            rows: Vec::new(),
            selected: 0,
            focus: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
        }
    }

    /// Replaces the content.
    pub fn set(&mut self, columns: Vec<SharedString>, rows: Vec<Vec<SharedString>>, cx: &mut Context<Self>) {
        self.columns = columns;
        self.rows = rows;
        self.selected = self.selected.min(self.rows.len().saturating_sub(1));
        cx.notify();
    }

    /// The focus handle.
    #[must_use]
    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }

    /// The selected row.
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    fn move_to(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.rows.is_empty() {
            return;
        }
        let ix = ix.min(self.rows.len() - 1);
        self.scroll.scroll_to_item(ix, ScrollStrategy::Nearest);
        if ix != self.selected {
            self.selected = ix;
            cx.emit(TableEvent::Selected(ix));
        }
        cx.notify();
    }
}

impl Render for Table {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Settings::theme(cx).clone();
        let (row_count, col_count) = (self.rows.len(), self.columns.len());
        let row_height = theme.px(26.0);
        let header = div()
            .id("nie-table-header")
            .role(Role::Row)
            .aria_label("Columns")
            .aria_row_index(0)
            .flex()
            .w_full().h(row_height)
            .border_b_1()
            .border_color(theme.border.hsla())
            .text_color(theme.accent.hsla())
            .children(self.columns.iter().enumerate().map(|(c, title)| {
                div()
                    .id(("nie-table-col", c))
                    .role(Role::ColumnHeader)
                    .aria_label(title.clone())
                    .aria_column_index(c)
                    .flex_1()
                    .px(theme.px(8.0))
                    .flex()
                    .items_center()
                    .child(title.clone())
            }));
        div()
            .id("nie-table")
            .role(Role::Table)
            .aria_label(self.label.clone())
            .aria_row_count(row_count + 1)
            .aria_column_count(col_count)
            .track_focus(&self.focus)
            .tab_stop(true)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                match event.keystroke.key.as_str() {
                    "up" => this.move_to(this.selected.saturating_sub(1), cx),
                    "down" => this.move_to(this.selected + 1, cx),
                    "home" => this.move_to(0, cx),
                    "end" => this.move_to(usize::MAX, cx),
                    "pageup" => this.move_to(this.selected.saturating_sub(10), cx),
                    "pagedown" => this.move_to(this.selected + 10, cx),
                    _ => {}
                }
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.surface.hsla())
            .text_color(theme.text.hsla())
            .font_family(theme.ui_font)
            .text_size(theme.px(theme.text_size))
            .border_2()
            .border_color(transparent_black())
            .focus_visible(|s| s.border_color(theme.focus.hsla()))
            .child(header)
            .child(
                uniform_list(
                    "nie-table-rows",
                    row_count,
                    cx.processor(move |this, range: std::ops::Range<usize>, window, cx| {
                        let theme = Settings::theme(cx).clone();
                        let width = this.scroll.0.borrow().last_item_size.map(|s| s.item.width);
                        if width.is_none() {
                            // The list reports its width after its first layout: draw once more.
                            window.refresh();
                        }
                        range
                            .map(|r| {
                                let selected = r == this.selected;
                                div()
                                    .id(("nie-table-row", r))
                                    .role(Role::Row)
                                    .aria_label(this.rows[r].first().cloned().unwrap_or_default())
                                    .aria_row_index(r + 1)
                                    .aria_selected(selected)
                                    .when_some(width, |d, w| d.w(w)).h(row_height)
                                    .flex()
                                    .cursor_pointer()
                                    .when(selected, |d| d.bg(theme.selection.hsla()).text_color(theme.selection_text.hsla()))
                                    .hover(|d| d.bg(theme.surface_hover.hsla()))
                                    .on_click(cx.listener(move |this, _, _window, cx| this.move_to(r, cx)))
                                    .children(this.rows[r].iter().enumerate().map(|(c, text)| {
                                        div()
                                            .id(("nie-table-cell", r * 4096 + c))
                                            .role(Role::Cell)
                                            .aria_label(text.clone())
                                            .aria_column_index(c)
                                            .flex_1()
                                            .px(theme.px(8.0))
                                            .flex()
                                            .items_center()
                                            .overflow_hidden()
                                            .child(text.clone())
                                    }))
                            })
                            .collect()
                    }),
                )
                .track_scroll(&self.scroll)
                .size_full(),
            )
    }
}
