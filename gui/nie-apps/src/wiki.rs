//! The wiki: the game's characters, skills and teams as computed from the VFS by `nie-wiki-vfs`,
//! in a table with the selected record's fields beside it. Every field is a value of a VFS table;
//! a record's source (VFS path and table) is listed with its fields.

use std::sync::Arc;

use gpui::{
    App, AppContext as _, Bounds, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, Role,
    SharedString, Styled, Subscription, Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use nie_ui::prelude::*;
use nie_wiki_vfs::{Query, Wiki};
use serde_json::Value;

/// Rows fetched per family.
const LIMIT: usize = 400;
const FAMILIES: [&str; 3] = ["Characters", "Skills", "Teams"];

struct Record {
    title: String,
    subtitle: String,
    fields: Vec<(String, String)>,
}

/// The first text of a `Localized` value, English first.
fn localized(v: &Value) -> Option<String> {
    let texts = v.get("texts")?.as_object()?;
    let lang = texts.get("en").or_else(|| texts.values().next())?;
    lang["forms"].as_array()?.iter().find(|f| f["form"] == 0).or_else(|| lang["forms"].get(0))?["text"].as_str().map(str::to_owned)
}

fn flatten(prefix: &str, v: &Value, out: &mut Vec<(String, String)>) {
    if let Some(text) = localized(v) {
        out.push((prefix.to_owned(), text));
        return;
    }
    match v {
        Value::Object(map) => {
            for (k, x) in map {
                let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                flatten(&key, x, out);
            }
        }
        Value::Array(items) => {
            for (i, x) in items.iter().enumerate() {
                flatten(&format!("{prefix}[{i}]"), x, out);
            }
        }
        Value::String(s) => out.push((prefix.to_owned(), s.clone())),
        other => out.push((prefix.to_owned(), other.to_string())),
    }
}

fn record(item: &Value) -> Record {
    let title = ["name", "family_name"].iter().find_map(|k| item.get(*k).and_then(localized)).or_else(|| {
        ["skill_id_str", "internal_code"].iter().find_map(|k| item[*k].as_str().map(str::to_owned))
    });
    let subtitle = ["internal_code", "skill_id_str", "belong_team_id"]
        .iter()
        .find_map(|k| match &item[*k] {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        })
        .unwrap_or_default();
    let mut fields = Vec::new();
    flatten("", item, &mut fields);
    Record { title: title.unwrap_or_else(|| subtitle.clone()), subtitle, fields }
}

fn load(wiki: &Wiki, family: usize) -> anyhow::Result<(usize, Vec<Record>)> {
    let query = Query { limit: Some(LIMIT), ..Query::default() };
    let (total, items) = match family {
        0 => {
            let p = wiki.characters(&query)?;
            (p.total, p.items.iter().map(serde_json::to_value).collect::<Result<Vec<_>, _>>()?)
        }
        1 => {
            let p = wiki.skills(&query)?;
            (p.total, p.items.iter().map(serde_json::to_value).collect::<Result<Vec<_>, _>>()?)
        }
        _ => {
            let p = wiki.teams(&query)?;
            (p.total, p.items.iter().map(serde_json::to_value).collect::<Result<Vec<_>, _>>()?)
        }
    };
    Ok((total, items.iter().map(record).collect()))
}

/// The wiki view.
pub struct WikiApp {
    wiki: Arc<Wiki>,
    family: usize,
    records: Vec<Record>,
    total: usize,
    list: Entity<Table>,
    detail: Entity<Table>,
    status: SharedString,
    _subscription: Subscription,
}

impl WikiApp {
    /// A wiki over a mounted game.
    pub fn new(wiki: Arc<Wiki>, cx: &mut Context<Self>) -> Self {
        let list = cx.new(|cx| Table::new("Records", cx));
        let detail = cx.new(|cx| Table::new("Fields of the selected record", cx));
        let subscription = cx.subscribe(&list, |this, _, event: &TableEvent, cx| {
            let TableEvent::Selected(ix) = event;
            this.show(*ix, cx);
        });
        let mut app = Self {
            wiki,
            family: 0,
            records: Vec::new(),
            total: 0,
            list,
            detail,
            status: SharedString::default(),
            _subscription: subscription,
        };
        app.open_family(0, cx);
        app
    }

    fn open_family(&mut self, family: usize, cx: &mut Context<Self>) {
        self.family = family;
        self.status = SharedString::from(format!("Loading {}...", FAMILIES[family]));
        cx.notify();
        let wiki = Arc::clone(&self.wiki);
        cx.spawn(async move |this, cx| {
            let loaded = cx.background_spawn(async move { load(&wiki, family) }).await;
            this.update(cx, |this, cx| {
                if this.family != family {
                    return;
                }
                match loaded {
                    Ok((total, records)) => {
                        this.total = total;
                        this.records = records;
                        this.status = SharedString::from(format!("{} of {} {}", this.records.len(), total, FAMILIES[family]));
                    }
                    Err(e) => this.status = SharedString::from(format!("{}: {e}", FAMILIES[family])),
                }
                let rows = this
                    .records
                    .iter()
                    .map(|r| vec![SharedString::from(r.title.clone()), SharedString::from(r.subtitle.clone())])
                    .collect();
                this.list.update(cx, |t, cx| t.set(vec!["name".into(), "code".into()], rows, cx));
                this.show(0, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn show(&mut self, ix: usize, cx: &mut Context<Self>) {
        let rows = self
            .records
            .get(ix)
            .map(|r| r.fields.iter().map(|(k, v)| vec![SharedString::from(k.clone()), SharedString::from(v.clone())]).collect())
            .unwrap_or_default();
        self.detail.update(cx, |t, cx| t.set(vec!["field".into(), "value".into()], rows, cx));
    }
}

impl Render for WikiApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Settings::theme(cx).clone();
        let this = cx.entity().downgrade();
        let tabs = Tabs::new("Wiki families", FAMILIES.map(SharedString::from).to_vec(), self.family)
            .on_select(move |ix, _window, cx| {
                this.update(cx, |this, cx| this.open_family(ix, cx)).ok();
            })
            .content(
                div()
                    .size_full()
                    .flex()
                    .child(div().w_2_5().h_full().child(self.list.clone()))
                    .child(div().flex_1().h_full().child(self.detail.clone())),
            );
        div()
            .id("nie-wiki")
            .role(Role::Application)
            .aria_label("nie wiki")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background.hsla())
            .text_color(theme.text.hsla())
            .child(div().flex_1().min_h_0().child(tabs))
            .child(
                div()
                    .id("nie-wiki-status")
                    .role(Role::Status)
                    .aria_label(self.status.clone())
                    .h(theme.px(28.0))
                    .px(theme.px(10.0))
                    .flex()
                    .items_center()
                    .border_t_1()
                    .border_color(theme.border.hsla())
                    .text_color(theme.text_dim.hsla())
                    .font_family(theme.ui_font)
                    .child(self.status.clone()),
            )
    }
}

/// Opens the wiki window.
pub fn open(cx: &mut App, wiki: Arc<Wiki>) {
    let bounds = Bounds::centered(None, size(px(1280.0), px(760.0)), cx);
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(gpui::TitlebarOptions { title: Some("nie wiki".into()), ..Default::default() }),
            ..Default::default()
        },
        |_, cx| cx.new(|cx| WikiApp::new(wiki, cx)),
    );
    if let Err(error) = opened {
        eprintln!("nie-wiki: cannot open a window: {error:#}");
        cx.quit();
    }
}
