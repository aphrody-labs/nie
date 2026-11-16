//! Whole-VFS import into PostgreSQL.
//!
//! [`PgImporter`] streams one SQL script (psql syntax) for every `cfg.bin` of the VFS:
//!
//! - `vfs_file` is the provenance table: VFS path, pack, size, SHA-256 of the extracted bytes,
//!   detected format, schema name, decode error if any. `vfs_table` lists every generated table.
//! - T2B files share `t2b_entry` / `t2b_var`, keyed by `file_id` (names are not unique across
//!   files, so they cannot be one table per file); real foreign keys tie them together.
//! - RDBN files get their own schema `rdbn_<file_id>` holding one table per list (list and
//!   column names are the game's own and differ between versions of a file).
//! - Rows load with `COPY ... FROM STDIN` (text format), constraints are added after the load.
//! - Foreign keys between RDBN tables are inferred from the data, see [`infer_foreign_keys`].

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};

use crate::{Cell, ColumnType, Dataset, Dialect, Error, Table, qualified, quote_ident};

/// Where a decoded file came from.
#[derive(Debug, Clone)]
pub struct FileMeta {
    /// VFS path.
    pub path: String,
    /// Pack file name ("" for a loose file).
    pub pack: String,
    /// Extracted size in bytes.
    pub size: u64,
    /// Lower-case hex SHA-256 of the extracted bytes.
    pub sha256: String,
}

/// A foreign key inferred from the data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignKey {
    /// Referencing table.
    pub table: String,
    /// Referencing column.
    pub column: String,
    /// Referenced table.
    pub ref_table: String,
    /// Referenced column (made `UNIQUE`).
    pub ref_column: String,
}

fn key_of(c: &Cell) -> Option<String> {
    match c {
        Cell::Int(n) => Some(n.to_string()),
        Cell::Text(s) => Some(s.clone()),
        _ => None,
    }
}

/// Infers foreign keys inside one file. A column `a.c` references `b.c` when all of these hold:
/// same column name, integer-hash or text type or a name ending in `id`, `b.c` holds at least 4
/// distinct values and no duplicates and no NULL, and every non-NULL value of `a.c` is in `b.c`.
/// Value containment is verified on the real rows, so a declared key is true for this data.
#[must_use]
pub fn infer_foreign_keys(data: &Dataset) -> Vec<ForeignKey> {
    // (table index, column index, set of keys) for candidate referenced columns
    let mut targets: Vec<(usize, usize, BTreeSet<String>)> = Vec::new();
    let eligible = |name: &str, ty: ColumnType| {
        name != "row_index"
            && (matches!(ty, ColumnType::BigInt | ColumnType::Text) || name.ends_with("id"))
            && matches!(ty, ColumnType::BigInt | ColumnType::Int | ColumnType::SmallInt | ColumnType::Text)
    };
    for (ti, (t, rows)) in data.tables.iter().zip(&data.rows).enumerate() {
        for (ci, col) in t.columns.iter().enumerate() {
            if !eligible(&col.name, col.ty) || rows.len() < 4 {
                continue;
            }
            let mut set = BTreeSet::new();
            let unique = rows.iter().all(|r| key_of(&r[ci]).is_some_and(|k| set.insert(k)));
            if unique {
                targets.push((ti, ci, set));
            }
        }
    }
    let mut out = Vec::new();
    for (ti, ci, set) in &targets {
        let name = &data.tables[*ti].columns[*ci].name;
        for (ai, (a, rows)) in data.tables.iter().zip(&data.rows).enumerate() {
            if ai == *ti {
                continue;
            }
            let Some(ac) = a.columns.iter().position(|c| &c.name == name && c.ty == data.tables[*ti].columns[*ci].ty) else {
                continue;
            };
            let mut seen = false;
            let ok = rows.iter().all(|r| match key_of(&r[ac]) {
                Some(k) => {
                    seen = true;
                    set.contains(&k)
                }
                None => matches!(r[ac], Cell::Null),
            });
            if ok && seen {
                out.push(ForeignKey {
                    table: a.name.clone(),
                    column: name.clone(),
                    ref_table: data.tables[*ti].name.clone(),
                    ref_column: name.clone(),
                });
            }
        }
    }
    out
}

