//! Queries over the PostgreSQL import written by `nie-cfgsql` (`import_vfs`).

use nie_sitemap::VfsFile;
use tokio_postgres::{Client, NoTls};

/// Error type of every query.
pub type DbError = tokio_postgres::Error;

/// Connection wrapper.
pub struct Db {
    client: Client,
}

/// One row of `vfs_file` (a decoded `cfg.bin`).
#[derive(Debug, Clone)]
pub struct FileRow {
    /// Numeric id (suffix of the `rdbn_<id>` schema).
    pub id: i32,
    /// VFS path.
    pub path: String,
    /// Pack file name.
    pub pack: String,
    /// Extracted size.
    pub size: i64,
    /// SHA-256 of the extracted bytes.
    pub sha256: String,
    /// `rdbn`, `t2b` or `unreadable`.
    pub format: String,
    /// Decode error, if any.
    pub error: Option<String>,
}

/// Counters for the home page.
#[derive(Debug, Clone, Default)]
pub struct Stats {
    /// Files in the VFS listing.
    pub entries: i64,
    /// `(format, files)` of decoded cfg.bin files.
    pub formats: Vec<(String, i64)>,
    /// Rows in `t2b_entry`.
    pub t2b_entries: i64,
    /// Rows in `t2b_var`.
    pub t2b_vars: i64,
}

/// Content of a directory page.
#[derive(Debug, Clone, Default)]
pub struct DirListing {
    /// Immediate sub-directories (names).
    pub dirs: Vec<String>,
    /// Immediate files `(name, size)`, at most `limit`.
    pub files: Vec<(String, i64)>,
    /// Total files in the directory.
    pub total_files: i64,
}

/// Quotes a SQL identifier.
#[must_use]
pub fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

impl Db {
    /// Connects and spawns the connection task.
    ///
    /// # Errors
    /// Connection errors.
    pub async fn connect(url: &str) -> Result<Self, DbError> {
        let (client, conn) = tokio_postgres::connect(url, NoTls).await?;
        tokio::spawn(async move {
            if let Err(e) = conn.await {
                eprintln!("postgres connection ended: {e}");
            }
        });
        Ok(Self { client })
    }

    /// Home page counters.
    ///
    /// # Errors
    /// Query errors.
    pub async fn stats(&self) -> Result<Stats, DbError> {
        let entries: i64 = self.client.query_one("SELECT count(*) FROM vfs_entry", &[]).await?.get(0);
        let formats = self
            .client
            .query("SELECT format, count(*) FROM vfs_file GROUP BY format ORDER BY format", &[])
            .await?
            .iter()
            .map(|r| (r.get(0), r.get(1)))
            .collect();
        let t2b_entries: i64 = self.client.query_one("SELECT count(*) FROM t2b_entry", &[]).await?.get(0);
        let t2b_vars: i64 = self.client.query_one("SELECT count(*) FROM t2b_var", &[]).await?.get(0);
        Ok(Stats { entries, formats, t2b_entries, t2b_vars })
    }

    /// Pack and size of a listed file, `None` when the path is not a file.
    ///
    /// # Errors
    /// Query errors.
    pub async fn entry(&self, path: &str) -> Result<Option<(String, i64)>, DbError> {
        Ok(self
            .client
            .query_opt("SELECT pack, size FROM vfs_entry WHERE path = $1 LIMIT 1", &[&path])
            .await?
            .map(|r| (r.get(0), r.get(1))))
    }

    /// Decoded-file row for a path.
    ///
    /// # Errors
    /// Query errors.
    pub async fn file(&self, path: &str) -> Result<Option<FileRow>, DbError> {
        Ok(self
            .client
            .query_opt("SELECT id, path, pack, size, sha256, format, error FROM vfs_file WHERE path = $1", &[&path])
            .await?
            .map(|r| FileRow { id: r.get(0), path: r.get(1), pack: r.get(2), size: r.get(3), sha256: r.get(4), format: r.get(5), error: r.get(6) }))
    }

