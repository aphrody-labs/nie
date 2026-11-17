//! Exact `cfg.bin` to SQL mapping.
//!
//! A decoded RDBN list becomes one table, one RDBN row becomes one SQL row, one RDBN field
//! becomes one column (multi-component fields are flattened into numbered columns). Nothing is
//! invented: a value the reader flagged `Invalid` is SQL `NULL`, a column keeps the SQL type of
//! the RDBN field type it was read from. T2B files (hierarchical) map to two generic tables.
//!
//! The same [`Schema`] renders to [`Dialect::Postgres`] and [`Dialect::Sqlite`], so the production
//! database and the embedded one are generated from one source.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use nie_formats::cfgbin::{self, CfgBinFile, CfgEntry, RdbnList, RdbnValue, Value};

/// Errors raised while turning a `cfg.bin` into SQL.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The buffer is neither RDBN nor T2B.
    #[error("not a cfg.bin (neither RDBN nor T2B)")]
    UnknownFormat,
    /// The underlying parser rejected the file.
    #[error("cfg.bin parse failed: {0}")]
    Parse(String),
}

/// Target SQL dialect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// PostgreSQL (production).
    Postgres,
    /// SQLite (local and embedded).
    Sqlite,
}

/// Logical column type, rendered per dialect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnType {
    /// RDBN `Bool`.
    Bool,
    /// 16-bit signed (`Byte` is stored here: 0..=255 does not fit `i8`).
    SmallInt,
    /// 32-bit signed.
    Int,
    /// Unsigned 32-bit (`Hash`): `BIGINT` so the value stays exact in Postgres.
    BigInt,
    /// 32-bit float.
    Real,
    /// UTF-8 text.
    Text,
    /// Raw bytes.
    Blob,
}

impl ColumnType {
    /// SQL type name for `dialect`.
    #[must_use]
    pub const fn sql(self, dialect: Dialect) -> &'static str {
        match (self, dialect) {
            (Self::Bool, Dialect::Postgres) => "BOOLEAN",
            (Self::Bool, Dialect::Sqlite) => "INTEGER",
            (Self::SmallInt, Dialect::Postgres) => "SMALLINT",
            (Self::Int, Dialect::Postgres) => "INTEGER",
            (Self::BigInt, Dialect::Postgres) => "BIGINT",
            (Self::SmallInt | Self::Int | Self::BigInt, Dialect::Sqlite) => "INTEGER",
            (Self::Real, Dialect::Postgres) => "REAL",
            (Self::Real, Dialect::Sqlite) => "REAL",
            (Self::Text, _) => "TEXT",
            (Self::Blob, Dialect::Postgres) => "BYTEA",
            (Self::Blob, Dialect::Sqlite) => "BLOB",
        }
    }
}

/// One column of a generated table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    /// SQL identifier (sanitised, unique in the table).
    pub name: String,
    /// Original RDBN field name (and component index for flattened fields).
    pub source: String,
    /// Logical type.
    pub ty: ColumnType,
}

/// One generated table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// SQL identifier (sanitised, unique in the schema).
    pub name: String,
    /// Original RDBN list name.
    pub list_name: String,
    /// Original RDBN type name.
    pub type_name: String,
    /// Columns after the implicit `row_index` primary key.
    pub columns: Vec<Column>,
}

/// A schema plus its rows, ready to render.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Dataset {
    /// Tables in file order.
    pub tables: Vec<Table>,
    /// Rows per table (same order as `tables`); each row has one cell per column.
    pub rows: Vec<Vec<Vec<Cell>>>,
}

/// A SQL cell value.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    /// `NULL`.
    Null,
    /// Boolean.
    Bool(bool),
    /// Integer.
    Int(i64),
    /// Float.
    Real(f32),
    /// Text.
    Text(String),
    /// Bytes.
    Blob(Vec<u8>),
}