/// Escapes one value for COPY text format.
fn copy_value(cell: &Cell, out: &mut String) {
    use std::fmt::Write as _;
    match cell {
        Cell::Null => out.push_str("\\N"),
        Cell::Bool(b) => out.push(if *b { 't' } else { 'f' }),
        Cell::Int(n) => {
            let _ = write!(out, "{n}");
        }
        Cell::Real(f) if f.is_finite() => {
            let _ = write!(out, "{f:?}");
        }
        Cell::Real(_) => out.push_str("\\N"),
        Cell::Text(s) => copy_text(s, out),
        Cell::Blob(b) => {
            out.push_str("\\\\x");
            for byte in b {
                let _ = write!(out, "{byte:02x}");
            }
        }
    }
}

fn copy_text(s: &str, out: &mut String) {
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => {} // PostgreSQL text cannot hold NUL
            c => out.push(c),
        }
    }
}

/// `COPY` block (statement, rows, terminator) for rows already rendered as lines.
fn copy_block(target: &str, cols: &[String], body: &str) -> String {
    format!("COPY {target} ({}) FROM STDIN;\n{body}\\.\n", cols.join(", "))
}

const FLUSH_BYTES: usize = 8 << 20;

/// Streaming script writer.
pub struct PgImporter<W: Write> {
    out: W,
    next_id: i32,
    files: String,
    tables: String,
    entry_buf: String,
    var_buf: String,
    listing_buf: String,
    /// Files imported as RDBN, T2B, or recorded as unreadable.
    pub counts: BTreeMap<&'static str, u64>,
    /// Number of foreign keys inferred between RDBN tables.
    pub inferred_fks: u64,
}

const ENTRY_COLS: [&str; 5] = ["file_id", "row_index", "parent", "ord", "name"];
const VAR_COLS: [&str; 8] = ["file_id", "row_index", "entry", "ord", "kind", "int_value", "real_value", "text_value"];

impl<W: Write> PgImporter<W> {
    /// Writes the fixed DDL and returns the importer.
    ///
    /// # Errors
    /// I/O errors of `out`.
    pub fn new(mut out: W) -> io::Result<Self> {
        out.write_all(
            b"\\set ON_ERROR_STOP on\nSET client_min_messages = warning;\nBEGIN;\n\
CREATE TABLE vfs_file (id INTEGER PRIMARY KEY, path TEXT NOT NULL UNIQUE, pack TEXT NOT NULL, size BIGINT NOT NULL, sha256 TEXT NOT NULL, format TEXT NOT NULL, schema_name TEXT, error TEXT);\n\
CREATE TABLE vfs_table (file_id INTEGER NOT NULL, schema_name TEXT NOT NULL, table_name TEXT NOT NULL, list_name TEXT NOT NULL, type_name TEXT NOT NULL, row_count INTEGER NOT NULL);\n\
CREATE TABLE vfs_entry (path TEXT NOT NULL, dir TEXT NOT NULL, name TEXT NOT NULL, pack TEXT NOT NULL, size BIGINT NOT NULL);\n\
CREATE TABLE t2b_entry (file_id INTEGER NOT NULL, row_index INTEGER NOT NULL, parent INTEGER, ord INTEGER NOT NULL, name TEXT NOT NULL);\n\
CREATE TABLE t2b_var (file_id INTEGER NOT NULL, row_index INTEGER NOT NULL, entry INTEGER NOT NULL, ord INTEGER NOT NULL, kind TEXT NOT NULL, int_value INTEGER, real_value REAL, text_value TEXT);\n",
        )?;
        Ok(Self {
            out,
            next_id: 1,
            files: String::new(),
            tables: String::new(),
            entry_buf: String::new(),
            var_buf: String::new(),
            listing_buf: String::new(),
            counts: BTreeMap::new(),
            inferred_fks: 0,
        })
    }

