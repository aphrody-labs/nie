//! Server-rendered pages: semantic HTML with no script. Every page names the VFS path, table and
//! index behind its values and shows an explicit empty state for what the VFS does not give.

use std::fmt::Write as _;

use serde_json::Value;

use crate::model::{
    CatalogEntry, Character, CharacterClass, ClassCount, ClassRole, Coded, Emblem, Language,
    LearnedSkill, Localized, NoSource, Provenance, RefSlice, Skill, SkillFamily, TableInfo,
    TableRecord, Team,
};
use crate::wiki::{Page, Source};

const STYLE: &str = "body{font:16px/1.5 system-ui,sans-serif;margin:0 auto;max-width:72rem;padding:0 1rem 3rem}\
nav,footer{color:#555;font-size:.9rem}nav a{margin-right:.75rem}table{border-collapse:collapse;width:100%;margin:.5rem 0 1.5rem}\
th,td{border:1px solid #ccc;padding:.25rem .5rem;text-align:left;vertical-align:top}th{background:#f3f3f3}\
code,pre{font-family:ui-monospace,monospace;font-size:.85rem}.empty{border:1px dashed #999;padding:.5rem 1rem;color:#444}\
.prov{color:#555;font-size:.85rem}img{vertical-align:middle;max-height:3rem}dt{font-weight:600}dl{display:grid;grid-template-columns:max-content 1fr;gap:.25rem 1rem}\
@media(prefers-color-scheme:dark){body{background:#111;color:#ddd}th{background:#222}th,td{border-color:#444}nav,footer,.prov{color:#aaa}a{color:#8ab4f8}}";

fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// A path segment of a URL.
fn seg(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// The URL of an emblem texture on the asset origin served beside this host:
/// `data/dx11/menu/200_icon/01_icon_emblem/em010001.g4tx` becomes
/// `/tex/dx11/menu/200_icon/01_icon_emblem/em010001.png`.
#[must_use]
pub fn texture_url(vfs_path: &str) -> Option<String> {
    let rel = vfs_path.strip_prefix("data/").unwrap_or(vfs_path);
    let stem = rel.strip_suffix(".g4tx")?;
    Some(format!("/tex/{stem}.png"))
}

/// Which page is rendered, for the language switcher and the links.
pub struct Ctx<'a> {
    pub languages: &'a [Language],
    pub lang: &'a Language,
    /// The path after `/wiki/<lang>/`, for example `characters` or `skills/whs00010`.
    pub rest: &'a str,
}

/// The list parameters a page links back with.
#[derive(Debug, Clone, Default)]
pub struct ListParams {
    pub text: Option<String>,
    pub offset: usize,
    pub limit: usize,
}

fn layout(ctx: Option<&Ctx<'_>>, languages: &[Language], title: &str, body: &str) -> String {
    let tag = ctx.map_or("en", |c| c.lang.tag.as_str());
    let mut nav = String::from("<a href=\"/wiki\">Wiki</a>");
    if let Some(ctx) = ctx {
        for family in ["characters", "skills", "teams", "tables"] {
            let _ = write!(
                nav,
                "<a href=\"/wiki/{}/{family}\">{family}</a>",
                esc(&ctx.lang.tag)
            );
        }
    }
    let mut switch = String::new();
    for language in languages {
        let href = match ctx {
            Some(ctx) => format!("/wiki/{}/{}", seg(&language.tag), ctx.rest),
            None => format!("/wiki/{}/characters", seg(&language.tag)),
        };
        let _ = write!(
            switch,
            "<a href=\"{}\" hreflang=\"{}\" lang=\"{}\">{}</a> ",
            esc(&href),
            esc(&language.tag),
            esc(&language.tag),
            esc(&language.tag)
        );
    }
    format!(
        "<!doctype html>\n<html lang=\"{}\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<title>{}</title><style>{STYLE}</style></head><body>\
<header><nav>{nav}</nav><nav aria-label=\"Language\">{switch}</nav></header>\
<main>{body}</main>\
<footer><p>Computed from the installation's VFS at request time. Values are the game's, \
labels are its own texts; a field with no source is absent.</p></footer></body></html>",
        esc(tag),
        esc(title),
    )
}

fn prov(p: &Provenance) -> String {
    format!(
        "<code>{}</code> table <code>{}</code> entry {}",
        esc(&p.vfs_path),
        esc(&p.table),
        p.index
    )
}

fn empty(state: &NoSource) -> String {
    format!(
        "<p class=\"empty\"><strong>{}</strong>: no source in the VFS. {}.</p>",
        esc(state.family),
        esc(state.reason)
    )
}

