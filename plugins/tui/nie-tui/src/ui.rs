//! The screen: header, three columns (parent, current, preview), status bar, input line, help.

use nie_explore::fm::keymap::KeyMode;
use nie_explore::fm::model::{Column, InputKind, Row as FmRow, Target, View};
use nie_explore::fm::preview::human_size;
use nie_explore::fm::{Keymap, NodeKind, TreeMode};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};

use crate::pane::Pane;
use crate::theme;

/// Facts the status bar shows besides the view.
#[derive(Debug, Clone, Default)]
pub struct Status {
    /// The graphics protocol in use.
    pub protocol: String,
    /// Time of the last finished preview, in milliseconds.
    pub preview_ms: Option<u128>,
    /// Whether a preview is being built.
    pub busy: bool,
}

fn row_item(row: &FmRow, width: u16, show_size: bool) -> ListItem<'static> {
    let mut style = match row.kind {
        NodeKind::Dir => theme::directory(),
        NodeKind::File => Style::new(),
    };
    if row.marked {
        style = theme::marked().add_modifier(Modifier::REVERSED);
    }
    let detail = match row.kind {
        NodeKind::Dir => format!("{}", row.files),
        NodeKind::File if show_size => human_size(row.size),
        NodeKind::File => String::new(),
    };
    let room = usize::from(width).saturating_sub(detail.chars().count() + 2);
    let mut name: String = row.name.clone();
    if row.kind == NodeKind::Dir && !row.name.ends_with('/') && room > 1 {
        name.push('/');
    }
    let shown: String = if name.chars().count() > room {
        let keep = room.saturating_sub(1);
        // Long search paths keep their tail, the part that tells files apart.
        let tail: String = name.chars().rev().take(keep).collect::<Vec<_>>().into_iter().rev().collect();
        format!("~{tail}")
    } else {
        name
    };
    let pad = room.saturating_sub(shown.chars().count());
    ListItem::new(Line::from(vec![
        Span::styled(format!(" {shown}{}", " ".repeat(pad)), style),
        Span::styled(format!("{detail} "), Style::new().add_modifier(Modifier::DIM)),
    ]))
}

fn column(frame: &mut Frame<'_>, area: Rect, col: &Column, show_size: bool, focused: bool) {
    let title = format!(" {} ", col.title);
    let block = Block::new()
        .borders(Borders::RIGHT | Borders::TOP)
        .border_style(theme::border())
        .title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let items: Vec<ListItem<'_>> = col
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let item = row_item(row, inner.width, show_size);
            if col.cursor == Some(i) && focused {
                item.style(theme::cursor())
            } else if col.cursor == Some(i) {
                item.style(Style::new().add_modifier(Modifier::REVERSED | Modifier::DIM))
            } else {
                item
            }
        })
        .collect();
    frame.render_widget(List::new(items), inner);
}

fn directory_preview(frame: &mut Frame<'_>, area: Rect, target: &Target) {
    let lines: Vec<Line<'_>> = std::iter::once(Line::styled(
        format!("{} files below", target.files),
        Style::new().add_modifier(Modifier::DIM),
    ))
    .chain(target.children.iter().take(usize::from(area.height)).map(|(name, kind)| match kind {
        NodeKind::Dir => Line::styled(format!("{name}/"), theme::directory()),
        NodeKind::File => Line::raw(name.clone()),
    }))
    .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