    fn flush_t2b(&mut self) -> io::Result<()> {
        if !self.entry_buf.is_empty() {
            let cols: Vec<String> = ENTRY_COLS.iter().map(|c| quote_ident(c)).collect();
            self.out.write_all(copy_block("t2b_entry", &cols, &self.entry_buf).as_bytes())?;
            self.entry_buf.clear();
        }
        if !self.var_buf.is_empty() {
            let cols: Vec<String> = VAR_COLS.iter().map(|c| quote_ident(c)).collect();
            self.out.write_all(copy_block("t2b_var", &cols, &self.var_buf).as_bytes())?;
            self.var_buf.clear();
        }
        Ok(())
    }

    fn file_row(&mut self, id: i32, meta: &FileMeta, format: &str, schema: Option<&str>, error: Option<&str>) {
        let mut line = format!("{id}\t");
        copy_text(&meta.path, &mut line);
        line.push('\t');
        copy_text(&meta.pack, &mut line);
        line.push_str(&format!("\t{}\t{}\t{format}\t", meta.size, meta.sha256));
        line.push_str(schema.unwrap_or("\\N"));
        line.push('\t');
        match error {
            Some(e) => copy_text(e, &mut line),
            None => line.push_str("\\N"),
        }
        line.push('\n');
        self.files.push_str(&line);
    }

    fn table_row(&mut self, id: i32, schema: &str, t: &Table, rows: usize) {
        self.tables.push_str(&format!("{id}\t{schema}\t"));
        copy_text(&t.name, &mut self.tables);
        self.tables.push('\t');
        copy_text(&t.list_name, &mut self.tables);
        self.tables.push('\t');
        copy_text(&t.type_name, &mut self.tables);
        self.tables.push_str(&format!("\t{rows}\n"));
    }

    fn flush_listing(&mut self) -> io::Result<()> {
        if !self.listing_buf.is_empty() {
            let cols: Vec<String> = ["path", "dir", "name", "pack", "size"].iter().map(|c| quote_ident(c)).collect();
            self.out.write_all(copy_block("vfs_entry", &cols, &self.listing_buf).as_bytes())?;
            self.listing_buf.clear();
        }
        Ok(())
    }

    /// Records one logical VFS file (any type) in `vfs_entry`, with its directory split out so
    /// directory pages are an indexed lookup.
    ///
    /// # Errors
    /// I/O errors of the output.
    pub fn add_listing(&mut self, path: &str, pack: &str, size: u64) -> io::Result<()> {
        let (dir, name) = path.rsplit_once('/').unwrap_or(("", path));
        for (i, v) in [path, dir, name, pack].iter().enumerate() {
            if i > 0 {
                self.listing_buf.push('\t');
            }
            copy_text(v, &mut self.listing_buf);
        }
        self.listing_buf.push_str(&format!("\t{size}\n"));
        if self.listing_buf.len() > FLUSH_BYTES {
            self.flush_listing()?;
        }
        Ok(())
    }