/// The label of `localized` in `lang`, if any.
fn label<'a>(localized: Option<&'a Localized>, lang: &Language) -> Option<&'a str> {
    localized?.text(&lang.folder)
}

fn name_or_code(c: &Character, lang: &Language) -> String {
    label(c.name.as_ref(), lang).map_or_else(
        || {
            c.internal_code
                .clone()
                .unwrap_or_else(|| c.chara_base_id.to_string())
        },
        str::to_owned,
    )
}

fn coded(value: &Coded, lang: &Language) -> String {
    match label(value.label.as_ref(), lang) {
        Some(text) => format!("{} (code <code>{}</code>)", esc(text), value.value),
        None => format!(
            "<code>{}</code> <span class=\"prov\">stored code, no label in the VFS</span>",
            value.value
        ),
    }
}

/// Every language's entries for one text key, with the file, section and index they come from.
fn texts_table(localized: &Localized) -> String {
    let mut out = String::from(
        "<table><thead><tr><th>Language</th><th>Form</th><th>Text</th><th>Source</th></tr></thead><tbody>",
    );
    for (language, text) in &localized.texts {
        for form in &text.forms {
            let _ = write!(
                out,
                "<tr><td>{}</td><td>{}</td><td lang=\"{}\">{}</td><td class=\"prov\"><code>{}</code> {} entry {}</td></tr>",
                esc(language),
                form.form,
                esc(language),
                esc(&form.text).replace('\n', "<br>"),
                esc(&text.vfs_path),
                esc(&text.section),
                form.index
            );
        }
    }
    out.push_str("</tbody></table>");
    out
}

fn localized_block(heading: &str, localized: Option<&Localized>) -> String {
    match localized {
        Some(l) => format!(
            "<h3>{}</h3><p class=\"prov\">text key <code>{}</code> in <code>{}</code></p>{}",
            esc(heading),
            l.id,
            esc(&l.table),
            texts_table(l)
        ),
        None => format!(
            "<h3>{}</h3><p class=\"empty\">No text in the VFS.</p>",
            esc(heading)
        ),
    }
}

fn fields_table(fields: &std::collections::BTreeMap<String, Value>) -> String {
    let mut out =
        String::from("<table><thead><tr><th>Field</th><th>Stored value</th></tr></thead><tbody>");
    for (name, value) in fields {
        let _ = write!(
            out,
            "<tr><td><code>{}</code></td><td><code>{}</code></td></tr>",
            esc(name),
            esc(&value.to_string())
        );
    }
    out.push_str("</tbody></table>");
    out
}

fn pager(base: &str, params: &ListParams, total: usize, shown: usize) -> String {
    let query = |offset: usize| {
        let mut q = format!("?offset={offset}&amp;limit={}", params.limit);
        if let Some(text) = &params.text {
            let _ = write!(q, "&amp;q={}", seg(text));
        }
        q
    };
    let mut out = format!(
        "<p>{total} match(es); showing {}-{}.</p><p>",
        if shown == 0 { 0 } else { params.offset + 1 },
        params.offset + shown
    );
    if params.offset > 0 {
        let _ = write!(
            out,
            "<a rel=\"prev\" href=\"{base}{}\">Previous</a> ",
            query(params.offset.saturating_sub(params.limit))
        );
    }
    if params.offset + shown < total {
        let _ = write!(
            out,
            "<a rel=\"next\" href=\"{base}{}\">Next</a>",
            query(params.offset + params.limit)
        );
    }
    out.push_str("</p>");
    out
}

fn search(base: &str, params: &ListParams) -> String {
    format!(
        "<form method=\"get\" action=\"{base}\"><label>Search names <input type=\"search\" name=\"q\" value=\"{}\"></label> \
<input type=\"hidden\" name=\"limit\" value=\"{}\"> <button type=\"submit\">Search</button></form>",
        esc(params.text.as_deref().unwrap_or("")),
        params.limit
    )
}

/// The page key of a character: the `chara_param` id, else `base-<n>` of its `chara_base` entry.
#[must_use]
pub fn character_key(c: &Character) -> String {
    match (c.chara_param_id, &c.base_source) {
        (Some(id), _) => id.to_string(),
        (None, Some(base)) => format!("base-{}", base.index),
        (None, None) => c.chara_base_id.to_string(),
    }
}

fn class_text(class: CharacterClass) -> &'static str {
    match class {
        CharacterClass::ParamAndBase => "param and base",
        CharacterClass::ParamWithoutBase => "param without base",
        CharacterClass::BaseWithoutParam => "base without param",
    }
}

