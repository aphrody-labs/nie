//! cfg.bin tables (RDBN and T2B) and the nine-language text tables.

use nie_formats::cfgbin::{self, CfgEntry, RdbnValue, Value};
use nie_formats::locale::LOCALE_TAGS;

use super::{Body, Built, Context, PreviewRequest, Table};

/// Rows kept per table.
const MAX_ROWS: usize = 500;
/// Longest cell, in characters.
const MAX_CELL: usize = 64;

fn cell(text: impl Into<String>) -> String {
    let text: String = text.into();
    if text.chars().count() > MAX_CELL {
        let cut: String = text.chars().take(MAX_CELL - 1).collect();
        format!("{cut}~")
    } else {
        text
    }
}

fn value(v: &Value) -> String {
    match v {
        Value::String(s) => cell(s.replace('\n', "\\n")),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => format!("{f}"),
    }
}

fn rdbn_value(v: &RdbnValue) -> String {
    match v {
        RdbnValue::Bool(b) => b.to_string(),
        RdbnValue::Byte(b) => b.to_string(),
        RdbnValue::Short(i) | RdbnValue::ActType(i) => i.to_string(),
        RdbnValue::Int(i) | RdbnValue::Flag(i) => i.to_string(),
        RdbnValue::Float(f) => format!("{f}"),
        RdbnValue::Hash(h) => format!("{h:08x}"),
        RdbnValue::Rates(r) | RdbnValue::Position(r) => format!("{} {} {} {}", r[0], r[1], r[2], r[3]),
        RdbnValue::Condition(s) => cell(s.clone()),
        RdbnValue::ShortTuple(t) => format!("{},{}", t[0], t[1]),
        RdbnValue::Blob(b) => format!("{} bytes", b.len()),
        RdbnValue::Invalid => "<invalid>".to_owned(),
    }
}

/// `CHARA_BASE_INFO_12` and `CHARA_BASE_INFO_13` belong to the same record type.
fn group_name(name: &str) -> &str {
    match name.rsplit_once('_') {
        Some((head, tail)) if !head.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) => head,
        _ => name,
    }
}

fn t2b_table(entries: &[CfgEntry], index: i32) -> (Table, Vec<(String, String)>) {
    let mut groups: Vec<(&str, Vec<&CfgEntry>)> = Vec::new();
    for entry in entries {
        let key = group_name(&entry.name);
        match groups.last_mut() {
            Some((name, rows)) if *name == key => rows.push(entry),
            _ => groups.push((key, vec![entry])),
        }
    }
    if groups.is_empty() {
        return (Table::default(), vec![("records".into(), "0".into())]);
    }
    let chosen = index.rem_euclid(i32::try_from(groups.len()).unwrap_or(1)) as usize;
    let (name, rows) = &groups[chosen];
    let width = rows.iter().map(|r| r.variables.len()).max().unwrap_or(0);
    let has_children = rows.iter().any(|r| !r.children.is_empty());
    let mut columns: Vec<String> = (0..width).map(|i| format!("#{i}")).collect();
    if has_children {
        columns.push("children".to_owned());
    }
    let table = Table {
        title: (*name).to_owned(),
        columns,
        rows: rows
            .iter()
            .take(MAX_ROWS)
            .map(|r| {
                let mut row: Vec<String> = r.variables.iter().map(value).collect();
                row.resize(width, String::new());
                if has_children {
                    row.push(r.children.len().to_string());
                }
                row
            })
            .collect(),
        total_rows: rows.len(),
    };
    let meta = vec![
        ("format".to_owned(), "T2B".to_owned()),
        ("record".to_owned(), format!("{name} ({} of {})", chosen + 1, groups.len())),
        ("rows".to_owned(), rows.len().to_string()),
    ];
    (table, meta)
}