    /// Lists one directory (`""` is the root).
    ///
    /// # Errors
    /// Query errors.
    pub async fn list_dir(&self, dir: &str, limit: i64) -> Result<DirListing, DbError> {
        let prefix = if dir.is_empty() { String::new() } else { format!("{dir}/") };
        let start = i32::try_from(prefix.len() + 1).unwrap_or(i32::MAX);
        let dirs = self
            .client
            .query(
                "SELECT sub FROM (SELECT DISTINCT split_part(substr(dir, $2), '/', 1) AS sub FROM vfs_entry WHERE starts_with(dir, $1)) s WHERE sub <> '' ORDER BY sub",
                &[&prefix, &start],
            )
            .await?
            .iter()
            .map(|r| r.get(0))
            .collect();
        let files = self
            .client
            .query("SELECT name, size FROM vfs_entry WHERE dir = $1 ORDER BY name LIMIT $2", &[&dir, &limit])
            .await?
            .iter()
            .map(|r| (r.get(0), r.get(1)))
            .collect();
        let total_files: i64 = self.client.query_one("SELECT count(*) FROM vfs_entry WHERE dir = $1", &[&dir]).await?.get(0);
        Ok(DirListing { dirs, files, total_files })
    }

    /// Every listed path with its size, sorted (feeds the sitemap).
    ///
    /// # Errors
    /// Query errors.
    pub async fn all_files(&self) -> Result<Vec<VfsFile>, DbError> {
        Ok(self
            .client
            .query("SELECT path, size FROM vfs_entry ORDER BY path", &[])
            .await?
            .iter()
            .map(|r| VfsFile { path: r.get(0), size: u64::try_from(r.get::<_, i64>(1)).unwrap_or(0) })
            .collect())
    }

    /// First `limit` T2B entries with their variables joined into one string.
    ///
    /// # Errors
    /// Query errors.
    pub async fn t2b_preview(&self, file_id: i32, limit: i64) -> Result<(Vec<String>, i64, Vec<Vec<String>>), DbError> {
        let total: i64 = self.client.query_one("SELECT count(*) FROM t2b_entry WHERE file_id = $1", &[&file_id]).await?.get(0);
        let rows = self
            .client
            .query(
                "SELECT e.row_index, e.parent, e.ord, e.name, \
                 (SELECT string_agg(coalesce(v.text_value, v.int_value::text, v.real_value::text, ''), ', ' ORDER BY v.ord) FROM t2b_var v WHERE v.file_id = e.file_id AND v.entry = e.row_index) \
                 FROM t2b_entry e WHERE e.file_id = $1 ORDER BY e.row_index LIMIT $2",
                &[&file_id, &limit],
            )
            .await?
            .iter()
            .map(|r| {
                vec![
                    r.get::<_, i32>(0).to_string(),
                    r.get::<_, Option<i32>>(1).map_or_else(|| "-".into(), |p| p.to_string()),
                    r.get::<_, i32>(2).to_string(),
                    r.get::<_, String>(3),
                    r.get::<_, Option<String>>(4).unwrap_or_default(),
                ]
            })
            .collect();
        let headers = ["row", "parent", "ord", "name", "values"].map(String::from).to_vec();
        Ok((headers, total, rows))
    }

    /// Tables of an RDBN file: `(schema, table, list name, row count)`.
    ///
    /// # Errors
    /// Query errors.
    pub async fn rdbn_tables(&self, file_id: i32) -> Result<Vec<(String, String, String, i32)>, DbError> {
        Ok(self
            .client
            .query("SELECT schema_name, table_name, list_name, row_count FROM vfs_table WHERE file_id = $1 ORDER BY table_name", &[&file_id])
            .await?
            .iter()
            .map(|r| (r.get(0), r.get(1), r.get(2), r.get(3)))
            .collect())
    }