/// `/wiki`: the languages, the tables behind each family, the classes of characters, the
/// families without a source, and every gamedata file with its status.
#[must_use]
pub fn index(
    languages: &[Language],
    sources: &[Source],
    classes: &[ClassCount],
    unsourced: &[NoSource],
    catalog: &[CatalogEntry],
) -> String {
    let mut body = String::from(
        "<h1>Inazuma Eleven: Victory Road</h1>\
<p>The game's own data, read from its VFS. Choose a language.</p><ul>",
    );
    for language in languages {
        let tag = seg(&language.tag);
        let _ = write!(
            body,
            "<li lang=\"{0}\"><strong>{0}</strong> (<code>{1}</code>): \
<a href=\"/wiki/{tag}/characters\">characters</a>, <a href=\"/wiki/{tag}/skills\">skills</a>, \
<a href=\"/wiki/{tag}/teams\">teams</a>, <a href=\"/wiki/{tag}/tables\">tables</a></li>",
            esc(&language.tag),
            esc(&language.folder)
        );
    }
    body.push_str("</ul><h2>Backing tables</h2><table><thead><tr><th>Family</th><th>VFS path</th><th>Class</th><th>Entries</th></tr></thead><tbody>");
    for source in sources {
        let _ = write!(
            body,
            "<tr><td>{}</td><td><code>{}</code></td><td><code>{}</code></td><td>{}</td></tr>",
            esc(source.family),
            esc(&source.vfs_path),
            esc(&source.table),
            source.entries
        );
    }
    body.push_str("</tbody></table><h2>Character records by class</h2><table><thead><tr><th>Class</th><th>Records</th><th>Meaning</th></tr></thead><tbody>");
    for class in classes {
        let _ = write!(
            body,
            "<tr><td><code>{}</code></td><td>{}</td><td>{}</td></tr>",
            class_text(class.class),
            class.count,
            esc(class.description)
        );
    }
    body.push_str("</tbody></table><h2>Families without a source</h2>");
    for state in unsourced {
        body.push_str(&empty(state));
    }
    let decoded = catalog.iter().filter(|e| e.status == "decoded").count();
    let _ = write!(
        body,
        "<h2>Gamedata files</h2><p>{} files under <code>data/common/gamedata</code>; {decoded} decoded by a family above, the others present and not decoded.</p><details><summary>List</summary><table><thead><tr><th>VFS path</th><th>Status</th></tr></thead><tbody>",
        catalog.len()
    );
    for entry in catalog {
        let _ = write!(
            body,
            "<tr><td><code>{}</code></td><td>{}</td></tr>",
            esc(&entry.vfs_path),
            esc(entry.status)
        );
    }
    body.push_str("</tbody></table></details>");
    layout(None, languages, "Wiki", &body)
}

/// A page that names what was not found.
#[must_use]
pub fn not_found(languages: &[Language], what: &str) -> String {
    layout(
        None,
        languages,
        "Not found",
        &format!("<h1>Not found</h1><p>{}</p>", esc(what)),
    )
}

/// `/wiki/<lang>/characters`.
#[must_use]
pub fn characters(
    ctx: &Ctx<'_>,
    page: &Page<Character>,
    params: &ListParams,
    classes: &[ClassCount],
) -> String {
    let base = format!("/wiki/{}/characters", seg(&ctx.lang.tag));
    let mut body = format!("<h1>Characters</h1>{}", search(&base, params));
    body.push_str("<p class=\"prov\">");
    for class in classes {
        let _ = write!(
            body,
            "<a href=\"{base}?class={0}&amp;limit={1}\"><code>{0}</code></a>: {2} &mdash; {3}<br>",
            class_slug(class.class),
            params.limit,
            class.count,
            esc(class.description)
        );
    }
    body.push_str("</p>");
    body.push_str(&pager(&base, params, page.total, page.items.len()));
    body.push_str("<table><thead><tr><th>Name</th><th>Romanized</th><th>Code</th><th>Class</th><th>Element</th><th>Main position</th><th>Sub position</th><th>Team</th></tr></thead><tbody>");
    for c in &page.items {
        let team = c.team.as_ref().map_or_else(String::new, |team| {
            let text = label(team.name.as_ref(), ctx.lang)
                .map_or_else(|| team.belong_team_id.to_string(), str::to_owned);
            format!(
                "<a href=\"/wiki/{}/teams/{}\">{}</a>",
                seg(&ctx.lang.tag),
                team.belong_team_id,
                esc(&text)
            )
        });
        let optional = |value: &Option<Coded>| {
            value
                .as_ref()
                .map_or_else(String::new, |v| coded(v, ctx.lang))
        };
        let _ = write!(
            body,
            "<tr><td><a href=\"{base}/{}\">{}</a></td><td>{}</td><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{team}</td></tr>",
            seg(&character_key(c)),
            esc(&name_or_code(c, ctx.lang)),
            esc(label(c.name_roma.as_ref(), ctx.lang).unwrap_or("")),
            esc(c.internal_code.as_deref().unwrap_or("")),
            class_text(c.class),
            optional(&c.element),
            optional(&c.main_position),
            optional(&c.sub_position),
        );
    }
    body.push_str("</tbody></table>");
    body.push_str("<h2>Sources</h2><p class=\"prov\">Every <code>chara_param</code> entry and every <code>chara_base</code> entry is one record; each page names its entries.</p>");
    layout(
        Some(ctx),
        ctx.languages,
        &format!("Characters ({})", ctx.lang.tag),
        &body,
    )
}

