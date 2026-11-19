//! The preview column: turns a finished [`Preview`] into widgets and keeps the graphics-protocol
//! state of its pictures.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use image::{DynamicImage, RgbaImage};
use nie_explore::fm::preview::{Body, Image, Preview};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};

use crate::theme;

/// The animation rate of a gallery that has no rate of its own.
const GALLERY_FPS: u32 = 6;

/// State of the preview column.
pub struct Pane {
    preview: Option<Arc<Preview>>,
    loading: Option<String>,
    picker: Picker,
    protocols: HashMap<usize, StatefulProtocol>,
    frame: usize,
    advanced: Instant,
}

impl Pane {
    /// A pane that encodes pictures with the protocol of `picker`.
    #[must_use]
    pub fn new(picker: Picker) -> Self {
        Self {
            preview: None,
            loading: None,
            picker,
            protocols: HashMap::new(),
            frame: 0,
            advanced: Instant::now(),
        }
    }

    /// The protocol in use, for the status bar.
    #[must_use]
    pub fn protocol_name(&self) -> String {
        format!("{:?}", self.picker.protocol_type()).to_lowercase()
    }

    /// Shows a finished preview.
    pub fn set(&mut self, preview: Preview) {
        self.preview = Some(Arc::new(preview));
        self.loading = None;
        self.protocols.clear();
        self.frame = 0;
        self.advanced = Instant::now();
    }

    /// Notes that a preview for `path` is on its way; the old one stays visible.
    pub fn loading(&mut self, path: String) {
        self.loading = Some(path);
    }

    /// Drops the preview.
    pub fn clear(&mut self) {
        self.preview = None;
        self.loading = None;
        self.protocols.clear();
    }

    /// The preview on screen.
    #[must_use]
    pub fn preview(&self) -> Option<&Preview> {
        self.preview.as_deref()
    }

    /// Number of frames of the current preview.
    #[must_use]
    pub fn frame_count(&self) -> usize {
        match self.preview.as_ref().map(|p| &p.body) {
            Some(Body::Frames(f)) => f.images.len(),
            _ => 0,
        }
    }

    /// Whether the picture is animated by itself (a film, a turntable).
    #[must_use]
    pub fn animates(&self) -> bool {
        matches!(self.preview.as_ref().map(|p| &p.body), Some(Body::Frames(f)) if f.fps > 0)
    }

    /// Selects a frame by absolute index (wraps).
    pub fn show_frame(&mut self, index: i32) {
        let count = self.frame_count();
        if count > 0 {
            self.frame = index.rem_euclid(i32::try_from(count).unwrap_or(1)) as usize;
        }
    }

    /// Advances the animation when due. Returns true when the frame changed.
    pub fn tick(&mut self, now: Instant) -> bool {
        let Some(Body::Frames(frames)) = self.preview.as_ref().map(|p| &p.body) else {
            return false;
        };
        let fps = if frames.fps > 0 { frames.fps } else { GALLERY_FPS };
        let due = Duration::from_secs_f64(1.0 / f64::from(fps.max(1)));
        if frames.images.len() > 1 && now.duration_since(self.advanced) >= due {
            self.frame = (self.frame + 1) % frames.images.len();
            self.advanced = now;
            return true;
        }
        false
    }

    /// How long until the next animation frame is due.
    #[must_use]
    pub fn next_tick(&self) -> Option<Duration> {
        let Some(Body::Frames(frames)) = self.preview.as_ref().map(|p| &p.body) else {
            return None;
        };
        let fps = if frames.fps > 0 { frames.fps } else { GALLERY_FPS };
        let due = Duration::from_secs_f64(1.0 / f64::from(fps.max(1)));
        Some(due.saturating_sub(self.advanced.elapsed()))
    }

    fn picture(&mut self, frame: &mut Frame<'_>, area: Rect, key: usize, image: &Image) {
        if !self.protocols.contains_key(&key) {
            let Some(rgba) = RgbaImage::from_raw(image.width, image.height, image.rgba.clone()) else {
                frame.render_widget(Paragraph::new("invalid picture"), area);
                return;
            };
            let protocol = self.picker.new_resize_protocol(DynamicImage::ImageRgba8(rgba));
            self.protocols.insert(key, protocol);
        }
        let Some(protocol) = self.protocols.get_mut(&key) else {
            return;
        };
        frame.render_stateful_widget(
            // `Scale` fills the pane, small sprites included; `Fit` never enlarges.
            StatefulImage::default().resize(Resize::Scale(None)),
            area,
            protocol,
        );
    }

