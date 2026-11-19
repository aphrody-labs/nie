//! The explorer view: three columns (parent directory, current directory, preview) drawn from
//! plain data. The data comes from a [`Backend`]: on native builds the `nie_explore::fm` model
//! shared with `nie fm`, in the browser an HTTP producer over `nie serve`. The view never knows
//! which one it talks to.

use gpui::{
    Context, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, Role, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::*, transparent_black,
};

use crate::a11y::Settings;
use crate::surface::{ImageSurface, Rgba8, render_image};
use crate::theme::Syntax;

/// Directory or file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// A directory.
    Dir,
    /// A file.
    File,
}

/// One row of a column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The name shown, also the accessible name.
    pub name: SharedString,
    /// Directory or file.
    pub kind: EntryKind,
    /// Size text such as `12.4 KiB`, or the file count of a directory.
    pub detail: SharedString,
    /// Whether the entry is marked.
    pub marked: bool,
}

/// A window of a column.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ColumnData {
    /// What the column lists.
    pub title: SharedString,
    /// The visible rows.
    pub rows: Vec<Entry>,
    /// The cursor, as an index into `rows`.
    pub cursor: Option<usize>,
    /// Rows in the whole list.
    pub total: usize,
}

/// The body of a preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewBody {
    /// Nothing selected or still loading.
    Empty,
    /// One short sentence.
    Message(String),
    /// A picture.
    Image(Rgba8),
    /// Styled text, one `Vec` per line.
    Text(Vec<Vec<(Syntax, String)>>),
    /// A table of records.
    Table {
        /// Column headings.
        columns: Vec<String>,
        /// Rows kept for display.
        rows: Vec<Vec<String>>,
        /// Rows in the file.
        total: usize,
    },
    /// The first bytes, as hex rows already formatted.
    Hex(Vec<String>),
}

/// The preview column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewData {
    /// The VFS path of the previewed file.
    pub path: SharedString,
    /// The kind of file (`texture`, `table`, ...).
    pub kind: SharedString,
    /// Facts about the file.
    pub meta: Vec<(String, String)>,
    /// The content.
    pub body: PreviewBody,
}

/// Everything the view draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// The directory path in the header.
    pub cwd: SharedString,
    /// The parent column.
    pub parent: ColumnData,
    /// The current column.
    pub current: ColumnData,
    /// The preview column.
    pub preview: Option<PreviewData>,
    /// A one-line status.
    pub status: SharedString,
}

/// A key press in the view's own vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyInput {
    /// The key name (`up`, `enter`, `a`).
    pub key: String,
    /// The text the key types, when it types one.
    pub key_char: Option<String>,
    /// Control held.
    pub ctrl: bool,
    /// Alt held.
    pub alt: bool,
    /// Shift held.
    pub shift: bool,
}

/// What produces the frames.
pub trait Backend: 'static {
    /// Tells the backend how many rows the current column shows.
    fn set_height(&mut self, rows: usize);
    /// Feeds a key.
    fn key(&mut self, key: &KeyInput);
    /// Takes background results; true when the frame changed.
    fn poll(&mut self) -> bool;
    /// The current frame.
    fn frame(&self) -> Frame;
    /// Whether the backend asked to quit.
    fn should_quit(&self) -> bool {
        false
    }
}

/// The explorer.
pub struct Explorer {
    backend: Box<dyn Backend>,
    frame: Frame,
    focus: FocusHandle,
}

impl Explorer {
    /// An explorer over `backend`.
    pub fn new(backend: impl Backend, cx: &mut Context<Self>) -> Self {
        let frame = backend.frame();
        let this = Self { backend: Box::new(backend), frame, focus: cx.focus_handle() };
        // Background work (searches, previews) finishes on its own schedule: poll while it runs.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_millis(40)).await;
                let alive = this
                    .update(cx, |this, cx| {
                        if this.backend.poll() {
                            this.frame = this.backend.frame();
                            cx.notify();
                        }
                    })
                    .is_ok();
                if !alive {
                    break;
                }
            }
        })
        .detach();
        this
    }

    /// The focus handle.
    #[must_use]
    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }

    /// The frame being shown.
    #[must_use]
    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    /// Feeds a key as if the user pressed it.
    pub fn press(&mut self, key: &KeyInput, cx: &mut Context<Self>) {
        self.backend.key(key);
        self.frame = self.backend.frame();
        cx.notify();
    }
}