    /// Column names and first `limit` rows of an RDBN table, every value as text.
    ///
    /// # Errors
    /// Query errors.
    pub async fn rdbn_preview(&self, schema: &str, table: &str, limit: i64) -> Result<(Vec<String>, Vec<Vec<String>>), DbError> {
        let cols: Vec<String> = self
            .client
            .query(
                "SELECT column_name FROM information_schema.columns WHERE table_schema = $1 AND table_name = $2 ORDER BY ordinal_position",
                &[&schema, &table],
            )
            .await?
            .iter()
            .map(|r| r.get(0))
            .collect();
        let sql = format!("SELECT to_jsonb(t)::text FROM {}.{} t ORDER BY row_index LIMIT $1", quote_ident(schema), quote_ident(table));
        let rows = self
            .client
            .query(&sql, &[&limit])
            .await?
            .iter()
            .map(|r| {
                let obj: serde_json::Value = serde_json::from_str(&r.get::<_, String>(0)).unwrap_or_default();
                cols.iter()
                    .map(|c| match obj.get(c) {
                        None | Some(serde_json::Value::Null) => "-".to_owned(),
                        Some(serde_json::Value::String(s)) => s.clone(),
                        Some(v) => v.to_string(),
                    })
                    .collect()
            })
            .collect();
        Ok((cols, rows))
    }
}

/// One character name row: hash id and text.
#[derive(Debug, Clone)]
pub struct Named {
    /// Name hash (`NOUN_INFO` first variable).
    pub id: i32,
    /// Text in the requested language.
    pub text: String,
}

impl Db {
    /// `NOUN_INFO` rows of the text file at `path` whose kind is `kind`, ordered by text, with paging.
    /// Variables: 0 = hash id, 1 = kind, 5 = text.
    ///
    /// # Errors
    /// Query errors.
    pub async fn nouns(&self, path: &str, entry: &str, kind: i32, search: &str, offset: i64, limit: i64) -> Result<(i64, Vec<Named>), DbError> {
        let sql = "WITH f AS (SELECT id FROM vfs_file WHERE path = $1), n AS ( \
              SELECT max(v.int_value) FILTER (WHERE v.ord = 0) AS hid, \
                     max(v.int_value) FILTER (WHERE v.ord = 1) AS kind, \
                     max(v.text_value) FILTER (WHERE v.ord = 5) AS txt \
              FROM t2b_entry e JOIN f ON e.file_id = f.id \
              JOIN t2b_var v ON v.file_id = e.file_id AND v.entry = e.row_index \
              WHERE e.name = $2 GROUP BY e.row_index) \
            , d AS (SELECT DISTINCT ON (txt) hid, txt FROM n \
              WHERE kind = $3 AND txt ~ '[[:alpha:]]{2}' AND txt ILIKE '%' || $4 || '%' ORDER BY txt, hid) \
            SELECT hid, txt, count(*) OVER () FROM d ORDER BY txt OFFSET $5 LIMIT $6";
        let rows = self.client.query(sql, &[&path, &entry, &kind, &search, &offset, &limit]).await?;
        let total = rows.first().map_or(0, |r| r.get::<_, i64>(2));
        Ok((total, rows.iter().map(|r| Named { id: r.get(0), text: r.get(1) }).collect()))
    }

    /// Every text of hash `id` in the `NOUN_INFO` file at `path`: `(kind, text)`.
    ///
    /// # Errors
    /// Query errors.
    pub async fn noun(&self, path: &str, entry: &str, id: i32) -> Result<Vec<(i32, String)>, DbError> {
        let sql = "WITH f AS (SELECT id FROM vfs_file WHERE path = $1), n AS ( \
              SELECT max(v.int_value) FILTER (WHERE v.ord = 0) AS hid, \
                     max(v.int_value) FILTER (WHERE v.ord = 1) AS kind, \
                     max(v.text_value) FILTER (WHERE v.ord = 5) AS txt \
              FROM t2b_entry e JOIN f ON e.file_id = f.id \
              JOIN t2b_var v ON v.file_id = e.file_id AND v.entry = e.row_index \
              WHERE e.name = $2 GROUP BY e.row_index) \
            SELECT kind, txt FROM n WHERE hid = $3 AND txt <> '' ORDER BY kind";
        let rows = self.client.query(sql, &[&path, &entry, &id]).await?;
        Ok(rows.iter().map(|r| (r.get(0), r.get(1))).collect())
    }
}