/// Lower-case snake identifier made of `[a-z0-9_]`, never empty, never starting with a digit.
#[must_use]
pub fn sanitize_ident(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut prev_underscore = false;
    let mut prev_lower = false;
    for ch in raw.chars() {
        if ch.is_ascii_uppercase() && prev_lower && !prev_underscore {
            out.push('_');
        }
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_underscore = false;
            prev_lower = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else if !prev_underscore && !out.is_empty() {
            out.push('_');
            prev_underscore = true;
            prev_lower = false;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        out.push('x');
    }
    // PostgreSQL truncates identifiers at 63 bytes; stay below it so suffixes still fit.
    out.truncate(56);
    while out.ends_with('_') {
        out.pop();
    }
    if out.as_bytes()[0].is_ascii_digit() {
        out.insert(0, 'x');
    }
    out
}

/// Makes `base` unique in `taken` by appending `_2`, `_3`, ...
fn unique(base: String, taken: &mut BTreeSet<String>) -> String {
    if taken.insert(base.clone()) {
        return base;
    }
    for n in 2u32.. {
        let candidate = format!("{base}_{n}");
        if taken.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!("u32 suffix space exhausted")
}

/// Components of one value: (suffix, type, cell).
fn components(value: &RdbnValue) -> Vec<(Option<usize>, Option<ColumnType>, Cell)> {
    use ColumnType as T;
    match value {
        RdbnValue::Bool(v) => vec![(None, Some(T::Bool), Cell::Bool(*v))],
        RdbnValue::Byte(v) => vec![(None, Some(T::SmallInt), Cell::Int(i64::from(*v)))],
        RdbnValue::Short(v) | RdbnValue::ActType(v) => {
            vec![(None, Some(T::SmallInt), Cell::Int(i64::from(*v)))]
        }
        RdbnValue::Int(v) | RdbnValue::Flag(v) => {
            vec![(None, Some(T::Int), Cell::Int(i64::from(*v)))]
        }
        RdbnValue::Float(v) => vec![(None, Some(T::Real), Cell::Real(*v))],
        RdbnValue::Hash(v) => vec![(None, Some(T::BigInt), Cell::Int(i64::from(*v)))],
        RdbnValue::Rates(v) | RdbnValue::Position(v) => v
            .iter()
            .enumerate()
            .map(|(i, f)| (Some(i), Some(T::Real), Cell::Real(*f)))
            .collect(),
        RdbnValue::Condition(s) => vec![(None, Some(T::Text), Cell::Text(s.clone()))],
        RdbnValue::ShortTuple(v) => v
            .iter()
            .enumerate()
            .map(|(i, n)| (Some(i), Some(T::SmallInt), Cell::Int(i64::from(*n))))
            .collect(),
        RdbnValue::Blob(b) => vec![(None, Some(T::Blob), Cell::Blob(b.clone()))],
        RdbnValue::Invalid => vec![(None, None, Cell::Null)],
    }
}

/// Text form of a value, used when a column mixes value kinds.
fn value_text(value: &RdbnValue) -> String {
    match value {
        RdbnValue::Bool(v) => v.to_string(),
        RdbnValue::Byte(v) => v.to_string(),
        RdbnValue::Short(v) | RdbnValue::ActType(v) => v.to_string(),
        RdbnValue::Int(v) | RdbnValue::Flag(v) => v.to_string(),
        RdbnValue::Float(v) => format!("{v:?}"),
        RdbnValue::Hash(v) => v.to_string(),
        RdbnValue::Rates(v) | RdbnValue::Position(v) => {
            format!("{:?},{:?},{:?},{:?}", v[0], v[1], v[2], v[3])
        }
        RdbnValue::Condition(s) => s.clone(),
        RdbnValue::ShortTuple(v) => format!("{},{}", v[0], v[1]),
        RdbnValue::Blob(b) => b.iter().map(|x| format!("{x:02x}")).collect(),
        RdbnValue::Invalid => String::new(),
    }
}

/// Number of SQL columns a value occupies, `None` for `Invalid` (width unknown).
fn width(value: &RdbnValue) -> Option<usize> {
    match value {
        RdbnValue::Invalid => None,
        other => Some(components(other).len()),
    }
}

/// Builds the dataset for decoded RDBN lists.
///
/// The column set of a table is taken from the first row whose field is not `Invalid` (types are
/// per list, so every row shares it); `Invalid` cells become `NULL`.
#[must_use]
pub fn from_rdbn_lists(lists: &[RdbnList]) -> Dataset {
    let mut data = Dataset::default();
    let mut table_names = BTreeSet::new();
    for list in lists {
        let table_name = unique(sanitize_ident(&list.name), &mut table_names);
        let mut col_names = BTreeSet::from(["row_index".to_owned()]);
        // field position -> (field name, component types)
        let nfields = list.rows.iter().map(|r| r.fields.len()).max().unwrap_or(0);
        let mut layout: Vec<(String, Vec<(Option<usize>, ColumnType)>)> = Vec::new();
        let mut mixed: Vec<bool> = Vec::new();
        for pos in 0..nfields {
            let mut chosen: Option<(String, Vec<(Option<usize>, ColumnType)>)> = None;
            let mut name = String::new();
            for row in &list.rows {
                let Some((n, v)) = row.fields.get(pos) else {
                    continue;
                };
                name = n.clone();
                if width(v).is_some() {
                    let comps = components(v)
                        .into_iter()
                        .filter_map(|(s, t, _)| t.map(|t| (s, t)))
                        .collect();
                    chosen = Some((n.clone(), comps));
                    break;
                }
            }
            // A field whose value kind changes between rows (the reader can yield a different
            // variant for the same field) becomes TEXT so no row is rejected or altered.
            let is_mixed = chosen.as_ref().is_some_and(|(_, comps)| {
                list.rows
                    .iter()
                    .filter_map(|r| r.fields.get(pos))
                    .filter(|(_, v)| width(v).is_some())
                    .any(|(_, v)| {
                        let sig: Vec<(Option<usize>, ColumnType)> = components(v)
                            .into_iter()
                            .filter_map(|(s, t, _)| t.map(|t| (s, t)))
                            .collect();
                        &sig != comps
                    })
            });
            mixed.push(is_mixed);
            if is_mixed {
                chosen = Some((name.clone(), vec![(None, ColumnType::Text)]));
            }
            // Field invalid in every row: keep one nullable TEXT column so the field is not lost.
            layout.push(chosen.unwrap_or((name, vec![(None, ColumnType::Text)])));
        }
        let mut columns = Vec::new();
        for (fname, comps) in &layout {
            for (suffix, ty) in comps {
                let base = match suffix {
                    Some(i) => format!("{}_{i}", sanitize_ident(fname)),
                    None => sanitize_ident(fname),
                };
                let source = match suffix {
                    Some(i) => format!("{fname}[{i}]"),
                    None => fname.clone(),
                };
                columns.push(Column {
                    name: unique(base, &mut col_names),
                    source,
                    ty: *ty,
                });
            }
        }
        let mut rows = Vec::with_capacity(list.rows.len());
        for row in &list.rows {
            let mut cells = Vec::with_capacity(columns.len());
            for (pos, (_, comps)) in layout.iter().enumerate() {
                match row.fields.get(pos) {
                    Some((_, v)) if mixed[pos] && width(v).is_some() => {
                        cells.push(Cell::Text(value_text(v)))
                    }
                    Some((_, v)) if width(v) == Some(comps.len()) => {
                        cells.extend(components(v).into_iter().map(|(_, _, c)| c));
                    }
                    _ => cells.extend(comps.iter().map(|_| Cell::Null)),
                }
            }
            rows.push(cells);
        }
        data.tables.push(Table {
            name: table_name,
            list_name: list.name.clone(),
            type_name: list.type_name.clone(),
            columns,
        });
        data.rows.push(rows);
    }
    data
}

/// Builds the generic T2B dataset: `t2b_entry(row_index, parent, ord, name)` and
/// `t2b_var(row_index, entry, ord, kind, int_value, real_value, text_value)`.
#[must_use]
pub fn from_t2b(file: &CfgBinFile) -> Dataset {
    fn col(name: &str, ty: ColumnType) -> Column {
        Column {
            name: name.to_owned(),
            source: name.to_owned(),
            ty,
        }
    }
    let mut entries: Vec<Vec<Cell>> = Vec::new();
    let mut vars: Vec<Vec<Cell>> = Vec::new();
    fn walk(
        e: &CfgEntry,
        parent: Option<i64>,
        ord: i64,
        entries: &mut Vec<Vec<Cell>>,
        vars: &mut Vec<Vec<Cell>>,
    ) {
        let id = i64::try_from(entries.len()).unwrap_or(i64::MAX);
        entries.push(vec![
            parent.map_or(Cell::Null, Cell::Int),
            Cell::Int(ord),
            Cell::Text(e.name.clone()),
        ]);
        for (i, v) in e.variables.iter().enumerate() {
            let (kind, iv, rv, tv) = match v {
                Value::Int(n) => ("int", Cell::Int(i64::from(*n)), Cell::Null, Cell::Null),
                Value::Float(f) => ("float", Cell::Null, Cell::Real(*f), Cell::Null),
                Value::String(s) => ("string", Cell::Null, Cell::Null, Cell::Text(s.clone())),
            };
            vars.push(vec![
                Cell::Int(id),
                Cell::Int(i64::try_from(i).unwrap_or(i64::MAX)),
                Cell::Text(kind.to_owned()),
                iv,
                rv,
                tv,
            ]);
        }
        for (i, c) in e.children.iter().enumerate() {
            walk(
                c,
                Some(id),
                i64::try_from(i).unwrap_or(i64::MAX),
                entries,
                vars,
            );
        }
    }
    for (i, e) in file.entries.iter().enumerate() {
        walk(
            e,
            None,
            i64::try_from(i).unwrap_or(i64::MAX),
            &mut entries,
            &mut vars,
        );
    }
    Dataset {
        tables: vec![
            Table {
                name: "t2b_entry".into(),
                list_name: "t2b".into(),
                type_name: "CfgEntry".into(),
                columns: vec![
                    col("parent", ColumnType::Int),
                    col("ord", ColumnType::Int),
                    col("name", ColumnType::Text),
                ],
            },
            Table {
                name: "t2b_var".into(),
                list_name: "t2b".into(),
                type_name: "CfgValue".into(),
                columns: vec![
                    col("entry", ColumnType::Int),
                    col("ord", ColumnType::Int),
                    col("kind", ColumnType::Text),
                    col("int_value", ColumnType::Int),
                    col("real_value", ColumnType::Real),
                    col("text_value", ColumnType::Text),
                ],
            },
        ],
        rows: vec![entries, vars],
    }
}

/// Parses raw `cfg.bin` bytes (RDBN or T2B) into a [`Dataset`].
///
/// # Errors
/// [`Error::UnknownFormat`] when the magic matches neither format, [`Error::Parse`] when the
/// parser rejects the file.
pub fn from_bytes(bytes: &[u8]) -> Result<Dataset, Error> {
    if cfgbin::is_rdbn(bytes) {
        let rdbn = cfgbin::parse(bytes).map_err(|e| Error::Parse(e.to_string()))?;
        Ok(from_rdbn_lists(&cfgbin::read_values(&rdbn, bytes)))
    } else if is_empty_t2b(bytes) {
        Ok(from_t2b(&CfgBinFile {
            format: cfgbin::Format::T2b,
            entries: Vec::new(),
        }))
    } else if cfgbin::is_t2b(bytes) || has_empty_string_table(bytes) {
        let file = cfgbin::cfgbin_parse(bytes).map_err(|e| Error::Parse(e.to_string()))?;
        Ok(from_t2b(&file))
    } else {
        Err(Error::UnknownFormat)
    }
}

/// T2B files whose names all live in the key table have a string table of length 0
/// (`chara_param_*.cfg.bin`, `seasonal_chara_param_*.cfg.bin`); `cfgbin::is_t2b` requires a
/// positive length, so recognise that header shape here: count > 0, offset inside the buffer,
/// string table length 0 and string count 0.
#[must_use]
pub fn has_empty_string_table(bytes: &[u8]) -> bool {
    if bytes.len() < 16 {
        return false;
    }
    let word = |i: usize| i32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    let (count, off, len, strings) = (word(0), word(4), word(8), word(12));
    count > 0
        && len == 0
        && strings == 0
        && off >= 16
        && usize::try_from(off).is_ok_and(|o| o <= bytes.len())
}

/// A T2B file with no entry at all: 0 entries, string table at offset 16, length 0 and 0
/// strings (48 bytes in the game: 38 files such as `*_encount_config_*.cfg.bin`, the key table
/// header is empty too). Valid, and it maps to two empty tables.
#[must_use]
pub fn is_empty_t2b(bytes: &[u8]) -> bool {
    if bytes.len() < 16 {
        return false;
    }
    let word = |i: usize| i32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    word(0) == 0 && word(4) == 16 && word(8) == 0 && word(12) == 0
}

pub(crate) fn qualified(schema: Option<&str>, name: &str) -> String {
    schema.map_or_else(
        || quote_ident(name),
        |s| format!("{}.{}", quote_ident(s), quote_ident(name)),
    )
}

pub(crate) fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn quote_text(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Renders one cell as an SQL literal.
#[must_use]
pub fn literal(cell: &Cell, dialect: Dialect) -> String {
    match cell {
        Cell::Null => "NULL".into(),
        Cell::Bool(b) => match dialect {
            Dialect::Postgres => if *b { "TRUE" } else { "FALSE" }.into(),
            Dialect::Sqlite => if *b { "1" } else { "0" }.into(),
        },
        Cell::Int(n) => n.to_string(),
        Cell::Real(f) if f.is_finite() => {
            // `{:?}` is the shortest representation that round-trips the f32 exactly.
            let s = format!("{f:?}");
            match dialect {
                Dialect::Postgres => format!("{s}::real"),
                Dialect::Sqlite => s,
            }
        }
        // NaN and infinities have no portable literal; NULL keeps the row loadable.
        Cell::Real(_) => "NULL".into(),
        Cell::Text(s) => quote_text(s),
        Cell::Blob(b) => match dialect {
            Dialect::Postgres => format!("'\\x{}'", hex(b)),
            Dialect::Sqlite => format!("X'{}'", hex(b)),
        },
    }
}

impl Table {
    /// `CREATE TABLE` statement.
    #[must_use]
    pub fn create_sql(&self, dialect: Dialect) -> String {
        self.create_sql_in(dialect, None)
    }

    /// `CREATE TABLE` statement, optionally qualified with a schema.
    #[must_use]
    pub fn create_sql_in(&self, dialect: Dialect, schema: Option<&str>) -> String {
        let mut s = format!(
            "CREATE TABLE {} (\n  \"row_index\" INTEGER PRIMARY KEY",
            qualified(schema, &self.name)
        );
        for c in &self.columns {
            let _ = write!(s, ",\n  {} {}", quote_ident(&c.name), c.ty.sql(dialect));
        }
        s.push_str("\n);\n");
        s
    }
}

impl Dataset {
    /// Full script: DDL then one `INSERT` per row, wrapped in a transaction.
    #[must_use]
    pub fn to_sql(&self, dialect: Dialect) -> String {
        let mut out = String::from("BEGIN;\n");
        for t in &self.tables {
            out.push_str(&t.create_sql(dialect));
        }
        for (t, rows) in self.tables.iter().zip(&self.rows) {
            let cols = std::iter::once("\"row_index\"".to_owned())
                .chain(t.columns.iter().map(|c| quote_ident(&c.name)))
                .collect::<Vec<_>>()
                .join(", ");
            for (i, row) in rows.iter().enumerate() {
                let vals = std::iter::once(i.to_string())
                    .chain(row.iter().map(|c| literal(c, dialect)))
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(
                    out,
                    "INSERT INTO {} ({cols}) VALUES ({vals});",
                    quote_ident(&t.name)
                );
            }
        }
        out.push_str("COMMIT;\n");
        out
    }

    /// Table name to source list name, for the provenance table.
    #[must_use]
    pub fn provenance(&self) -> BTreeMap<&str, &str> {
        self.tables
            .iter()
            .map(|t| (t.name.as_str(), t.list_name.as_str()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nie_formats::cfgbin::RdbnRow;

    fn list() -> RdbnList {
        RdbnList {
            name: "CharaParam".into(),
            type_name: "CharaParamType".into(),
            rows: vec![
                RdbnRow {
                    fields: vec![
                        ("CharaId".into(), RdbnValue::Hash(0xFFFF_FFFF)),
                        ("Name".into(), RdbnValue::Condition("O'Neil".into())),
                        ("Pos".into(), RdbnValue::Position([1.0, 2.5, -3.0, 0.0])),
                        ("Tup".into(), RdbnValue::ShortTuple([-1, 7])),
                        ("Ok".into(), RdbnValue::Bool(true)),
                        ("Raw".into(), RdbnValue::Blob(vec![0xde, 0xad])),
                        ("Pos".into(), RdbnValue::Invalid),
                    ],
                },
                RdbnRow {
                    fields: vec![
                        ("CharaId".into(), RdbnValue::Hash(1)),
                        ("Name".into(), RdbnValue::Invalid),
                        ("Pos".into(), RdbnValue::Position([0.0; 4])),
                        ("Tup".into(), RdbnValue::ShortTuple([0, 0])),
                        ("Ok".into(), RdbnValue::Bool(false)),
                        ("Raw".into(), RdbnValue::Blob(vec![])),
                        ("Pos".into(), RdbnValue::Invalid),
                    ],
                },
            ],
        }
    }

    #[test]
    fn identifiers() {
        assert_eq!(sanitize_ident("CharaParam"), "chara_param");
        assert_eq!(sanitize_ident("9lives"), "x9lives");
        assert_eq!(sanitize_ident("--"), "x");
        assert_eq!(sanitize_ident("a b-c"), "a_b_c");
    }

    #[test]
    fn schema_is_exact() {
        let d = from_rdbn_lists(&[list()]);
        let t = &d.tables[0];
        assert_eq!(t.name, "chara_param");
        let cols: Vec<_> = t.columns.iter().map(|c| (c.name.as_str(), c.ty)).collect();
        assert_eq!(
            cols,
            vec![
                ("chara_id", ColumnType::BigInt),
                ("name", ColumnType::Text),
                ("pos_0", ColumnType::Real),
                ("pos_1", ColumnType::Real),
                ("pos_2", ColumnType::Real),
                ("pos_3", ColumnType::Real),
                ("tup_0", ColumnType::SmallInt),
                ("tup_1", ColumnType::SmallInt),
                ("ok", ColumnType::Bool),
                ("raw", ColumnType::Blob),
                ("pos", ColumnType::Text), // duplicate field name, invalid everywhere
            ]
        );
        assert_eq!(d.rows[0][1][1], Cell::Null);
        assert_eq!(d.rows[0][0][0], Cell::Int(0xFFFF_FFFF));
        assert_eq!(d.rows[0][0].len(), t.columns.len());
    }

    #[test]
    fn sqlite_executes_and_roundtrips() {
        let d = from_rdbn_lists(&[list()]);
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(&d.to_sql(Dialect::Sqlite)).unwrap();
        let (id, name, p1, tup, ok, raw): (i64, String, f64, i64, i64, Vec<u8>) = db
            .query_row(
                "SELECT chara_id, name, pos_1, tup_0, ok, raw FROM chara_param WHERE row_index = 0",
                [],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            (id, name.as_str(), p1, tup, ok, raw),
            (4_294_967_295, "O'Neil", 2.5, -1, 1, vec![0xde, 0xad])
        );
        let null: Option<String> = db
            .query_row(
                "SELECT name FROM chara_param WHERE row_index = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(null.is_none());
    }

    #[test]
    fn postgres_ddl() {
        let d = from_rdbn_lists(&[list()]);
        let sql = d.to_sql(Dialect::Postgres);
        assert!(sql.contains("\"chara_id\" BIGINT"));
        assert!(sql.contains("\"raw\" BYTEA"));
        assert!(sql.contains("'\\xdead'"));
        assert!(sql.contains("TRUE"));
    }

    #[test]
    fn duplicate_list_names_get_unique_tables() {
        let a = RdbnList {
            name: "X".into(),
            type_name: "T".into(),
            rows: vec![],
        };
        let d = from_rdbn_lists(&[a.clone(), a]);
        assert_eq!(d.tables[0].name, "x");
        assert_eq!(d.tables[1].name, "x_2");
    }

    #[test]
    fn t2b_tree() {
        let f = CfgBinFile {
            format: cfgbin::Format::T2b,
            entries: vec![CfgEntry {
                name: "ROOT".into(),
                variables: vec![Value::Int(3), Value::String("a".into()), Value::Float(0.5)],
                children: vec![CfgEntry {
                    name: "KID".into(),
                    variables: vec![],
                    children: vec![],
                }],
            }],
        };
        let d = from_t2b(&f);
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(&d.to_sql(Dialect::Sqlite)).unwrap();
        let kids: i64 = db
            .query_row("SELECT count(*) FROM t2b_entry WHERE parent = 0", [], |r| {
                r.get(0)
            })
            .unwrap();
        let vars: i64 = db
            .query_row("SELECT count(*) FROM t2b_var WHERE entry = 0", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!((kids, vars), (1, 3));
    }

    #[test]
    fn mixed_kinds_become_text_without_losing_rows() {
        let rows = vec![
            RdbnRow {
                fields: vec![("Condition".into(), RdbnValue::Hash(7))],
            },
            RdbnRow {
                fields: vec![("Condition".into(), RdbnValue::Condition("AAAAAAkC=".into()))],
            },
        ];
        let d = from_rdbn_lists(&[RdbnList {
            name: "L".into(),
            type_name: "T".into(),
            rows,
        }]);
        assert_eq!(d.tables[0].columns[0].ty, ColumnType::Text);
        assert_eq!(d.rows[0][0][0], Cell::Text("7".into()));
        assert_eq!(d.rows[0][1][0], Cell::Text("AAAAAAkC=".into()));
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(&d.to_sql(Dialect::Sqlite)).unwrap();
    }

    #[test]
    fn empty_string_table_header() {
        let mut h = vec![0u8; 32];
        h[0] = 3; // entries
        h[4] = 32; // string table offset
        assert!(has_empty_string_table(&h));
        h[8] = 1; // non-empty string table: handled by is_t2b instead
        assert!(!has_empty_string_table(&h));
        assert!(!has_empty_string_table(b"short"));
    }

    #[test]
    fn empty_t2b_is_two_empty_tables() {
        // The real 48-byte file: 0 entries, string table at 16, then an empty key table.
        let mut b = vec![0u8; 48];
        b[4] = 16;
        b[16] = 16;
        assert!(is_empty_t2b(&b));
        let d = from_bytes(&b).unwrap();
        assert_eq!(d.tables.len(), 2);
        assert!(d.rows.iter().all(Vec::is_empty));
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(&d.to_sql(Dialect::Sqlite)).unwrap();
    }

    #[test]
    fn rejects_garbage() {
        assert!(matches!(from_bytes(b"nope"), Err(Error::UnknownFormat)));
    }
}

pub mod pg;