    /// Draws the pane into `area`. `scroll` is the row offset of text, table and hex views.
    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect, scroll: usize) {
        let Some(preview) = self.preview.clone() else {
            let text = self
                .loading
                .as_ref()
                .map_or("nothing selected".to_owned(), |p| format!("loading {p} ..."));
            frame.render_widget(Paragraph::new(text).style(theme::style(nie_explore::fm::preview::Style::Dim)), area);
            return;
        };
        let meta_rows = u16::try_from(preview.meta.len()).unwrap_or(u16::MAX).min(area.height / 3);
        let [meta_area, body_area] =
            Layout::vertical([Constraint::Length(meta_rows), Constraint::Min(1)]).areas(area);
        let meta: Vec<Line<'_>> = preview
            .meta
            .iter()
            .map(|(k, v)| {
                Line::from(vec![
                    Span::styled(format!("{k:<11}"), Style::new().add_modifier(Modifier::DIM)),
                    Span::raw(v.clone()),
                ])
            })
            .collect();
        frame.render_widget(Paragraph::new(meta), meta_area);
        match &preview.body {
            Body::Image(image) => self.picture(frame, body_area, 0, image),
            Body::Frames(frames) => {
                if frames.images.is_empty() {
                    return;
                }
                let at = self.frame.min(frames.images.len() - 1);
                let [label_area, picture_area] =
                    Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(body_area);
                let label = frames.labels.get(at).cloned().unwrap_or_default();
                frame.render_widget(
                    Paragraph::new(format!("{}/{}  {label}", at + 1, frames.images.len()))
                        .style(theme::style(nie_explore::fm::preview::Style::Accent)),
                    label_area,
                );
                self.picture(frame, picture_area, at, &frames.images[at]);
            }
            Body::Text(lines) => {
                let rendered: Vec<Line<'_>> = lines
                    .iter()
                    .skip(scroll)
                    .take(body_area.height as usize)
                    .map(|line| {
                        Line::from(
                            line.0
                                .iter()
                                .map(|s| Span::styled(s.text.clone(), theme::style(s.style)))
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect();
                frame.render_widget(Paragraph::new(rendered), body_area);
            }
            Body::Table(table) => {
                let widths: Vec<Constraint> = (0..table.columns.len())
                    .map(|i| {
                        let longest = table
                            .rows
                            .iter()
                            .take(200)
                            .filter_map(|r| r.get(i))
                            .map(|c| c.chars().count())
                            .chain(std::iter::once(table.columns[i].chars().count()))
                            .max()
                            .unwrap_or(4);
                        Constraint::Length(u16::try_from(longest.clamp(3, 28)).unwrap_or(28))
                    })
                    .collect();
                let header = Row::new(table.columns.iter().map(|c| Cell::from(c.clone())))
                    .style(theme::style(nie_explore::fm::preview::Style::Header));
                let rows = table
                    .rows
                    .iter()
                    .skip(scroll)
                    .take(body_area.height as usize)
                    .map(|r| Row::new(r.iter().map(|c| Cell::from(c.clone()))));
                let title = format!(
                    "{}  ({} of {} rows from {})",
                    table.title,
                    table.rows.len().min(table.total_rows),
                    table.total_rows,
                    scroll + 1
                );
                frame.render_widget(
                    Table::new(rows, widths)
                        .header(header)
                        .column_spacing(1)
                        .block(Block::new().borders(Borders::TOP).title(title).border_style(theme::border())),
                    body_area,
                );
            }
            Body::Hex { bytes, total } => {
                let lines: Vec<Line<'_>> = bytes
                    .chunks(16)
                    .enumerate()
                    .skip(scroll)
                    .take(body_area.height as usize)
                    .map(|(i, chunk)| Line::raw(nie_explore::fm::preview::hex_row(i * 16, chunk)))
                    .collect();
                frame.render_widget(Paragraph::new(lines), body_area);
                if *total > bytes.len() {
                    frame.render_widget(
                        Paragraph::new(format!("first {} of {total} bytes", bytes.len()))
                            .alignment(Alignment::Right)
                            .style(theme::style(nie_explore::fm::preview::Style::Dim)),
                        Rect { height: 1, ..body_area },
                    );
                }
            }
            Body::Message(text) => {
                frame.render_widget(
                    Paragraph::new(text.clone()).wrap(ratatui::widgets::Wrap { trim: true }),
                    body_area,
                );
            }
        }
    }
}