fn column(
    id: &'static str,
    data: &ColumnData,
    theme: &crate::theme::Theme,
    active: bool,
) -> impl IntoElement + use<> {
    let row_height = theme.px(26.0);
    let total = data.rows.len();
    div()
        .id(id)
        .role(Role::List)
        .aria_label(SharedString::from(format!("{}, {} entries", data.title, data.total)))
        .h_full()
        .overflow_hidden()
        .border_r_1()
        .border_color(theme.border.hsla())
        .children(data.rows.iter().enumerate().map(|(ix, row)| {
            let cursor = data.cursor == Some(ix);
            div()
                .id((id, ix))
                .role(Role::ListItem)
                .aria_label(SharedString::from(match row.kind {
                    EntryKind::Dir => format!("{}, folder, {}", row.name, row.detail),
                    EntryKind::File => format!("{}, {}", row.name, row.detail),
                }))
                .aria_selected(cursor)
                .aria_position_in_set(data.total.saturating_sub(total) + ix + 1)
                .aria_size_of_set(data.total.max(total))
                .h(row_height)
                .px(theme.px(8.0))
                .flex()
                .items_center()
                .justify_between()
                .gap(theme.px(8.0))
                .when(cursor && active, |d| d.bg(theme.selection.hsla()).text_color(theme.selection_text.hsla()))
                .when(cursor && !active, |d| d.bg(theme.surface_hover.hsla()))
                .when(row.marked, |d| d.text_color(theme.warn.hsla()))
                .child(div().overflow_hidden().child(match row.kind {
                    EntryKind::Dir => SharedString::from(format!("{}/", row.name)),
                    EntryKind::File => row.name.clone(),
                }))
                .child(div().text_color(theme.text_dim.hsla()).child(row.detail.clone()))
        }))
}

fn preview(data: &PreviewData, theme: &crate::theme::Theme, cx: &mut gpui::App) -> gpui::AnyElement {
    let mono = theme.mono_font;
    let body: gpui::AnyElement = match &data.body {
        PreviewBody::Empty => div().into_any_element(),
        PreviewBody::Message(m) => div().p(theme.px(8.0)).text_color(theme.text_dim.hsla()).child(m.clone()).into_any_element(),
        PreviewBody::Image(img) => match render_image(img) {
            Some(image) => ImageSurface::new("nie-preview-image", SharedString::from(format!("{}, {}", data.kind, data.path)), image)
                .into_any_element(),
            None => div().child("The picture is malformed.").into_any_element(),
        },
        PreviewBody::Text(lines) => div()
            .id("nie-preview-text")
            .role(Role::Document)
            .aria_label(SharedString::from(format!("Text of {}", data.path)))
            .font_family(mono)
            .p(theme.px(8.0))
            .overflow_hidden()
            .children(lines.iter().map(|line| {
                div().flex().children(line.iter().map(|(style, text)| {
                    div().text_color(theme.syntax(*style).hsla()).child(SharedString::from(text.replace(' ', "\u{a0}")))
                }))
            }))
            .into_any_element(),
        PreviewBody::Table { columns, rows, total } => div()
            .id("nie-preview-table")
            .role(Role::Table)
            .aria_label(SharedString::from(format!("Records of {}, {} rows", data.path, total)))
            .aria_row_count(*total + 1)
            .aria_column_count(columns.len())
            .overflow_hidden()
            .child(
                div()
                    .id("nie-preview-table-head")
                    .role(Role::Row)
                    .aria_label("Columns")
                    .flex()
                    .text_color(theme.accent.hsla())
                    .children(columns.iter().enumerate().map(|(c, title)| {
                        div()
                            .id(("nie-preview-col", c))
                            .role(Role::ColumnHeader)
                            .aria_label(SharedString::from(title.clone()))
                            .flex_1()
                            .px(theme.px(6.0))
                            .child(SharedString::from(title.clone()))
                    })),
            )
            .children(rows.iter().enumerate().map(|(r, row)| {
                div()
                    .id(("nie-preview-row", r))
                    .role(Role::Row)
                    .aria_label(SharedString::from(row.first().cloned().unwrap_or_default()))
                    .flex()
                    .children(row.iter().enumerate().map(|(c, text)| {
                        div()
                            .id(("nie-preview-cell", r * 4096 + c))
                            .role(Role::Cell)
                            .aria_label(SharedString::from(text.clone()))
                            .flex_1()
                            .px(theme.px(6.0))
                            .overflow_hidden()
                            .child(SharedString::from(text.clone()))
                    }))
            }))
            .into_any_element(),
        PreviewBody::Hex(rows) => div()
            .id("nie-preview-hex")
            .role(Role::Document)
            .aria_label(SharedString::from(format!("Hex dump of {}", data.path)))
            .font_family(mono)
            .p(theme.px(8.0))
            .overflow_hidden()
            .children(rows.iter().map(|r| div().child(SharedString::from(r.replace(' ', "\u{a0}")))))
            .into_any_element(),
    };
    let _ = cx;
    div()
        .id("nie-preview")
        .role(Role::Group)
        .aria_label(SharedString::from(format!("Preview of {}", data.path)))
        .size_full()
        .flex()
        .flex_col()
        .child(
            div()
                .p(theme.px(8.0))
                .border_b_1()
                .border_color(theme.border.hsla())
                .text_color(theme.text_dim.hsla())
                .child(SharedString::from(format!(
                    "{}  {}",
                    data.kind,
                    data.meta.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join("  ")
                ))),
        )
        .child(div().flex_1().min_h_0().overflow_hidden().child(body))
        .into_any_element()
}

