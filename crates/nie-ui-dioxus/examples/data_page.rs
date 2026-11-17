//! `data_page <source> <internal_path> <out.html> [max_rows]`: renders one cfg.bin from the VFS
//! as an HTML page through the data-driven UI tree.

use nie_formats::vfs::Vfs;
use nie_ui_dioxus::{Theme, render_html, screen_from_dataset};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let [src, path, out, rest @ ..] = a.as_slice() else {
        panic!("usage: data_page <source> <internal_path> <out.html> [max_rows]");
    };
    let max_rows = rest.first().and_then(|s| s.parse().ok()).unwrap_or(12);
    let mut vfs = Vfs::new();
    vfs.init_source(src).expect("init VFS");
    let bytes = vfs.read(path).expect("read");
    let data = nie_cfgsql::from_bytes(&bytes).expect("decode");
    let ui = screen_from_dataset(&data, path, max_rows, &Theme::default());
    std::fs::write(out, render_html(&ui, path)).expect("write");
    eprintln!("nodes={} tables={}", ui.count(), data.tables.len());
}