fn class_slug(class: CharacterClass) -> &'static str {
    match class {
        CharacterClass::ParamAndBase => "param_and_base",
        CharacterClass::ParamWithoutBase => "param_without_base",
        CharacterClass::BaseWithoutParam => "base_without_param",
    }
}

fn family_stem(family: SkillFamily) -> &'static str {
    match family {
        SkillFamily::Skill => "skill_config",
        SkillFamily::Aura => "aura_skill_config",
        SkillFamily::Passive => "passive_skill_config",
    }
}

fn learned_row(ctx: &Ctx<'_>, skill: &LearnedSkill) -> String {
    let tag = seg(&ctx.lang.tag);
    let name = label(skill.name.as_ref(), ctx.lang);
    let shown = name.map_or_else(|| skill.skill_id.to_string(), str::to_owned);
    let (link, status) = match (&skill.family, &skill.entry) {
        (Some(SkillFamily::Skill), _) => (
            format!("/wiki/{tag}/skills/{}", skill.skill_id),
            String::new(),
        ),
        (Some(family), Some(entry)) => (
            format!(
                "/wiki/{tag}/tables/{}/{}/{}",
                family_stem(*family),
                seg(&entry.table),
                entry.index
            ),
            String::new(),
        ),
        _ => (String::new(), String::new()),
    };
    let note = match (&skill.family, skill.name_text_id, &skill.name) {
        (None, _, _) => {
            "no entry with this id in skill_config, aura_skill_config or passive_skill_config"
                .to_owned()
        }
        (Some(_), None, _) => "the entry has no name text key".to_owned(),
        (Some(_), Some(key), None) => {
            format!("text key {key} has no row in skill_text, in any language")
        }
        (Some(_), Some(_), Some(_)) => status,
    };
    let family = skill.family.map_or("none", |f| family_stem(f));
    let cell = if link.is_empty() {
        esc(&shown)
    } else {
        format!("<a href=\"{}\">{}</a>", esc(&link), esc(&shown))
    };
    let note = if note.is_empty() {
        String::new()
    } else {
        format!("<span class=\"empty\">{}</span>", esc(&note))
    };
    format!(
        "<tr><td>{cell} {note}</td><td>{}</td><td><code>{}</code></td><td>{}</td></tr>",
        skill.learn_level,
        skill.skill_id,
        esc(family)
    )
}

fn slice_block(heading: &str, slice: &RefSlice) -> String {
    let mut out = format!(
        "<h3>{}</h3><p class=\"prov\">entries {}..{} of <code>{}</code></p>",
        esc(heading),
        slice.offset,
        slice.offset + slice.count,
        esc(&slice.class)
    );
    if slice.items.is_empty() {
        out.push_str("<p class=\"empty\">No entry in the slice.</p>");
    } else {
        out.push_str("<table><thead><tr><th>Entry</th><th>Variables</th></tr></thead><tbody>");
        for item in &slice.items {
            let _ = write!(
                out,
                "<tr><td>{}</td><td><code>{}</code></td></tr>",
                item.index,
                esc(&serde_json::to_string(&item.variables).unwrap_or_default())
            );
        }
        out.push_str("</tbody></table>");
    }
    out
}