impl Render for Explorer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Settings::theme(cx).clone();
        let row_px = theme.px(26.0);
        let rows = ((window.viewport_size().height - theme.px(72.0)) / row_px).floor().max(3.0) as usize;
        self.backend.set_height(rows);
        self.frame = self.backend.frame();
        let frame = self.frame.clone();
        div()
            .id("nie-explorer")
            .role(Role::Application)
            .aria_label("nie file explorer")
            .track_focus(&self.focus)
            .tab_stop(true)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                let ks = &event.keystroke;
                this.press(
                    &KeyInput {
                        key: ks.key.clone(),
                        key_char: ks.key_char.clone(),
                        ctrl: ks.modifiers.control,
                        alt: ks.modifiers.alt,
                        shift: ks.modifiers.shift,
                    },
                    cx,
                );
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background.hsla())
            .text_color(theme.text.hsla())
            .font_family(theme.ui_font)
            .text_size(theme.px(theme.text_size))
            .border_2()
            .border_color(transparent_black())
            .focus_visible(|s| s.border_color(theme.focus.hsla()))
            .child(
                div()
                    .id("nie-explorer-header")
                    .role(Role::Heading)
                    .aria_level(1)
                    .aria_label(SharedString::from(format!("Folder {}", frame.cwd)))
                    .h(theme.px(36.0))
                    .px(theme.px(10.0))
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(theme.border.hsla())
                    .text_color(theme.accent.hsla())
                    .child(frame.cwd.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(div().w_1_4().h_full().child(column("nie-col-parent", &frame.parent, &theme, false)))
                    .child(div().w_1_3().h_full().child(column("nie-col-current", &frame.current, &theme, true)))
                    .child(div().flex_1().h_full().children(frame.preview.as_ref().map(|p| preview(p, &theme, cx)))),
            )
            .child(
                div()
                    .id("nie-explorer-status")
                    .role(Role::Status)
                    .aria_label(if frame.status.is_empty() { SharedString::from("Ready") } else { frame.status.clone() })
                    .h(theme.px(28.0))
                    .px(theme.px(10.0))
                    .flex()
                    .items_center()
                    .border_t_1()
                    .border_color(theme.border.hsla())
                    .text_color(theme.text_dim.hsla())
                    .child(frame.status.clone()),
            )
    }
}