    /// Adds one file: its provenance row and, when decoded, its rows.
    ///
    /// # Errors
    /// I/O errors of the output.
    pub fn add(&mut self, meta: &FileMeta, decoded: Result<Dataset, Error>) -> io::Result<()> {
        let id = self.next_id;
        self.next_id += 1;
        let data = match decoded {
            Ok(d) => d,
            Err(e) => {
                *self.counts.entry("unreadable").or_default() += 1;
                self.file_row(id, meta, "unreadable", None, Some(&e.to_string()));
                return Ok(());
            }
        };
        let is_t2b = data.tables.len() == 2 && data.tables[0].name == "t2b_entry" && data.tables[1].name == "t2b_var";
        if is_t2b {
            *self.counts.entry("t2b").or_default() += 1;
            self.file_row(id, meta, "t2b", None, None);
            for (t, rows) in data.tables.iter().zip(&data.rows) {
                self.table_row(id, "public", t, rows.len());
            }
            for (i, row) in data.rows[0].iter().enumerate() {
                let mut line = format!("{id}\t{i}");
                for c in row {
                    line.push('\t');
                    copy_value(c, &mut line);
                }
                line.push('\n');
                self.entry_buf.push_str(&line);
            }
            for (i, row) in data.rows[1].iter().enumerate() {
                let mut line = format!("{id}\t{i}");
                for c in row {
                    line.push('\t');
                    copy_value(c, &mut line);
                }
                line.push('\n');
                self.var_buf.push_str(&line);
            }
            if self.entry_buf.len() + self.var_buf.len() > FLUSH_BYTES {
                self.flush_t2b()?;
            }
            return Ok(());
        }
        *self.counts.entry("rdbn").or_default() += 1;
        let schema = format!("rdbn_{id}");
        self.file_row(id, meta, "rdbn", Some(&schema), None);
        writeln!(self.out, "CREATE SCHEMA {};", quote_ident(&schema))?;
        for (t, rows) in data.tables.iter().zip(&data.rows) {
            self.table_row(id, &schema, t, rows.len());
            self.out.write_all(t.create_sql_in(Dialect::Postgres, Some(&schema)).as_bytes())?;
            let mut cols = vec![quote_ident("row_index")];
            cols.extend(t.columns.iter().map(|c| quote_ident(&c.name)));
            let mut body = String::new();
            for (i, row) in rows.iter().enumerate() {
                body.push_str(&i.to_string());
                for c in row {
                    body.push('\t');
                    copy_value(c, &mut body);
                }
                body.push('\n');
            }
            if !rows.is_empty() {
                self.out.write_all(copy_block(&qualified(Some(&schema), &t.name), &cols, &body).as_bytes())?;
            }
        }
        for (n, fk) in infer_foreign_keys(&data).iter().enumerate() {
            self.inferred_fks += 1;
            writeln!(
                self.out,
                "ALTER TABLE {r} ADD CONSTRAINT {u} UNIQUE ({c});\nALTER TABLE {a} ADD CONSTRAINT {f} FOREIGN KEY ({c}) REFERENCES {r} ({c});",
                r = qualified(Some(&schema), &fk.ref_table),
                a = qualified(Some(&schema), &fk.table),
                c = quote_ident(&fk.column),
                u = quote_ident(&format!("uq_{id}_{n}")),
                f = quote_ident(&format!("fk_{id}_{n}")),
            )?;
        }
        Ok(())
    }