/// `/wiki/<lang>/characters/<key>`; `c` carries every language's labels.
#[must_use]
pub fn character(ctx: &Ctx<'_>, c: &Character) -> String {
    let title = name_or_code(c, ctx.lang);
    let tag = seg(&ctx.lang.tag);
    let mut body = format!("<h1>{}</h1><dl>", esc(&title));
    let mut row = |term: &str, value: String| {
        let _ = write!(body, "<dt>{}</dt><dd>{value}</dd>", esc(term));
    };
    row("Class", format!("<code>{}</code>", class_text(c.class)));
    if let Some(code) = &c.internal_code {
        row("Internal code", format!("<code>{}</code>", esc(code)));
    }
    match c.chara_param_id {
        Some(id) => row("chara_param id", format!("<code>{id}</code>")),
        None => row(
            "chara_param id",
            "<span class=\"empty\">no chara_param entry names this chara_base entry</span>".into(),
        ),
    }
    row("chara_base id", format!("<code>{}</code>", c.chara_base_id));
    if c.base_source.is_none() {
        row(
            "chara_base entry",
            "<span class=\"empty\">no chara_base entry has this id</span>".into(),
        );
    }
    match &c.gender {
        Some(gender) => row("Gender", coded(gender, ctx.lang)),
        None => row(
            "Gender",
            "<span class=\"empty\">absent from the entry</span>".into(),
        ),
    }
    for (term, value) in [
        ("Element", &c.element),
        ("Main position", &c.main_position),
        ("Sub position", &c.sub_position),
    ] {
        match value {
            Some(value) => row(term, coded(value, ctx.lang)),
            None => row(
                term,
                "<span class=\"empty\">no chara_param entry</span>".into(),
            ),
        }
    }
    if let Some(growth) = c.growth_pattern {
        row("Growth pattern", format!("<code>{growth}</code>"));
    }
    if let Some(sort) = c.sort_index {
        row("Sort index", format!("<code>{sort}</code>"));
    }
    if let Some(series) = &c.series {
        let text = label(series.name.as_ref(), ctx.lang);
        row(
            "Series",
            format!(
                "{} <code>{}</code>{}",
                text.map_or_else(
                    || "<span class=\"empty\">no name text</span>".to_owned(),
                    esc
                ),
                series.series_id,
                series
                    .series_type
                    .map_or_else(String::new, |t| format!(" type <code>{t}</code>"))
            ),
        );
    }
    if let Some(team) = &c.team {
        let text = label(team.name.as_ref(), ctx.lang)
            .map_or_else(|| team.belong_team_id.to_string(), str::to_owned);
        row(
            "Team",
            format!(
                "<a href=\"/wiki/{tag}/teams/{}\">{}</a>",
                team.belong_team_id,
                esc(&text)
            ),
        );
    }
    body.push_str("</dl>");
    if let Some(text) = label(c.description.as_ref(), ctx.lang) {
        let _ = write!(
            body,
            "<h2>Description</h2><p>{}</p>",
            esc(text).replace('\n', "<br>")
        );
    }

    body.push_str("<h2>Learned skills</h2>");
    if c.skills.is_empty() {
        body.push_str("<p class=\"empty\">No skill in the <code>chara_param</code> entry.</p>");
    } else {
        body.push_str("<table><thead><tr><th>Skill</th><th>Level</th><th>Id</th><th>Table</th></tr></thead><tbody>");
        for skill in &c.skills {
            body.push_str(&learned_row(ctx, skill));
        }
        body.push_str("</tbody></table>");
    }

    body.push_str("<h2>Texts in every language</h2>");
    body.push_str(&localized_block("Name", c.name.as_ref()));
    body.push_str(&localized_block("Family name", c.family_name.as_ref()));
    body.push_str(&localized_block(
        "Family name, variable 5",
        c.family_name_variant.as_ref(),
    ));
    body.push_str(&localized_block("Name, romanized", c.name_roma.as_ref()));
    body.push_str(&localized_block(
        "Family name, romanized",
        c.family_name_roma.as_ref(),
    ));
    body.push_str(&localized_block(
        "Family name variable 5, romanized",
        c.family_name_variant_roma.as_ref(),
    ));
    body.push_str(&localized_block("Description", c.description.as_ref()));
    if let Some(label) = c.element.as_ref().and_then(|e| e.label.as_ref()) {
        body.push_str(&localized_block("Element", Some(label)));
    }
    if let Some(name) = c.series.as_ref().and_then(|s| s.name.as_ref()) {
        body.push_str(&localized_block("Series", Some(name)));
    }
    if let Some(battle) = &c.battle {
        body.push_str(&slice_block("Battle entries", battle));
    }

    let _ = write!(
        body,
        "<h2>Stored variables</h2><details><summary>chara_param</summary><pre>{}</pre></details>\
<details><summary>chara_base</summary><pre>{}</pre></details>",
        esc(&serde_json::to_string(&c.param_raw).unwrap_or_default()),
        esc(&serde_json::to_string(&c.base_raw).unwrap_or_default()),
    );
    body.push_str("<h2>Provenance</h2><ul>");
    for source in [&c.param_source, &c.base_source].into_iter().flatten() {
        let _ = write!(body, "<li>{}</li>", prov(source));
    }
    body.push_str("</ul>");
    layout(Some(ctx), ctx.languages, &title, &body)
}