fn help(frame: &mut Frame<'_>, area: Rect, keymap: &Keymap) {
    let width = area.width.saturating_sub(2).min(160);
    let height = area.height.min(30);
    let popup = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    frame.render_widget(Clear, popup);
    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(" keys (? to close) ");
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let lines: Vec<Line<'_>> = keymap
        .bindings(KeyMode::Manager)
        .iter()
        .map(|b| {
            let keys: String = b.keys.iter().map(ToString::to_string).collect();
            Line::from(vec![
                Span::styled(format!("{keys:<12}"), theme::style(nie_explore::fm::preview::Style::Accent)),
                Span::raw(b.desc.clone()),
            ])
        })
        .collect();
    // Every binding stays visible: the list flows into as many columns as the height needs.
    let rows = usize::from(inner.height).max(1);
    let columns = Layout::horizontal(vec![Constraint::Fill(1); lines.len().div_ceil(rows).max(1)]).split(inner);
    for (column, chunk) in columns.iter().zip(lines.chunks(rows)) {
        frame.render_widget(Paragraph::new(chunk.to_vec()), *column);
    }
}

/// Draws one frame.
pub fn draw(
    frame: &mut Frame<'_>,
    view: &View,
    pane: &mut Pane,
    keymap: &Keymap,
    status: &Status,
    preview_scroll: usize,
) {
    let area = frame.area();
    let [header, body, bar] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(area);

    let mode = match view.mode {
        TreeMode::Path => "path",
        TreeMode::Cpk => "cpk",
    };
    let target_cpk = view
        .target
        .as_ref()
        .and_then(|t| t.cpk.clone())
        .map(|c| format!("  [{c}]"))
        .unwrap_or_default();
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" {mode} "), theme::style(nie_explore::fm::preview::Style::Accent)),
            Span::raw(format!("/{}", view.cwd.trim_start_matches('/'))),
            Span::styled(target_cpk, Style::new().add_modifier(Modifier::DIM)),
        ])),
        header,
    );

    let [parent_area, current_area, preview_area] = Layout::horizontal([
        Constraint::Percentage(16),
        Constraint::Percentage(34),
        Constraint::Percentage(50),
    ])
    .areas(body);
    column(frame, parent_area, &view.parent, false, false);
    column(frame, current_area, &view.current, true, true);
    let inner = Block::new().borders(Borders::TOP).inner(preview_area);
    frame.render_widget(
        Block::new().borders(Borders::TOP).border_style(theme::border()).title(" preview "),
        preview_area,
    );
    match &view.target {
        Some(target) if target.kind == NodeKind::Dir => directory_preview(frame, inner, target),
        Some(_) => pane.render(frame, inner, preview_scroll),
        None => {}
    }

    let left = match &view.input {
        Some((kind, text)) => {
            let label = match kind {
                InputKind::Search => "search",
                InputKind::Filter => "filter",
            };
            format!(" {label}> {text}_")
        }
        None => {
            let mut parts = vec![format!(
                " {}/{}",
                view.current.cursor.map_or(0, |c| c + view.current.offset + 1),
                view.current.total
            )];
            if let Some(filter) = &view.filter {
                parts.push(format!("filter {filter}"));
            }
            if let Some(search) = &view.search {
                let progress = view
                    .progress
                    .map(|p| format!("{} of {}{}", p.matched, p.total, if p.running { " ..." } else { "" }))
                    .unwrap_or_default();
                parts.push(format!("search {search} ({progress})"));
            }
            if view.marked > 0 {
                parts.push(format!("{} marked", view.marked));
            }
            if !view.pending.is_empty() {
                parts.push(format!("keys {}", view.pending));
            }
            if let Some(message) = &view.status {
                parts.push(message.clone());
            }
            parts.join("  ")
        }
    };
    let right = {
        let kind = pane.preview().map_or("", |p| p.kind);
        let timing = match (status.busy, status.preview_ms) {
            (true, _) => " loading".to_owned(),
            (false, Some(ms)) => format!(" {ms} ms"),
            _ => String::new(),
        };
        format!("{kind}{timing}  {}  {} files ", status.protocol, view.files)
    };
    let width = usize::from(bar.width);
    let gap = width.saturating_sub(left.chars().count() + right.chars().count());
    frame.render_widget(
        Paragraph::new(format!("{left}{}{right}", " ".repeat(gap))).style(theme::bar()),
        bar,
    );
    if view.help {
        help(frame, area, keymap);
    }
}