    /// Flushes buffers, loads the provenance tables, adds the keys and commits.
    ///
    /// # Errors
    /// I/O errors of the output.
    pub fn finish(mut self) -> io::Result<W> {
        self.flush_t2b()?;
        self.flush_listing()?;
        let file_cols: Vec<String> = ["id", "path", "pack", "size", "sha256", "format", "schema_name", "error"].iter().map(|c| quote_ident(c)).collect();
        let table_cols: Vec<String> = ["file_id", "schema_name", "table_name", "list_name", "type_name", "row_count"].iter().map(|c| quote_ident(c)).collect();
        self.out.write_all(copy_block("vfs_file", &file_cols, &self.files).as_bytes())?;
        self.out.write_all(copy_block("vfs_table", &table_cols, &self.tables).as_bytes())?;
        self.out.write_all(
            b"ALTER TABLE t2b_entry ADD PRIMARY KEY (file_id, row_index);\n\
ALTER TABLE t2b_var ADD PRIMARY KEY (file_id, row_index);\n\
ALTER TABLE t2b_entry ADD FOREIGN KEY (file_id) REFERENCES vfs_file (id);\n\
ALTER TABLE t2b_entry ADD FOREIGN KEY (file_id, parent) REFERENCES t2b_entry (file_id, row_index);\n\
ALTER TABLE t2b_var ADD FOREIGN KEY (file_id, entry) REFERENCES t2b_entry (file_id, row_index);\n\
CREATE INDEX vfs_entry_dir ON vfs_entry (dir);\n\
CREATE INDEX t2b_entry_name ON t2b_entry (name);\n\
CREATE INDEX t2b_var_entry ON t2b_var (file_id, entry);\n\
ALTER TABLE vfs_table ADD FOREIGN KEY (file_id) REFERENCES vfs_file (id);\n\
COMMIT;\nANALYZE;\n",
        )?;
        self.out.flush()?;
        Ok(self.out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::from_rdbn_lists;
    use nie_formats::cfgbin::{RdbnList, RdbnRow, RdbnValue};

    fn lists() -> Vec<RdbnList> {
        let mk = |name: &str, vals: Vec<(u32, &str)>| RdbnList {
            name: name.into(),
            type_name: "T".into(),
            rows: vals
                .into_iter()
                .map(|(h, s)| RdbnRow { fields: vec![("CharaId".into(), RdbnValue::Hash(h)), ("Name".into(), RdbnValue::Condition(s.into()))] })
                .collect(),
        };
        vec![
            mk("Chara", vec![(10, "a\tb"), (11, "c\\d"), (12, "e\nf"), (13, "g")]),
            mk("Link", vec![(11, "x"), (13, "y"), (11, "z")]),
            mk("Other", vec![(99, "q")]),
        ]
    }

    #[test]
    fn foreign_key_is_inferred_and_true() {
        let d = from_rdbn_lists(&lists());
        let fks = infer_foreign_keys(&d);
        assert_eq!(
            fks,
            vec![ForeignKey { table: "link".into(), column: "chara_id".into(), ref_table: "chara".into(), ref_column: "chara_id".into() }]
        );
    }

    #[test]
    fn script_has_copy_escapes_and_provenance() {
        let mut imp = PgImporter::new(Vec::new()).unwrap();
        let meta = FileMeta { path: "data/x.cfg.bin".into(), pack: "p.cpk".into(), size: 7, sha256: "ab".into() };
        imp.add(&meta, Ok(from_rdbn_lists(&lists()))).unwrap();
        imp.add(&FileMeta { path: "bad".into(), ..meta.clone() }, Err(Error::UnknownFormat)).unwrap();
        assert_eq!(imp.inferred_fks, 1);
        let sql = String::from_utf8(imp.finish().unwrap()).unwrap();
        assert!(sql.contains("CREATE SCHEMA \"rdbn_1\";"));
        assert!(sql.contains("COPY \"rdbn_1\".\"chara\""));
        assert!(sql.contains("1\t0\t10\ta\\tb") || sql.contains("0\t10\ta\\tb"));
        assert!(sql.contains("c\\\\d") && sql.contains("e\\nf"));
        assert!(sql.contains("FOREIGN KEY (\"chara_id\") REFERENCES \"rdbn_1\".\"chara\""));
        assert!(sql.contains("2\tbad\tp.cpk\t7\tab\tunreadable"));
        assert!(sql.trim_end().ends_with("ANALYZE;"));
    }

    #[test]
    fn t2b_goes_to_shared_tables() {
        use nie_formats::cfgbin::{CfgBinFile, CfgEntry, Format, Value};
        let f = CfgBinFile {
            format: Format::T2b,
            entries: vec![CfgEntry { name: "R".into(), variables: vec![Value::Int(1)], children: vec![CfgEntry { name: "K".into(), variables: vec![], children: vec![] }] }],
        };
        let mut imp = PgImporter::new(Vec::new()).unwrap();
        let meta = FileMeta { path: "t".into(), pack: String::new(), size: 1, sha256: "00".into() };
        imp.add(&meta, Ok(crate::from_t2b(&f))).unwrap();
        let sql = String::from_utf8(imp.finish().unwrap()).unwrap();
        assert!(sql.contains("COPY t2b_entry"));
        assert!(sql.contains("1\t1\t0\t0\tK"), "child row: file 1, row 1, parent 0, ord 0");
        assert!(sql.contains("COPY t2b_var"));
    }
}