/// `/wiki/<lang>/skills`.
#[must_use]
pub fn skills(ctx: &Ctx<'_>, page: &Page<Skill>, params: &ListParams) -> String {
    let base = format!("/wiki/{}/skills", seg(&ctx.lang.tag));
    let mut body = format!("<h1>Skills</h1>{}", search(&base, params));
    body.push_str(&pager(&base, params, page.total, page.items.len()));
    body.push_str("<table><thead><tr><th>Name</th><th>Code</th><th>Element</th><th>Category</th></tr></thead><tbody>");
    for s in &page.items {
        let name = label(s.name.as_ref(), ctx.lang).unwrap_or(&s.skill_id_str);
        let element = s
            .element
            .as_ref()
            .map_or_else(String::new, |e| coded(e, ctx.lang));
        let category = s
            .category
            .as_ref()
            .map_or_else(String::new, |c| coded(c, ctx.lang));
        let _ = write!(
            body,
            "<tr><td><a href=\"{base}/0x{:08X}\">{}</a></td><td><code>{}</code></td><td>{element}</td><td>{category}</td></tr>",
            s.skill_id.0,
            esc(name),
            esc(&s.skill_id_str)
        );
    }
    body.push_str("</tbody></table>");
    layout(
        Some(ctx),
        ctx.languages,
        &format!("Skills ({})", ctx.lang.tag),
        &body,
    )
}

/// `/wiki/<lang>/skills/<key>`.
#[must_use]
pub fn skill(ctx: &Ctx<'_>, s: &Skill) -> String {
    let title = label(s.name.as_ref(), ctx.lang).unwrap_or(&s.skill_id_str);
    let mut body = format!(
        "<h1>{}</h1><dl><dt>Code</dt><dd><code>{}</code></dd><dt>Id</dt><dd><code>{}</code></dd>",
        esc(title),
        esc(&s.skill_id_str),
        s.skill_id
    );
    if let Some(element) = &s.element {
        let _ = write!(
            body,
            "<dt>Element</dt><dd>{}</dd>",
            coded(element, ctx.lang)
        );
    }
    if let Some(category) = &s.category {
        let _ = write!(
            body,
            "<dt>Category</dt><dd>{}</dd>",
            coded(category, ctx.lang)
        );
    }
    body.push_str("</dl>");
    if let Some(text) = label(s.description.as_ref(), ctx.lang) {
        let _ = write!(
            body,
            "<h2>Description</h2><p>{}</p>",
            esc(text).replace('\n', "<br>")
        );
    }
    body.push_str("<h2>Texts in every language</h2>");
    body.push_str(&localized_block("Name", s.name.as_ref()));
    body.push_str(&localized_block("Description", s.description.as_ref()));
    if let Some(label) = s.element.as_ref().and_then(|e| e.label.as_ref()) {
        body.push_str(&localized_block("Element", Some(label)));
    }
    body.push_str("<h2>Stored fields</h2>");
    body.push_str(&fields_table(&s.fields));
    let _ = write!(
        body,
        "<h2>Provenance</h2><ul><li>{}</li></ul><h2>Invocation rate</h2>{}",
        prov(&s.source),
        empty(&crate::model::UNSOURCED[2])
    );
    layout(Some(ctx), ctx.languages, title, &body)
}

fn emblem_cell(name: &str, emblem: &Emblem) -> String {
    let mut out = format!(
        "<tr><td><code>{}</code></td><td><code>{}</code></td><td>",
        esc(name),
        emblem.emblem_id
    );
    if emblem.files.is_empty() {
        out.push_str("<span class=\"empty\">no VFS file name hashes to this id</span>");
    }
    for file in &emblem.files {
        if let Some(url) = texture_url(file) {
            let _ = write!(
                out,
                "<img src=\"{}\" alt=\"\" loading=\"lazy\"> ",
                esc(&url)
            );
        }
        let _ = write!(out, "<code>{}</code><br>", esc(file));
    }
    out.push_str("</td></tr>");
    out
}

