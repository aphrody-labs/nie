//! The Lua-driven pilot: header tabs decided by Lua 5.2 (the game's VM), drawn by nie-ui, rendered
//! offscreen and audited. With `NIE_GAME_SOURCE` set, the tabs come from the real main menu
//! bytecode of the mounted game instead of the synthetic script, and the picture is written to
//! `NIE_LUA_SHOT`.
#![cfg(target_os = "linux")]

use std::borrow::Cow;
use std::sync::Arc;

use gpui::{AppContext as _, HeadlessAppContext, px, size};
use nie_apps::lua_tabs::{LuaTabs, main_menu_script, tabs_from_bytecode, tabs_from_source};
use nie_ui::prelude::*;

const FONTS: [&[u8]; 2] = [
    include_bytes!("../../../../aphrody/assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf"),
    include_bytes!("../../../../aphrody/assets/fonts/ibm-plex-sans/IBMPlexSans-SemiBold.ttf"),
];

/// The structure of `main_menu`: the tab table is chosen by Lua, the object and text hashes come
/// from two lookup functions.
const SCRIPT: &str = r#"
MAIN_MENU = {}
function MAIN_MENU.IsChronicleModeMainMenu() return funcLuaCommand(1) end
local NORMAL = {10,20,30,70,40,80,60,50,90}
local CHRON  = {10,20,60,50,90}
function GetSortOfTabs()
  if MAIN_MENU.IsChronicleModeMainMenu() then return CHRON end
  return NORMAL
end
function GetMenuObjectNameFromTabType(t)
  local m = {[10]=1730641026,[20]=3040181864,[30]=3607292371,[40]=283631169,
             [50]=3165722229,[60]=948899592,[70]=1598948986,[80]=37450337,[90]=613075359}
  return m[t] or 0
end
function GetTabTextIdCRC(t)
  local m = {[10]=2074787642,[20]=1059912976,[30]=3955157337,[40]=2378443629,
             [50]=1689681834,[60]=682656930,[70]=3095252521,[80]=1629403576,[90]=3754313428}
  return m[t] or 0
end
"#;

fn palette() -> Vec<nie_data::font_color::FontColor> {
    use nie_data::hash::HashId;
    [(18, 22, 42), (245, 245, 235), (235, 70, 80), (70, 200, 110), (240, 190, 60), (80, 150, 240), (180, 110, 230), (60, 200, 180), (130, 134, 150)]
        .iter()
        .enumerate()
        .map(|(i, rgb)| nie_data::font_color::FontColor { id: HashId(i as u32), rgb: *rgb, rubi_rgb: *rgb })
        .collect()
}

fn render(source: &str, tabs: Vec<nie_lua::HeaderTab>) -> (image::RgbaImage, nie_ui::a11y::Report) {
    let text = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts("IBM Plex Sans"));
    let mut cx = HeadlessAppContext::with_platform(text, Arc::new(()), gpui_platform::current_headless_renderer);
    cx.text_system().add_fonts(FONTS.iter().map(|f| Cow::Borrowed(*f)).collect()).expect("fonts");
    cx.update(|cx| Settings::install(cx, Theme::derive(&palette(), Preferences::default())));
    let source = source.to_owned();
    let window = cx.open_window(size(px(1100.0), px(200.0)), |_, cx| cx.new(|_| LuaTabs::new(source, tabs))).expect("window");
    cx.run_until_parked();
    let handle: gpui::AnyWindowHandle = window.into();
    cx.update_window(handle, |_, w, _| {
        w.set_window_title("nie lua tabs");
        w.refresh();
    })
    .ok();
    cx.run_until_parked();
    let image = cx.capture_screenshot(handle).expect("screenshot");
    let json = cx.update_window(handle, |_, w, _| w.debug_a11y_tree_json()).expect("window").expect("a11y tree");
    (image, nie_ui::a11y::audit(&json).expect("audit"))
}

#[test]
fn the_tabs_are_the_ones_lua_returns() {
    let tabs = tabs_from_source(SCRIPT).expect("script");
    let types: Vec<i64> = tabs.iter().map(|t| t.tab_type).collect();
    assert_eq!(types, [10, 20, 30, 70, 40, 80, 60, 50, 90], "the normal table, not the chronicle one");
    let (image, report) = render("synthetic main_menu script", tabs);
    assert!(report.is_clean(), "unnamed: {:?}", report.unnamed);
    assert_eq!(report.named_roles > 9, true, "nine Tab nodes at least");
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/lua-tabs.png");
    if std::env::var_os("NIE_UI_UPDATE").is_some() || !path.exists() {
        image.save(&path).expect("write snapshot");
        return;
    }
    let expected = image::open(&path).expect("read").to_rgba8();
    assert_eq!(expected.dimensions(), image.dimensions());
    let differing = expected.pixels().zip(image.pixels()).filter(|(a, b)| a.0.iter().zip(b.0.iter()).any(|(x, y)| x.abs_diff(*y) > 12)).count();
    assert!(differing * 1000 <= (image.width() * image.height()) as usize, "{differing} pixels differ");
}

#[test]
fn the_real_main_menu_decides_the_tabs() {
    let Some(source) = std::env::var("NIE_GAME_SOURCE").ok() else {
        eprintln!("NIE_GAME_SOURCE is not set: skipped");
        return;
    };
    let vfs = nie_formats::vfs::open_source(Some(&source)).expect("mount the game");
    let path = main_menu_script(&vfs).expect("a main_menu script in the VFS");
    let bytes = vfs.read(&path).expect("read the script");
    let tabs = tabs_from_bytecode(&bytes, &path).expect("run the real script");
    eprintln!("{path}: {} tabs {:?}", tabs.len(), tabs.iter().map(|t| t.tab_type).collect::<Vec<_>>());
    assert!(!tabs.is_empty(), "the real script returned no tabs");
    let (image, report) = render(&path, tabs);
    assert!(report.is_clean(), "unnamed: {:?}", report.unnamed);
    if let Some(out) = std::env::var_os("NIE_LUA_SHOT") {
        image.save(out).expect("write the picture");
    }
}
