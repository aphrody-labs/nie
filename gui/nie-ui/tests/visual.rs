//! Snapshot tests: every component is rendered offscreen with GPUI's headless wgpu renderer and
//! compared with the committed reference picture, and the AccessKit tree of each scene is audited.
//!
//! Needs a Vulkan device (lavapipe is enough): `cargo test -p nie-ui --features test-support`.
//! `NIE_UI_UPDATE=1` rewrites the references. The scenes hold synthetic data only: the references
//! contain no game asset.
#![cfg(all(feature = "test-support", target_os = "linux"))]

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

use gpui::{
    AnyWindowHandle, App, AppContext as _, Context, Entity, HeadlessAppContext, IntoElement, Render, SharedString, Window,
    div, prelude::*, px, size,
};
use image::RgbaImage;
use nie_ui::explorer::{
    Backend, ColumnData, Entry, EntryKind, Explorer, Frame, KeyInput, PreviewBody, PreviewData,
};
use nie_ui::prelude::*;
use nie_ui::terminal::TermCell;
use nie_ui::theme::Syntax;

const FONTS: [&[u8]; 4] = [
    include_bytes!("../../../vendor/zed/assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf"),
    include_bytes!("../../../vendor/zed/assets/fonts/ibm-plex-sans/IBMPlexSans-SemiBold.ttf"),
    include_bytes!("../../../vendor/zed/assets/fonts/lilex/Lilex-Regular.ttf"),
    include_bytes!("../../../vendor/zed/assets/fonts/lilex/Lilex-Bold.ttf"),
];

/// A synthetic palette: dark navy, off-white and the usual hues. It stands for `FONT_COLOR`.
fn palette() -> Vec<nie_data::font_color::FontColor> {
    use nie_data::hash::HashId;
    [
        (18, 22, 42), (245, 245, 235), (235, 70, 80), (70, 200, 110), (240, 190, 60),
        (80, 150, 240), (180, 110, 230), (60, 200, 180), (130, 134, 150),
    ]
    .iter()
    .enumerate()
    .map(|(i, rgb)| nie_data::font_color::FontColor { id: HashId(i as u32), rgb: *rgb, rubi_rgb: *rgb })
    .collect()
}

fn harness() -> HeadlessAppContext {
    let text = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts("IBM Plex Sans"));
    let cx = HeadlessAppContext::with_platform(text, Arc::new(()), || gpui_platform::current_headless_renderer());
    cx.text_system().add_fonts(FONTS.iter().map(|f| Cow::Borrowed(*f)).collect()).expect("fonts");
    cx
}

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

fn compare(name: &str, actual: &RgbaImage) {
    let path = snapshot_dir().join(format!("{name}.png"));
    if std::env::var_os("NIE_UI_UPDATE").is_some() || !path.exists() {
        std::fs::create_dir_all(snapshot_dir()).expect("snapshot dir");
        actual.save(&path).expect("write snapshot");
        eprintln!("wrote {}", path.display());
        return;
    }
    let expected = image::open(&path).expect("read snapshot").to_rgba8();
    assert_eq!(expected.dimensions(), actual.dimensions(), "{name}: size changed");
    let differing = expected
        .pixels()
        .zip(actual.pixels())
        .filter(|(a, b)| a.0.iter().zip(b.0.iter()).any(|(x, y)| x.abs_diff(*y) > 12))
        .count();
    let total = (expected.width() * expected.height()) as usize;
    if differing * 1000 > total {
        let out = std::env::temp_dir().join(format!("nie-ui-{name}-actual.png"));
        actual.save(&out).expect("write actual");
        panic!("{name}: {differing} of {total} pixels differ; actual saved to {}", out.display());
    }
}