/// `/wiki/<lang>/teams`.
#[must_use]
pub fn teams(ctx: &Ctx<'_>, page: &Page<Team>, params: &ListParams) -> String {
    let base = format!("/wiki/{}/teams", seg(&ctx.lang.tag));
    let mut body = format!("<h1>Teams</h1>{}", search(&base, params));
    body.push_str(&pager(&base, params, page.total, page.items.len()));
    body.push_str("<table><thead><tr><th>Emblem</th><th>Name</th><th>Id</th><th>Affiliated characters</th></tr></thead><tbody>");
    for t in &page.items {
        let emblem = t
            .emblems
            .values()
            .flat_map(|e| e.files.iter())
            .find_map(|f| texture_url(f))
            .map_or_else(String::new, |url| {
                format!("<img src=\"{}\" alt=\"\" loading=\"lazy\">", esc(&url))
            });
        let name = label(t.name.as_ref(), ctx.lang)
            .map_or_else(|| format!("{}", t.belong_team_id), str::to_owned);
        let _ = write!(
            body,
            "<tr><td>{emblem}</td><td><a href=\"{base}/0x{:08X}\">{}</a></td><td><code>{}</code></td><td>{}</td></tr>",
            t.belong_team_id.0,
            esc(&name),
            t.belong_team_id,
            t.affiliated_characters.len()
        );
    }
    body.push_str("</tbody></table>");
    layout(
        Some(ctx),
        ctx.languages,
        &format!("Teams ({})", ctx.lang.tag),
        &body,
    )
}

/// `/wiki/<lang>/teams/<id>`.
#[must_use]
pub fn team(ctx: &Ctx<'_>, t: &Team) -> String {
    let tag = seg(&ctx.lang.tag);
    let title = label(t.name.as_ref(), ctx.lang)
        .map_or_else(|| format!("{}", t.belong_team_id), str::to_owned);
    let mut body = format!(
        "<h1>{}</h1><dl><dt>Id</dt><dd><code>{}</code></dd></dl>",
        esc(&title),
        t.belong_team_id
    );
    body.push_str("<h2>Emblems</h2>");
    if t.emblems.is_empty() {
        body.push_str("<p class=\"empty\">No emblem id in the entry.</p>");
    } else {
        body.push_str("<p class=\"prov\">An emblem id is the CRC32 of the emblem file name; files are found by hashing the names in the VFS listing.</p><table><thead><tr><th>Field</th><th>Id</th><th>Files</th></tr></thead><tbody>");
        for (name, emblem) in &t.emblems {
            body.push_str(&emblem_cell(name, emblem));
        }
        body.push_str("</tbody></table>");
    }
    body.push_str("<h2>Affiliated characters</h2>");
    if t.affiliated_characters.is_empty() {
        body.push_str("<p class=\"empty\">No <code>chara_base</code> entry names this team.</p>");
    } else {
        let _ = write!(
            body,
            "<p class=\"prov\">{} <code>chara_base</code> entries carry this team's id as belongTeamId.</p><ul>",
            t.affiliated_characters.len()
        );
        for member in &t.affiliated_characters {
            let _ = write!(
                body,
                "<li><a href=\"/wiki/{tag}/characters/{}\">{}</a> <span class=\"prov\">entry {}</span></li>",
                seg(&member.internal_code),
                esc(&member.internal_code),
                member.source.index
            );
        }
        body.push_str("</ul>");
    }
    body.push_str(&empty(&t.roster));
    body.push_str(&localized_block("Name", t.name.as_ref()));
    body.push_str("<h2>Stored fields</h2>");
    body.push_str(&fields_table(&t.fields));
    let _ = write!(
        body,
        "<h2>Provenance</h2><ul><li>{}</li></ul>",
        prov(&t.source)
    );
    layout(Some(ctx), ctx.languages, &title, &body)
}

fn role_text(role: ClassRole) -> &'static str {
    match role {
        ClassRole::Records => "records",
        ClassRole::SortIndex => "sort index of the records of",
        ClassRole::Reference => "slice references into",
    }
}

/// `/wiki/<lang>/tables`: every decoded table with its classes.
#[must_use]
pub fn tables(ctx: &Ctx<'_>, tables: &[TableInfo]) -> String {
    let tag = seg(&ctx.lang.tag);
    let mut body = String::from(
        "<h1>Tables</h1><p>Every node class or list of the decoded gamedata tables, as stored. \
A T2B file stores positional variables with no field names; an RDBN file stores named fields.</p>",
    );
    for table in tables {
        let _ = write!(
            body,
            "<h2><code>{}</code></h2><p class=\"prov\"><code>{}</code> ({})</p><table><thead><tr><th>Class</th><th>Role</th><th>Entries</th><th>Rendered richly by</th></tr></thead><tbody>",
            esc(&table.table),
            esc(&table.vfs_path),
            table.format
        );
        for class in &table.classes {
            let name = match class.role {
                ClassRole::Records => format!(
                    "<a href=\"/wiki/{tag}/tables/{}/{}\"><code>{}</code></a>",
                    seg(&table.table),
                    seg(&class.name),
                    esc(&class.name)
                ),
                _ => format!("<code>{}</code>", esc(&class.name)),
            };
            let role = match &class.target {
                Some(target) => format!("{} <code>{}</code>", role_text(class.role), esc(target)),
                None => role_text(class.role).to_owned(),
            };
            let _ = write!(
                body,
                "<tr><td>{name}</td><td>{role}</td><td>{}</td><td>{}</td></tr>",
                class.entries,
                esc(class.rich_family.unwrap_or(""))
            );
        }
        body.push_str("</tbody></table>");
    }
    layout(
        Some(ctx),
        ctx.languages,
        &format!("Tables ({})", ctx.lang.tag),
        &body,
    )
}

