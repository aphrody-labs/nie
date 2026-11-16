//! Data-driven UI shared by Azalee, Achillea and nie.
//!
//! A screen is a [`UiNode`] tree. The tree is not written by hand: [`screen_from_dataset`] builds
//! it from a [`nie_cfgsql::Dataset`], i.e. from a `cfg.bin` read through the VFS. Two renderers
//! consume the same tree and the same [`UiStyle`]:
//!
//! - Dioxus ([`NodeView`], [`render_html`]) emits CSS flexbox for the browser/desktop webview;
//! - taffy ([`layout`]) computes absolute rectangles for the Bevy renderer (Bevy UI sits on taffy
//!   too), so both agree on geometry because the style maps to the same flexbox model.

use dioxus::prelude::*;
use nie_cfgsql::{Cell, Dataset};
use serde::{Deserialize, Serialize};
use taffy::prelude::*;

/// RGBA colour, 8 bits per channel.
pub type Rgba = [u8; 4];

/// Main axis of a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Axis {
    /// Children side by side.
    Row,
    /// Children stacked.
    Column,
}

/// Box and text style, the common subset of CSS flexbox and taffy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiStyle {
    /// Main axis.
    pub axis: Axis,
    /// Gap between children, px.
    pub gap: f32,
    /// Padding on all sides, px.
    pub padding: f32,
    /// Fixed width, px.
    pub width: Option<f32>,
    /// Fixed height, px.
    pub height: Option<f32>,
    /// Flex grow factor.
    pub grow: f32,
    /// Background.
    pub background: Option<Rgba>,
    /// Foreground (text) colour.
    pub color: Option<Rgba>,
    /// Font size, px.
    pub font_px: f32,
    /// Wrap children onto several lines.
    pub wrap: bool,
}

impl Default for UiStyle {
    fn default() -> Self {
        Self {
            axis: Axis::Column,
            gap: 0.0,
            padding: 0.0,
            width: None,
            height: None,
            grow: 0.0,
            background: None,
            color: None,
            font_px: 14.0,
            wrap: false,
        }
    }
}

fn rgba_css(c: Rgba) -> String {
    format!("rgba({},{},{},{:.3})", c[0], c[1], c[2], f32::from(c[3]) / 255.0)
}

impl UiStyle {
    /// CSS declarations equivalent to this style.
    #[must_use]
    pub fn css(&self) -> String {
        let mut s = format!(
            "display:flex;flex-direction:{};gap:{}px;padding:{}px;font-size:{}px;box-sizing:border-box;",
            if self.axis == Axis::Row { "row" } else { "column" },
            self.gap,
            self.padding,
            self.font_px
        );
        if self.wrap {
            s.push_str("flex-wrap:wrap;");
        }
        if let Some(w) = self.width {
            s.push_str(&format!("width:{w}px;"));
        }
        if let Some(h) = self.height {
            s.push_str(&format!("height:{h}px;"));
        }
        if self.grow > 0.0 {
            s.push_str(&format!("flex-grow:{};", self.grow));
        }
        if let Some(c) = self.background {
            s.push_str(&format!("background:{};", rgba_css(c)));
        }
        if let Some(c) = self.color {
            s.push_str(&format!("color:{};", rgba_css(c)));
        }
        s
    }

    /// The same style as a taffy flexbox style.
    #[must_use]
    pub fn to_taffy(&self) -> Style {
        Style {
            flex_direction: if self.axis == Axis::Row { FlexDirection::Row } else { FlexDirection::Column },
            flex_wrap: if self.wrap { FlexWrap::Wrap } else { FlexWrap::NoWrap },
            gap: Size { width: length(self.gap), height: length(self.gap) },
            padding: Rect { left: length(self.padding), right: length(self.padding), top: length(self.padding), bottom: length(self.padding) },
            size: Size {
                width: self.width.map_or_else(auto, length),
                height: self.height.map_or_else(auto, length),
            },
            flex_grow: self.grow,
            ..Default::default()
        }
    }
}

/// A node of the UI tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UiNode {
    /// Flex container.
    Box {
        /// Style.
        style: UiStyle,
        /// Children in order.
        children: Vec<UiNode>,
    },
    /// A run of text.
    Text {
        /// Style (font size, colour).
        style: UiStyle,
        /// Content.
        text: String,
    },
    /// An image served by the CDN or embedded.
    Image {
        /// URL or data URI.
        src: String,
        /// Alternative text.
        alt: String,
        /// Width, px.
        width: f32,
        /// Height, px.
        height: f32,
    },
}

