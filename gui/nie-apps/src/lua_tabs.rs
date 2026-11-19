//! Pilot of the Lua-driven views: the header tabs of the game's main menu, decided by the game's
//! own Lua and drawn by nie-ui.
//!
//! The game runs its menus as Lua 5.2 bytecode (`data/common/script/lua/**.lua.bin`). `nie-lua` runs
//! that bytecode in the real Lua 5.2 VM with the menu host installed. The main menu script defines
//! `GetSortOfTabs`, `GetMenuObjectNameFromTabType` and `GetTabTextIdCRC`; this view calls them
//! through [`nie_lua::enumerate_header_tabs`] and draws one [`nie_ui::tabs::Tabs`] strip from the
//! answer, in the order and with the object and text hashes the script returns. Nothing about the
//! tabs is written here.

use anyhow::{Result, anyhow};
use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, Render, Role, SharedString, Styled, Window, div,
    prelude::*,
};
use nie_lua::{HeaderTab, enumerate_header_tabs, install_menu_host, load_bytecode, new_vm};
use nie_ui::prelude::*;

/// Runs a Lua 5.2 bytecode script (a `.lua.bin` of the VFS) and asks it for its header tabs.
///
/// # Errors
/// The bytecode is invalid or its top level raises.
pub fn tabs_from_bytecode(bytes: &[u8], name: &str) -> Result<Vec<HeaderTab>> {
    let lua = new_vm();
    let _state = install_menu_host(&lua).map_err(|e| anyhow!("install the menu host: {e}"))?;
    // The top level defines the functions; `OnOpenLayer` is not needed to read the tabs.
    let main = load_bytecode(&lua, bytes, name).map_err(|e| anyhow!("load {name}: {e}"))?;
    main.call::<()>(()).map_err(|e| anyhow!("run the top level of {name}: {e}"))?;
    Ok(enumerate_header_tabs(&lua))
}

/// The same through Lua source text, for hosts without a game (tests).
///
/// # Errors
/// The source does not run.
pub fn tabs_from_source(source: &str) -> Result<Vec<HeaderTab>> {
    let lua = new_vm();
    let _state = install_menu_host(&lua).map_err(|e| anyhow!("install the menu host: {e}"))?;
    lua.load(source).exec().map_err(|e| anyhow!("run the script: {e}"))?;
    Ok(enumerate_header_tabs(&lua))
}

/// Finds the main menu script of a mounted game: `data/common/script/lua/**/main_menu_<version>.lua.bin`.
#[must_use]
pub fn main_menu_script(vfs: &nie_formats::vfs::Vfs) -> Option<String> {
    let mut found: Vec<String> = vfs
        .iter()
        .map(|(p, _)| p)
        .filter(|p| {
            p.starts_with("data/common/script/lua/")
                && p.rsplit('/').next().is_some_and(|f| f.starts_with("main_menu_") && !f.starts_with("main_menu_inc") && f.ends_with(".lua.bin"))
        })
        .map(str::to_owned)
        .collect();
    found.sort();
    found.into_iter().next()
}

/// The view: a tab strip whose tabs the Lua decided.
pub struct LuaTabs {
    source: SharedString,
    tabs: Vec<HeaderTab>,
    selected: usize,
}

impl LuaTabs {
    /// A view over the tabs a script returned; `source` names the script.
    pub fn new(source: impl Into<SharedString>, tabs: Vec<HeaderTab>) -> Self {
        Self { source: source.into(), tabs, selected: 0 }
    }
}

impl Render for LuaTabs {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Settings::theme(cx).clone();
        let this = cx.entity().downgrade();
        let titles: Vec<SharedString> =
            self.tabs.iter().map(|t| SharedString::from(format!("Tab {}", t.tab_type))).collect();
        let detail = self.tabs.get(self.selected).map_or_else(
            || SharedString::from("The script defines no header tabs."),
            |t| {
                SharedString::from(format!(
                    "tab type {}, object 0x{:08X}, text id 0x{:08X}, position {} of {}",
                    t.tab_type,
                    t.obj_hash,
                    t.text_id,
                    t.index + 1,
                    self.tabs.len()
                ))
            },
        );
        div()
            .id("nie-lua-tabs")
            .role(Role::Application)
            .aria_label(SharedString::from(format!("Header tabs from {}", self.source)))
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background.hsla())
            .text_color(theme.text.hsla())
            .font_family(theme.ui_font)
            .text_size(theme.px(theme.text_size))
            .child(
                Tabs::new("Header tabs", titles, self.selected)
                    .on_select(move |ix, _window, cx| {
                        this.update(cx, |this, cx| {
                            this.selected = ix;
                            cx.notify();
                        })
                        .ok();
                    })
                    .content(
                        div()
                            .id("nie-lua-tab-detail")
                            .role(Role::Status)
                            .aria_label(detail.clone())
                            .p(theme.px(14.0))
                            .flex()
                            .flex_col()
                            .gap(theme.px(8.0))
                            .child(div().text_color(theme.accent.hsla()).child(SharedString::from(format!("from {}", self.source))))
                            .child(detail),
                    ),
            )
    }
}