fn labels_cell(record: &TableRecord, lang: &Language) -> String {
    record
        .labels
        .iter()
        .filter_map(|(name, l)| Some(format!("{}: {}", esc(name), esc(label(Some(l), lang)?))))
        .collect::<Vec<_>>()
        .join("; ")
}

fn stored(record: &TableRecord) -> String {
    match (&record.fields, &record.variables) {
        (Some(fields), _) => esc(&serde_json::to_string(fields).unwrap_or_default()),
        (None, Some(variables)) => esc(&serde_json::to_string(variables).unwrap_or_default()),
        (None, None) => String::new(),
    }
}

/// `/wiki/<lang>/tables/<table>/<class>`.
#[must_use]
pub fn table_list(
    ctx: &Ctx<'_>,
    table: &str,
    class: &str,
    page: &Page<TableRecord>,
    params: &ListParams,
) -> String {
    let base = format!(
        "/wiki/{}/tables/{}/{}",
        seg(&ctx.lang.tag),
        seg(table),
        seg(class)
    );
    let mut body = format!(
        "<h1><code>{}</code> / <code>{}</code></h1>{}",
        esc(table),
        esc(class),
        search(&base, params)
    );
    body.push_str(&pager(&base, params, page.total, page.items.len()));
    body.push_str(
        "<table><thead><tr><th>Entry</th><th>Stored</th><th>Labels</th></tr></thead><tbody>",
    );
    for record in &page.items {
        let _ = write!(
            body,
            "<tr><td><a href=\"{base}/{0}\">{0}</a></td><td><code>{1}</code></td><td>{2}</td></tr>",
            record.index,
            stored(record),
            labels_cell(record, ctx.lang)
        );
    }
    body.push_str("</tbody></table>");
    layout(
        Some(ctx),
        ctx.languages,
        &format!("{table} {class} ({})", ctx.lang.tag),
        &body,
    )
}

/// `/wiki/<lang>/tables/<table>/<class>/<index>`; `r` carries every language's labels.
#[must_use]
pub fn table_record(ctx: &Ctx<'_>, r: &TableRecord) -> String {
    let title = format!("{} {} #{}", r.table, r.class, r.index);
    let mut body = format!("<h1>{}</h1>", esc(&title));
    match (&r.fields, &r.variables) {
        (Some(fields), _) => {
            body.push_str("<h2>Stored fields</h2>");
            body.push_str(&fields_table(fields));
        }
        (None, Some(variables)) => {
            let _ = write!(
                body,
                "<h2>Stored variables</h2><p class=\"prov\">The file stores no field names; positions are as stored.</p><pre>{}</pre>",
                esc(&serde_json::to_string(variables).unwrap_or_default())
            );
        }
        (None, None) => {}
    }
    if let Some(sort) = r.sort_index {
        let _ = write!(body, "<p>Sort index <code>{sort}</code></p>");
    }
    for (target, slice) in &r.refs {
        body.push_str(&slice_block(&format!("Entries of {target}"), slice));
    }
    if !r.children.is_empty() {
        let _ = write!(
            body,
            "<h2>Nested nodes</h2><pre>{}</pre>",
            esc(&serde_json::to_string(&r.children).unwrap_or_default())
        );
    }
    for (name, labelled) in &r.labels {
        body.push_str(&localized_block(name, Some(labelled)));
    }
    let _ = write!(
        body,
        "<h2>Provenance</h2><ul><li>{}</li></ul>",
        prov(&r.source)
    );
    layout(Some(ctx), ctx.languages, &title, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_escaped() {
        assert_eq!(
            esc("<a href=\"x\">&'"),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
        );
    }

    #[test]
    fn emblem_textures_follow_the_asset_origin_routes() {
        assert_eq!(
            texture_url("data/dx11/menu/200_icon/01_icon_emblem/em010001.g4tx").as_deref(),
            Some("/tex/dx11/menu/200_icon/01_icon_emblem/em010001.png")
        );
        assert_eq!(texture_url("data/common/x.cfg.bin"), None);
    }

    #[test]
    fn url_segments_are_percent_encoded() {
        assert_eq!(seg("zh-Hans"), "zh-Hans");
        assert_eq!(seg("a b/c"), "a%20b%2Fc");
    }
}
