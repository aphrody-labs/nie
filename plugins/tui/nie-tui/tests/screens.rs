//! Screen snapshots of `nie fm` on the synthetic game, drawn on a ratatui `TestBackend`.
//!
//! A snapshot lives in `tests/snapshots/<name>.txt`. `NIE_UPDATE_SNAPSHOTS=1 cargo test` rewrites
//! them; without it a difference fails the test and prints both screens.

use std::path::PathBuf;
use std::time::Duration;

use nie_explore::fm::fixture::{self, Fixture};
use nie_explore::fm::keymap::Keymap;
use nie_explore::fm::keys::Key;
use nie_tui::session::Session;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui_image::picker::{Picker, ProtocolType};

const WIDTH: u16 = 110;
const HEIGHT: u16 = 24;

fn session(game: &Fixture, picker: Picker) -> Session {
    let mut session = Session::new(game.vfs.clone(), None, picker, Keymap::builtin());
    session.show_timings(false);
    session
}

fn halfblocks() -> Picker {
    Picker::halfblocks()
}

fn press(session: &mut Session, keys: &str) {
    for key in Key::parse_sequence(keys).expect("keys") {
        session.key(key);
    }
}

async fn screen(session: &mut Session) -> Buffer {
    assert!(session.settle(Duration::from_secs(30)).await, "the preview did not finish");
    session.tick();
    let mut terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap();
    terminal.draw(|frame| session.draw(frame)).unwrap();
    terminal.backend().buffer().clone()
}