/// Renders `build` in a window of the given size, checks the snapshot and the accessibility tree.
fn scene<V: Render + 'static>(
    name: &str,
    (w, h): (f32, f32),
    preferences: Preferences,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) {
    let mut cx = harness();
    cx.update(|cx| Settings::install(cx, Theme::derive(&palette(), preferences)));
    let window = cx.open_window(size(px(w), px(h)), build).expect("window");
    cx.run_until_parked();
    let handle: AnyWindowHandle = window.into();
    // Real apps set a title through WindowOptions; it names the root node of the tree.
    cx.update_window(handle, |_, window, _| {
        window.set_window_title(&format!("nie-ui scene {name}"));
        window.refresh();
    })
    .ok();
    // Two frames: the first lays out, the second paints with measured sizes.
    for _ in 0..2 {
        cx.update_window(handle, |_, _, _| ()).ok();
        cx.run_until_parked();
    }
    let image = cx.capture_screenshot(handle).expect("screenshot");
    compare(name, &image);
    let json = cx.update_window(handle, |_, window, _| window.debug_a11y_tree_json()).expect("window");
    let json = json.expect("the window produced no accessibility tree");
    let report = audit(&json).expect("audit");
    std::fs::write(std::env::temp_dir().join(format!("nie-ui-{name}-a11y.json")), &json).ok();
    assert!(report.named_roles > 0, "{name}: no named roles in {} nodes", report.nodes);
    assert!(report.is_clean(), "{name}: unnamed nodes {:?}", report.unnamed);
}

fn rows(n: usize) -> Vec<TreeRow> {
    (0..n)
        .map(|i| TreeRow {
            id: SharedString::from(format!("n{i}")),
            label: SharedString::from(["common", "menu", "50_title", "title.cfg.bin", "dx11", "font", "gaiji_game.g4tx"][i % 7]),
            depth: [0, 1, 2, 2, 0, 1, 1][i % 7],
            branch: [true, true, true, false, true, true, false][i % 7],
            expanded: [true, true, false, false, true, false, false][i % 7],
        })
        .collect()
}

#[test]
fn tree() {
    scene("tree", (420.0, 260.0), Preferences::default(), |_, cx| {
        cx.new(|cx| {
            let mut t = Tree::new("Game files", cx);
            t.set_rows(rows(14), cx);
            t
        })
    });
}

#[test]
fn tree_high_contrast_scaled() {
    let prefs = Preferences { high_contrast: true, scale: 1.5, reduce_motion: true };
    scene("tree-high-contrast", (520.0, 340.0), prefs, |_, cx| {
        cx.new(|cx| {
            let mut t = Tree::new("Game files", cx);
            t.set_rows(rows(14), cx);
            t
        })
    });
}

#[test]
fn table() {
    scene("table", (520.0, 260.0), Preferences::default(), |_, cx| {
        cx.new(|cx| {
            let mut t = Table::new("Characters", cx);
            t.set(
                ["id", "name", "position", "level"].map(SharedString::from).to_vec(),
                (0..12)
                    .map(|i| {
                        vec![
                            SharedString::from(format!("0x{:04X}", 0x1000 + i)),
                            SharedString::from(format!("Player {i}")),
                            SharedString::from(["GK", "DF", "MF", "FW"][i % 4]),
                            SharedString::from(format!("{}", 40 + i * 3)),
                        ]
                    })
                    .collect(),
                cx,
            );
            t
        })
    });
}

struct TabsScene;

impl Render for TabsScene {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Tabs::new("Views", ["Explorer", "Viewer", "Wiki"].map(SharedString::from).to_vec(), 1)
            .content(div().p_4().child("Viewer panel"))
    }
}

#[test]
fn tabs() {
    scene("tabs", (420.0, 160.0), Preferences::default(), |_, cx| cx.new(|_| TabsScene));
}

