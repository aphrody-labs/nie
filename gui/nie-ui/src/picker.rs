//! A fuzzy picker over a list of strings, scored by `nucleo` (the matcher of helix): a query line
//! (`ComboBox`) over a result list (`ListBox` of `ListBoxOption`).

use gpui::{
    Context, EventEmitter, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render,
    Role, ScrollStrategy, SharedString, StatefulInteractiveElement, Styled, UniformListScrollHandle, Window, div,
    prelude::*, transparent_black, uniform_list,
};
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher};

use crate::a11y::Settings;

/// Results kept after scoring.
const MAX_RESULTS: usize = 200;

/// What the picker tells its host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerEvent {
    /// The user chose the item at this index of the original list.
    Chosen(usize),
    /// The user dismissed the picker.
    Dismissed,
}

struct Candidate<'a>(&'a str);

impl AsRef<str> for Candidate<'_> {
    fn as_ref(&self) -> &str {
        self.0
    }
}

/// The picker view.
pub struct Picker {
    label: SharedString,
    items: Vec<SharedString>,
    query: String,
    /// Indices into `items`, best first.
    results: Vec<usize>,
    selected: usize,
    matcher: Matcher,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
}

impl EventEmitter<PickerEvent> for Picker {}

impl Picker {
    /// A picker named `label` over `items`.
    pub fn new(label: impl Into<SharedString>, items: Vec<SharedString>, cx: &mut Context<Self>) -> Self {
        let mut picker = Self {
            label: label.into(),
            items,
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            matcher: Matcher::new(Config::DEFAULT),
            focus: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
        };
        picker.rescore();
        picker
    }

    /// The focus handle.
    #[must_use]
    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }

    /// The query typed so far.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Sets the query and rescores.
    pub fn set_query(&mut self, query: impl Into<String>, cx: &mut Context<Self>) {
        self.query = query.into();
        self.rescore();
        cx.notify();
    }

    /// The items best first for the current query, as indices into the original list.
    #[must_use]
    pub fn results(&self) -> &[usize] {
        &self.results
    }

    fn rescore(&mut self) {
        self.selected = 0;
        if self.query.is_empty() {
            self.results = (0..self.items.len().min(MAX_RESULTS)).collect();
            return;
        }
        let pattern = Pattern::parse(&self.query, CaseMatching::Smart, Normalization::Smart);
        let mut scored: Vec<(usize, u32)> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(ix, item)| {
                let mut hit = pattern.match_list([Candidate(item.as_ref())], &mut self.matcher);
                hit.pop().map(|(_, score)| (ix, score))
            })
            .collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        scored.truncate(MAX_RESULTS);
        self.results = scored.into_iter().map(|(ix, _)| ix).collect();
    }

    fn move_to(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.results.is_empty() {
            return;
        }
        self.selected = ix.min(self.results.len() - 1);
        self.scroll.scroll_to_item(self.selected, ScrollStrategy::Nearest);
        cx.notify();
    }

    fn on_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let ks = &event.keystroke;
        match ks.key.as_str() {
            "up" => self.move_to(self.selected.saturating_sub(1), cx),
            "down" => self.move_to(self.selected + 1, cx),
            "enter" => {
                if let Some(ix) = self.results.get(self.selected) {
                    cx.emit(PickerEvent::Chosen(*ix));
                }
            }
            "escape" => cx.emit(PickerEvent::Dismissed),
            "backspace" => {
                self.query.pop();
                self.rescore();
                cx.notify();
            }
            _ => {
                if !ks.modifiers.control
                    && !ks.modifiers.platform
                    && !ks.modifiers.alt
                    && let Some(text) = ks.key_char.as_deref()
                {
                    self.query.push_str(text);
                    self.rescore();
                    cx.notify();
                }
            }
        }
    }
}

impl Render for Picker {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Settings::theme(cx).clone();
        let count = self.results.len();
        let row_height = theme.px(26.0);
        div()
            .id("nie-picker")
            .role(Role::ComboBox)
            .aria_label(self.label.clone())
            .aria_value(SharedString::from(self.query.clone()))
            .aria_expanded(true)
            .track_focus(&self.focus)
            .tab_stop(true)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| this.on_key(event, cx)))
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
            .child(
                div()
                    .id("nie-picker-query")
                    .h(theme.px(34.0))
                    .px(theme.px(10.0))
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(theme.border.hsla())
                    .child(if self.query.is_empty() {
                        SharedString::from(format!("{}...", self.label))
                    } else {
                        SharedString::from(self.query.clone())
                    })
                    .text_color(if self.query.is_empty() { theme.text_dim.hsla() } else { theme.text.hsla() }),
            )
            .child(
                div()
                    .id("nie-picker-results")
                    .role(Role::ListBox)
                    .aria_label(SharedString::from(format!("{} results", self.label)))
                    .flex_1()
                    .min_h_0()
                    .child(
                        uniform_list(
                            "nie-picker-rows",
                            count,
                            cx.processor(move |this, range: std::ops::Range<usize>, window, cx| {
                                let theme = Settings::theme(cx).clone();
                                let width = this.scroll.0.borrow().last_item_size.map(|s| s.item.width);
                        if width.is_none() {
                            // The list reports its width after its first layout: draw once more.
                            window.refresh();
                        }
                                range
                                    .map(|pos| {
                                        let item = this.items[this.results[pos]].clone();
                                        let selected = pos == this.selected;
                                        div()
                                            .id(("nie-picker-row", pos))
                                            .role(Role::ListBoxOption)
                                            .aria_label(item.clone())
                                            .aria_selected(selected)
                                            .aria_position_in_set(pos + 1)
                                            .aria_size_of_set(count)
                                            .when_some(width, |d, w| d.w(w)).h(row_height)
                                            .px(theme.px(10.0))
                                            .flex()
                                            .items_center()
                                            .cursor_pointer()
                                            .when(selected, |d| {
                                                d.bg(theme.selection.hsla()).text_color(theme.selection_text.hsla())
                                            })
                                            .hover(|d| d.bg(theme.surface_hover.hsla()))
                                            .on_click(cx.listener(move |this, _, _window, cx| {
                                                this.selected = pos;
                                                if let Some(ix) = this.results.get(pos) {
                                                    cx.emit(PickerEvent::Chosen(*ix));
                                                }
                                            }))
                                            .child(item)
                                    })
                                    .collect()
                            }),
                        )
                        .track_scroll(&self.scroll)
                        .size_full(),
                    ),
            )
    }
}