fn text(buffer: &Buffer) -> String {
    (0..buffer.area.height)
        .map(|y| {
            let line: String = (0..buffer.area.width).map(|x| buffer[(x, y)].symbol()).collect();
            line.trim_end().to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn assert_snapshot(name: &str, actual: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(format!("{name}.txt"));
    if std::env::var_os("NIE_UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("missing snapshot {}; run with NIE_UPDATE_SNAPSHOTS=1", path.display()))
        .replace("\r\n", "\n");
    assert_eq!(actual, expected, "snapshot `{name}` differs\n--- actual ---\n{actual}\n--- expected ---\n{expected}");
}

#[tokio::test(flavor = "multi_thread")]
async fn three_columns_over_the_menu_directory() {
    let game = fixture::game();
    let mut s = session(&game, halfblocks());
    press(&mut s, "lljl");
    let buffer = screen(&mut s).await;
    let shot = text(&buffer);
    assert!(shot.contains("ui_main.cfg.bin"), "{shot}");
    assert!(shot.contains("UI_ITEM"), "the table preview shows the record: {shot}");
    assert_snapshot("menu_table", &shot);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_texture_atlas_shows_its_sprites() {
    let game = fixture::game();
    let mut s = session(&game, halfblocks());
    press(&mut s, "ljll");
    let buffer = screen(&mut s).await;
    let shot = text(&buffer);
    assert!(shot.contains("icons.g4tx"), "{shot}");
    assert!(shot.contains("1/2  icon_a"), "{shot}");
    // The half-block picture paints cells with the sprite colours.
    let painted = buffer.content().iter().filter(|c| matches!(c.symbol(), "\u{2580}" | "\u{2584}")).count();
    assert!(painted > 20, "only {painted} half-block cells");
    assert_snapshot("atlas_sprites", &shot);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_fuzzy_search_lists_matching_paths() {
    let game = fixture::game();
    let mut s = session(&game, halfblocks());
    press(&mut s, "/");
    for c in "intro lua".chars() {
        s.key(Key::char(c));
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while s.fm.view().current.total == 0 && std::time::Instant::now() < deadline {
        s.drain();
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let buffer = screen(&mut s).await;
    let shot = text(&buffer);
    assert!(shot.contains("data/common/script/intro.lua.bin"), "{shot}");
    assert_snapshot("search", &shot);
}

#[tokio::test(flavor = "multi_thread")]
async fn lua_bytecode_previews_as_highlighted_call_syntax() {
    let game = fixture::game();
    let mut s = session(&game, halfblocks());
    s.fm.reveal_path("data/common/script/intro.lua.bin");
    s.sync();
    let buffer = screen(&mut s).await;
    let shot = text(&buffer);
    assert!(shot.contains("local function main()"), "{shot}");
    assert!(shot.contains("LOADK(0, 0)"), "{shot}");
    // The call name is coloured as a function, the string as a string.
    let function = nie_tui::theme::style(nie_explore::fm::preview::Style::Function).fg;
    let colours: Vec<_> = buffer
        .content()
        .iter()
        .filter(|c| c.symbol() == "L")
        .map(|c| c.style().fg)
        .collect();
    assert!(colours.contains(&function), "{colours:?}");
    assert_snapshot("lua_bytecode", &shot);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_help_screen_lists_the_keys() {
    let game = fixture::game();
    let mut s = session(&game, halfblocks());
    press(&mut s, "?");
    let shot = text(&screen(&mut s).await);
    assert!(shot.contains("Fuzzy search every path"), "{shot}");
    assert!(shot.contains("keys (? to close)"), "{shot}");
}

/// Kitty draws with a random image id: `i=123` in the transmission and, on the unicode
/// placeholder cells that follow it, the third diacritic after U+10EEEE. Drop both so two runs
/// compare byte for byte (the placeholder, its row and its column stay).
fn without_kitty_ids(stream: &str) -> String {
    let mut out = String::with_capacity(stream.len());
    let mut rest = stream;
    while let Some(at) = rest.find("\u{1b}_G") {
        let (head, tail) = rest.split_at(at + 3);
        out.push_str(head);
        let end = tail.find(';').unwrap_or(tail.len());
        let params: Vec<&str> = tail[..end].split(',').filter(|p| !p.starts_with("i=")).collect();
        out.push_str(&params.join(","));
        let tail = &tail[end..];
        let stop = tail.find("\u{1b}\\").map_or(tail.len(), |t| t + 2);
        out.push_str(&tail[..stop]);
        let after = &tail[stop..];
        let next = after.find('\u{1b}').unwrap_or(after.len());
        out.extend(after[..next].chars().take(3));
        rest = &after[next..];
    }
    out.push_str(rest);
    out
}

fn escapes(buffer: &Buffer) -> Vec<String> {
    buffer
        .content()
        .iter()
        .map(|c| c.symbol().to_owned())
        .filter(|s| s.contains('\u{1b}'))
        .collect()
}

fn graphics_hash(protocol: ProtocolType) -> (usize, String) {
    let game = fixture::game();
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_time().build().unwrap();
    runtime.block_on(async {
        #[allow(deprecated)]
        let mut picker = Picker::from_fontsize(ratatui_image::FontSize::new(8, 16));
        picker.set_protocol_type(protocol);
        let mut s = session(&game, picker);
        s.fm.reveal_path("data/dx11/menu/plain.g4tx");
        s.sync();
        let buffer = screen(&mut s).await;
        let streams = escapes(&buffer);
        assert!(!streams.is_empty(), "{protocol:?} emitted no escape stream");
        let all = without_kitty_ids(&streams.concat());
        (streams.len(), blake3::hash(all.as_bytes()).to_hex().to_string())
    })
}

#[test]
fn kitty_and_sixel_emit_stable_escape_streams() {
    let (_, kitty) = graphics_hash(ProtocolType::Kitty);
    let (_, kitty_again) = graphics_hash(ProtocolType::Kitty);
    assert_eq!(kitty, kitty_again, "the kitty stream is deterministic");
    let (_, sixel) = graphics_hash(ProtocolType::Sixel);
    let (_, sixel_again) = graphics_hash(ProtocolType::Sixel);
    assert_eq!(sixel, sixel_again, "the sixel stream is deterministic");
    assert_ne!(kitty, sixel);
}