impl UiNode {
    /// Container with `style`.
    #[must_use]
    pub fn boxed(style: UiStyle, children: Vec<UiNode>) -> Self {
        Self::Box { style, children }
    }

    /// Text with `style`.
    #[must_use]
    pub fn text(style: UiStyle, text: impl Into<String>) -> Self {
        Self::Text { style, text: text.into() }
    }

    /// Number of nodes in the tree.
    #[must_use]
    pub fn count(&self) -> usize {
        match self {
            Self::Box { children, .. } => 1 + children.iter().map(Self::count).sum::<usize>(),
            _ => 1,
        }
    }
}

/// Colours of a generated screen.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    /// Page background.
    pub page: Rgba,
    /// Table header background.
    pub header: Rgba,
    /// Alternate row background.
    pub row_alt: Rgba,
    /// Text.
    pub text: Rgba,
    /// Muted text.
    pub muted: Rgba,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            page: [18, 20, 28, 255],
            header: [44, 52, 82, 255],
            row_alt: [26, 30, 44, 255],
            text: [232, 236, 245, 255],
            muted: [140, 150, 175, 255],
        }
    }
}

fn cell_text(c: &Cell) -> String {
    match c {
        Cell::Null => "-".into(),
        Cell::Bool(b) => b.to_string(),
        Cell::Int(n) => n.to_string(),
        Cell::Real(f) => format!("{f}"),
        Cell::Text(s) => s.clone(),
        Cell::Blob(b) => format!("{} bytes", b.len()),
    }
}

/// Builds a screen from a `cfg.bin` dataset: one titled section per table, a header row of column
/// names and at most `max_rows` data rows, with the true row count under each table.
#[must_use]
pub fn screen_from_dataset(data: &Dataset, title: &str, max_rows: usize, theme: &Theme) -> UiNode {
    let col_w = 120.0;
    let text = |size: f32, color: Rgba| UiStyle { font_px: size, color: Some(color), ..UiStyle::default() };
    let mut sections = vec![UiNode::text(text(24.0, theme.text), title)];
    for (table, rows) in data.tables.iter().zip(&data.rows) {
        sections.push(UiNode::text(text(18.0, theme.text), table.name.clone()));
        let row_style = |bg: Option<Rgba>| UiStyle { axis: Axis::Row, gap: 4.0, padding: 4.0, background: bg, ..UiStyle::default() };
        let cell_style = |color: Rgba| UiStyle { width: Some(col_w), font_px: 12.0, color: Some(color), ..UiStyle::default() };
        let head = std::iter::once("row_index".to_owned())
            .chain(table.columns.iter().map(|c| c.name.clone()))
            .map(|n| UiNode::text(cell_style(theme.text), n))
            .collect();
        let mut lines = vec![UiNode::boxed(row_style(Some(theme.header)), head)];
        for (i, row) in rows.iter().take(max_rows).enumerate() {
            let cells = std::iter::once(i.to_string())
                .chain(row.iter().map(cell_text))
                .map(|t| UiNode::text(cell_style(theme.text), t))
                .collect();
            lines.push(UiNode::boxed(row_style((i % 2 == 1).then_some(theme.row_alt)), cells));
        }
        sections.push(UiNode::boxed(UiStyle { gap: 1.0, ..UiStyle::default() }, lines));
        sections.push(UiNode::text(text(12.0, theme.muted), format!("{} rows, {} columns", rows.len(), table.columns.len() + 1)));
    }
    UiNode::boxed(UiStyle { gap: 12.0, padding: 24.0, background: Some(theme.page), ..UiStyle::default() }, sections)
}

/// Dioxus view of a node; the same component serves SSR, web and desktop.
#[component]
pub fn NodeView(node: UiNode) -> Element {
    match node {
        UiNode::Box { style, children } => {
            let css = style.css();
            rsx! {
                div { style: "{css}",
                    for child in children {
                        NodeView { node: child }
                    }
                }
            }
        }
        UiNode::Text { style, text } => {
            let css = style.css();
            rsx! { span { style: "{css}", "{text}" } }
        }
        UiNode::Image { src, alt, width, height } => {
            rsx! { img { src: "{src}", alt: "{alt}", width: "{width}", height: "{height}" } }
        }
    }
}

