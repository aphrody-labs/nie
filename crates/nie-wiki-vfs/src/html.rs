//! Server-rendered pages: semantic HTML with no script. Every page names the VFS path, table and
//! index behind its values and shows an explicit empty state for what the VFS does not give.

use std::fmt::Write as _;

use serde_json::Value;

use crate::model::{
    Character, Coded, Emblem, Language, Localized, NoSource, Provenance, Skill, Team,
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
        for family in ["characters", "skills", "teams"] {
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
    label(c.name.as_ref(), lang).map_or_else(|| c.internal_code.clone(), str::to_owned)
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

/// `/wiki`: the languages, the families, their backing tables and the families without a source.
#[must_use]
pub fn index(languages: &[Language], sources: &[Source], unsourced: &[NoSource]) -> String {
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
<a href=\"/wiki/{tag}/teams\">teams</a></li>",
            esc(&language.tag),
            esc(&language.folder)
        );
    }
    body.push_str("</ul><h2>Backing tables</h2><table><thead><tr><th>Family</th><th>VFS path</th><th>Table</th><th>Entries</th></tr></thead><tbody>");
    for source in sources {
        let _ = write!(
            body,
            "<tr><td>{}</td><td><code>{}</code></td><td><code>{}</code></td><td>{}</td></tr>",
            esc(source.family),
            esc(&source.vfs_path),
            esc(source.table),
            source.entries
        );
    }
    body.push_str("</tbody></table><h2>Families without a source</h2>");
    for state in unsourced {
        body.push_str(&empty(state));
    }
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
pub fn characters(ctx: &Ctx<'_>, page: &Page<Character>, params: &ListParams) -> String {
    let base = format!("/wiki/{}/characters", seg(&ctx.lang.tag));
    let mut body = format!("<h1>Characters</h1>{}", search(&base, params));
    body.push_str(&pager(&base, params, page.total, page.items.len()));
    body.push_str("<table><thead><tr><th>Name</th><th>Code</th><th>Element</th><th>Main position</th><th>Sub position</th><th>Team</th></tr></thead><tbody>");
    for c in &page.items {
        let team = c.team.as_ref().map_or_else(String::new, |team| {
            let text = label(team.name.as_ref(), ctx.lang)
                .map_or_else(|| format!("{}", team.belong_team_id), str::to_owned);
            format!(
                "<a href=\"/wiki/{}/teams/0x{:08X}\">{}</a>",
                seg(&ctx.lang.tag),
                team.belong_team_id.0,
                esc(&text)
            )
        });
        let _ = write!(
            body,
            "<tr><td><a href=\"{base}/0x{:08X}\">{}</a></td><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{team}</td></tr>",
            c.chara_param_id.0,
            esc(&name_or_code(c, ctx.lang)),
            esc(&c.internal_code),
            coded(&c.element, ctx.lang),
            coded(&c.main_position, ctx.lang),
            coded(&c.sub_position, ctx.lang),
        );
    }
    body.push_str("</tbody></table>");
    body.push_str("<h2>Sources</h2><p class=\"prov\">Rows are <code>chara_param</code> entries joined to <code>chara_base</code>; each page names its entries.</p>");
    layout(
        Some(ctx),
        ctx.languages,
        &format!("Characters ({})", ctx.lang.tag),
        &body,
    )
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
    row(
        "Internal code",
        format!("<code>{}</code>", esc(&c.internal_code)),
    );
    row(
        "chara_param id",
        format!("<code>{}</code>", c.chara_param_id),
    );
    row("chara_base id", format!("<code>{}</code>", c.chara_base_id));
    match &c.gender {
        Some(gender) => row("Gender", coded(gender, ctx.lang)),
        None => row(
            "Gender",
            "<span class=\"empty\">absent from the entry</span>".into(),
        ),
    }
    row("Element", coded(&c.element, ctx.lang));
    row("Main position", coded(&c.main_position, ctx.lang));
    row("Sub position", coded(&c.sub_position, ctx.lang));
    row(
        "Growth pattern",
        format!("<code>{}</code>", c.growth_pattern),
    );
    if let Some(series) = c.series_id {
        row("Series id", format!("<code>{series}</code>"));
    }
    if let Some(team) = &c.team {
        let text = label(team.name.as_ref(), ctx.lang)
            .map_or_else(|| format!("{}", team.belong_team_id), str::to_owned);
        row(
            "Team",
            format!(
                "<a href=\"/wiki/{tag}/teams/0x{:08X}\">{}</a>",
                team.belong_team_id.0,
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
        body.push_str(
            "<table><thead><tr><th>Skill</th><th>Level</th><th>Id</th></tr></thead><tbody>",
        );
        for skill in &c.skills {
            let text = label(skill.name.as_ref(), ctx.lang)
                .map_or_else(|| format!("{}", skill.skill_id), str::to_owned);
            let _ = write!(
                body,
                "<tr><td><a href=\"/wiki/{tag}/skills/0x{:08X}\">{}</a></td><td>{}</td><td><code>{}</code></td></tr>",
                skill.skill_id.0,
                esc(&text),
                skill.learn_level,
                skill.skill_id
            );
        }
        body.push_str("</tbody></table>");
    }

    body.push_str("<h2>Texts in every language</h2>");
    body.push_str(&localized_block("Name", c.name.as_ref()));
    body.push_str(&localized_block("Family name", c.family_name.as_ref()));
    body.push_str(&localized_block("Description", c.description.as_ref()));
    if let Some(label) = &c.element.label {
        body.push_str(&localized_block("Element", Some(label)));
    }

    let _ = write!(
        body,
        "<h2>Stored variables</h2><details><summary>chara_param</summary><pre>{}</pre></details>\
<details><summary>chara_base</summary><pre>{}</pre></details>",
        esc(&serde_json::to_string(&c.param_raw).unwrap_or_default()),
        esc(&serde_json::to_string(&c.base_raw).unwrap_or_default()),
    );
    let _ = write!(
        body,
        "<h2>Provenance</h2><ul><li>{}</li><li>{}</li></ul>",
        prov(&c.param_source),
        prov(&c.base_source)
    );
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