fn rdbn_table(data: &[u8], index: i32) -> Result<(Table, Vec<(String, String)>), String> {
    let rdbn = cfgbin::parse(data).map_err(|e| e.to_string())?;
    let lists = cfgbin::read_values(&rdbn, data);
    if lists.is_empty() {
        return Ok((Table::default(), vec![("lists".into(), "0".into())]));
    }
    let chosen = index.rem_euclid(i32::try_from(lists.len()).unwrap_or(1)) as usize;
    let list = &lists[chosen];
    let columns: Vec<String> = list
        .rows
        .first()
        .map(|r| r.fields.iter().map(|(n, _)| n.clone()).collect())
        .unwrap_or_default();
    let table = Table {
        title: list.name.clone(),
        columns,
        rows: list
            .rows
            .iter()
            .take(MAX_ROWS)
            .map(|r| r.fields.iter().map(|(_, v)| rdbn_value(v)).collect())
            .collect(),
        total_rows: list.rows.len(),
    };
    let meta = vec![
        ("format".to_owned(), "RDBN".to_owned()),
        (
            "list".to_owned(),
            format!("{} : {} ({} of {})", list.name, list.type_name, chosen + 1, lists.len()),
        ),
        ("rows".to_owned(), list.rows.len().to_string()),
    ];
    Ok((table, meta))
}

/// The same text table in the other languages of the game, when the path is localized:
/// `data/common/text/<locale>/<rest>`.
fn language_variants(ctx: &Context<'_>, path: &str) -> Option<(Vec<(&'static str, String)>, usize)> {
    let rest = path.strip_prefix("data/common/text/")?;
    let (tag, tail) = rest.split_once('/')?;
    let here = LOCALE_TAGS.iter().position(|t| *t == tag)?;
    let variants: Vec<(&'static str, String)> = LOCALE_TAGS
        .iter()
        .map(|t| (*t, format!("data/common/text/{t}/{tail}")))
        .filter(|(_, p)| ctx.vfs.find_any(p).is_some())
        .collect();
    let at = variants.iter().position(|(t, _)| *t == LOCALE_TAGS[here])?;
    (variants.len() > 1).then_some((variants, at))
}

pub(super) fn table(ctx: &Context<'_>, request: &PreviewRequest, data: &[u8]) -> Built {
    let mut extra = Vec::new();
    let mut bytes = std::borrow::Cow::Borrowed(data);
    let mut group = request.index;
    // A localized text file cycles through the languages; every other table through its records.
    if let Some((variants, at)) = language_variants(ctx, &request.path) {
        let step = request.index.rem_euclid(i32::try_from(variants.len()).unwrap_or(1)) as usize;
        let (tag, path) = &variants[(at + step) % variants.len()];
        extra.push((
            "language".to_owned(),
            format!("{tag} ({} of {})", (at + step) % variants.len() + 1, variants.len()),
        ));
        if step != 0 && let Ok(other) = ctx.vfs.read(path) {
            bytes = std::borrow::Cow::Owned(other);
        }
        group = 0;
    }
    let result = if cfgbin::is_rdbn(&bytes) {
        rdbn_table(&bytes, group)
    } else {
        cfgbin::cfgbin_parse(&bytes)
            .map(|file| t2b_table(&file.entries, group))
            .map_err(|e| e.to_string())
    };
    match result {
        Ok((table, mut meta)) => {
            extra.append(&mut meta);
            (Body::Table(table), extra, false)
        }
        Err(error) => (Body::Message(format!("cfg.bin: {error}")), extra, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, vars: Vec<Value>) -> CfgEntry {
        CfgEntry {
            name: name.into(),
            variables: vars,
            children: Vec::new(),
        }
    }

    #[test]
    fn t2b_records_group_by_name_and_page_with_the_index() {
        let entries = vec![
            entry("A_INFO_0", vec![Value::Int(1), Value::String("x".into())]),
            entry("A_INFO_1", vec![Value::Int(2), Value::Float(1.5)]),
            entry("B_NUM_0", vec![Value::Int(9)]),
        ];
        let (first, meta) = t2b_table(&entries, 0);
        assert_eq!(first.title, "A_INFO");
        assert_eq!(first.total_rows, 2);
        assert_eq!(first.rows[1], ["2", "1.5"]);
        assert!(meta.iter().any(|(k, v)| k == "record" && v.contains("1 of 2")));
        let (second, _) = t2b_table(&entries, 1);
        assert_eq!(second.title, "B_NUM");
        let (wrapped, _) = t2b_table(&entries, 2);
        assert_eq!(wrapped.title, "A_INFO");
    }

    #[test]
    fn long_cells_are_cut() {
        assert_eq!(cell("a".repeat(100)).chars().count(), MAX_CELL);
    }
}