/// Renders a tree to an HTML document.
#[cfg(feature = "ssr")]
#[must_use]
pub fn render_html(root: &UiNode, title: &str) -> String {
    let mut dom = VirtualDom::new_with_props(NodeView, NodeViewProps { node: root.clone() });
    dom.rebuild_in_place();
    let body = dioxus_ssr::render(&dom);
    let title = title.replace('&', "&amp;").replace('<', "&lt;");
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{title}</title>\
         <style>body{{margin:0;font-family:system-ui,sans-serif;background:#12141c}}</style></head><body>{body}</body></html>"
    )
}

/// A node placed by taffy, in absolute pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    /// Depth in the tree (root 0).
    pub depth: usize,
    /// `box`, `text` or `image`.
    pub kind: &'static str,
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

fn text_size(style: &UiStyle, text: &str) -> (f32, f32) {
    let w = style.width.unwrap_or(text.chars().count() as f32 * style.font_px * 0.6);
    (w, style.font_px * 1.3)
}

fn build(tree: &mut TaffyTree<()>, node: &UiNode) -> NodeId {
    match node {
        UiNode::Box { style, children } => {
            let kids: Vec<NodeId> = children.iter().map(|c| build(tree, c)).collect();
            tree.new_with_children(style.to_taffy(), &kids).expect("taffy node")
        }
        UiNode::Text { style, text } => {
            let (w, h) = text_size(style, text);
            tree.new_leaf(Style { size: Size { width: length(w), height: length(h) }, flex_shrink: 0.0, ..Default::default() })
                .expect("taffy leaf")
        }
        UiNode::Image { width, height, .. } => tree
            .new_leaf(Style { size: Size { width: length(*width), height: length(*height) }, flex_shrink: 0.0, ..Default::default() })
            .expect("taffy leaf"),
    }
}

fn collect(tree: &TaffyTree<()>, node: &UiNode, id: NodeId, depth: usize, ox: f32, oy: f32, out: &mut Vec<Placed>) {
    let l = tree.layout(id).expect("layout");
    let (x, y) = (ox + l.location.x, oy + l.location.y);
    let kind = match node {
        UiNode::Box { .. } => "box",
        UiNode::Text { .. } => "text",
        UiNode::Image { .. } => "image",
    };
    out.push(Placed { depth, kind, x, y, width: l.size.width, height: l.size.height });
    if let UiNode::Box { children, .. } = node {
        let ids = tree.children(id).expect("children");
        for (c, cid) in children.iter().zip(ids) {
            collect(tree, c, cid, depth + 1, x, y, out);
        }
    }
}

/// Lays the tree out in a viewport `width` px wide and returns every node in pre-order.
#[must_use]
pub fn layout(root: &UiNode, width: f32) -> Vec<Placed> {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let id = build(&mut tree, root);
    tree.compute_layout(id, Size { width: AvailableSpace::Definite(width), height: AvailableSpace::MaxContent })
        .expect("compute layout");
    let mut out = Vec::with_capacity(root.count());
    collect(&tree, root, id, 0, 0.0, 0.0, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use nie_formats::cfgbin::{RdbnList, RdbnRow, RdbnValue};

    fn data() -> Dataset {
        nie_cfgsql::from_rdbn_lists(&[RdbnList {
            name: "CharaParam".into(),
            type_name: "T".into(),
            rows: (0..3)
                .map(|i| RdbnRow { fields: vec![("Name".into(), RdbnValue::Condition(format!("<n{i}>"))), ("Kick".into(), RdbnValue::Int(i))] })
                .collect(),
        }])
    }

    #[test]
    fn screen_has_every_value() {
        let ui = screen_from_dataset(&data(), "chara", 2, &Theme::default());
        let html = render_html(&ui, "chara");
        assert!(html.contains("chara_param"));
        assert!(html.contains("n0") && html.contains("n1") && !html.contains("<n0>"), "values present and escaped");
        assert!(!html.contains("n2"), "max_rows must cut the third row");
        assert!(html.contains("3 rows, 3 columns"));
        assert!(html.contains("flex-direction:row"));
    }

    #[test]
    fn taffy_layout_matches_flexbox() {
        let t = UiStyle { width: Some(100.0), ..UiStyle::default() };
        let row = UiNode::boxed(
            UiStyle { axis: Axis::Row, gap: 10.0, padding: 5.0, ..UiStyle::default() },
            vec![UiNode::text(t.clone(), "a"), UiNode::text(t, "b")],
        );
        let p = layout(&row, 400.0);
        assert_eq!(p.len(), 3);
        // second child starts after padding + first width + gap
        assert_eq!((p[1].x, p[2].x), (5.0, 115.0));
        assert_eq!(p[0].width, 400.0_f32.min(p[0].width));
        assert!(p[0].height > 0.0);
    }

    #[test]
    fn tree_roundtrips_json() {
        let ui = screen_from_dataset(&data(), "x", 5, &Theme::default());
        let back: UiNode = serde_json::from_str(&serde_json::to_string(&ui).unwrap()).unwrap();
        assert_eq!(ui, back);
    }
}