#[test]
fn picker() {
    scene("picker", (460.0, 300.0), Preferences::default(), |_, cx| {
        cx.new(|cx| {
            let items: Vec<SharedString> = [
                "data/common/menu/50_title/title.cfg.bin",
                "data/common/menu/60_story/story.cfg.bin",
                "data/dx11/menu/50_title/title.g4tx",
                "data/common/font/font_color.cfg.bin",
                "data/dx11/font/gaiji_game.g4tx",
            ]
            .map(SharedString::from)
            .to_vec();
            let mut p = Picker::new("Find a file", items, cx);
            p.set_query("title", cx);
            p
        })
    });
}

struct Checker;

impl FrameSource for Checker {
    fn fps(&self) -> u32 {
        0
    }
    fn count(&self) -> Option<u64> {
        Some(1)
    }
    fn frame(&mut self, _: u64) -> Option<Rgba8> {
        let (w, h) = (64u32, 48u32);
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let on = (x / 8 + y / 8) % 2 == 0;
                rgba.extend_from_slice(&if on { [235, 70, 80, 255] } else { [80, 150, 240, 255] });
            }
        }
        Some(Rgba8 { width: w, height: h, rgba })
    }
}

#[test]
fn surface() {
    scene("surface", (320.0, 240.0), Preferences::default(), |_, cx| {
        cx.new(|_| FrameSurface::new("checker", "Checkerboard test picture", Checker))
    });
}

#[test]
fn terminal() {
    scene("terminal", (480.0, 200.0), Preferences::default(), |_, cx| {
        cx.new(|cx| {
            let mut t = TerminalView::new("Terminal", cx);
            let mut grid = TermGrid::blank(48, 6);
            for (r, line) in ["$ nie fm --snapshot 80x24", "data/common/menu", "  50_title", "  60_story"].iter().enumerate() {
                for (c, ch) in line.chars().enumerate() {
                    grid.rows[r][c] = TermCell { ch, fg: (r == 0).then_some(Rgb(70, 200, 110)), ..TermCell::default() };
                }
            }
            grid.cursor = Some((4, 0));
            grid.title = "nie".into();
            t.set_grid(grid, cx);
            t
        })
    });
}

struct Fixed(Frame);

impl Backend for Fixed {
    fn set_height(&mut self, _: usize) {}
    fn key(&mut self, _: &KeyInput) {}
    fn poll(&mut self) -> bool {
        false
    }
    fn frame(&self) -> Frame {
        self.0.clone()
    }
}

fn entries(names: &[(&str, EntryKind, &str)]) -> Vec<Entry> {
    names
        .iter()
        .map(|(n, k, d)| Entry { name: SharedString::from(*n), kind: *k, detail: SharedString::from(*d), marked: false })
        .collect()
}

#[test]
fn explorer() {
    use EntryKind::{Dir, File};
    let parent = ColumnData {
        title: "data/common".into(),
        rows: entries(&[("font", Dir, "12 files"), ("menu", Dir, "340 files"), ("text", Dir, "90 files")]),
        cursor: Some(1),
        total: 3,
    };
    let current = ColumnData {
        title: "data/common/menu".into(),
        rows: entries(&[("50_title", Dir, "8 files"), ("60_story", Dir, "5 files"), ("menu.cfg.bin", File, "12.4 KiB")]),
        cursor: Some(2),
        total: 3,
    };
    let preview = PreviewData {
        path: "data/common/menu/menu.cfg.bin".into(),
        kind: "table".into(),
        meta: vec![("records".into(), "3".into())],
        body: PreviewBody::Table {
            columns: vec!["id".into(), "name".into()],
            rows: vec![vec!["1".into(), "title".into()], vec!["2".into(), "story".into()]],
            total: 3,
        },
    };
    let frame = Frame { cwd: "data/common/menu".into(), parent, current, preview: Some(preview), status: "3 files".into() };
    scene("explorer", (900.0, 360.0), Preferences::default(), move |_, cx| {
        cx.new(|cx| Explorer::new(Fixed(frame), cx))
    });
    let _ = (Syntax::Plain, ());
}
