//! Tabs: a tab list with the selected tab's panel, `TabList`, `Tab` and `TabPanel` roles.

use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, Role, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::*,
};

use crate::a11y::Settings;

type OnSelect = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// A tab strip with the content of the selected tab.
#[derive(IntoElement)]
pub struct Tabs {
    label: SharedString,
    titles: Vec<SharedString>,
    selected: usize,
    on_select: Option<OnSelect>,
    content: Option<AnyElement>,
}

impl Tabs {
    /// Tabs named `label` with the given titles; `selected` is the open one.
    pub fn new(label: impl Into<SharedString>, titles: Vec<SharedString>, selected: usize) -> Self {
        Self { label: label.into(), titles, selected, on_select: None, content: None }
    }

    /// Called with the index of a clicked tab.
    #[must_use]
    pub fn on_select(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    /// The content of the selected tab.
    #[must_use]
    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }
}

impl RenderOnce for Tabs {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Settings::theme(cx).clone();
        let count = self.titles.len();
        let selected_title = self.titles.get(self.selected).cloned().unwrap_or_default();
        let strip = div()
            .id("nie-tablist")
            .role(Role::TabList)
            .aria_label(self.label.clone())
            .flex()
            .border_b_1()
            .border_color(theme.border.hsla())
            .children(self.titles.into_iter().enumerate().map(|(ix, title)| {
                let on_select = self.on_select.clone();
                let open = ix == self.selected;
                div()
                    .id(("nie-tab", ix))
                    .role(Role::Tab)
                    .aria_label(title.clone())
                    .aria_selected(open)
                    .aria_position_in_set(ix + 1)
                    .aria_size_of_set(count)
                    .focusable()
                    .tab_stop(true)
                    .px(theme.px(14.0))
                    .py(theme.px(6.0))
                    .cursor_pointer()
                    .border_b_2()
                    .border_color(if open { theme.accent.hsla() } else { theme.border.hsla().opacity(0.0) })
                    .text_color(if open { theme.text.hsla() } else { theme.text_dim.hsla() })
                    .hover(|d| d.bg(theme.surface_hover.hsla()))
                    .focus_visible(|d| d.border_color(theme.focus.hsla()))
                    .when_some(on_select, |d, f| d.on_click(move |_, window, cx| f(ix, window, cx)))
                    .child(title)
            }));
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background.hsla())
            .text_color(theme.text.hsla())
            .font_family(theme.ui_font)
            .text_size(theme.px(theme.text_size))
            .child(strip)
            .child(
                div()
                    .id("nie-tabpanel")
                    .role(Role::TabPanel)
                    .aria_label(selected_title)
                    .flex_1()
                    .min_h_0()
                    .children(self.content),
            )
    }
}
