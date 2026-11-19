//! Bounded read-only reports over the reverse-engineering knowledge database (`var/nie.sqlite`).
//!
//! Transport adapters (the MCP `re_query`, `re_function` and `re_coverage` tools) call these
//! functions so SQL, address formatting and row limits stay in one place.

use rusqlite::Connection;
use serde_json::{Value, json};

/// Accepts only read-only statements (`SELECT`, `PRAGMA`, `EXPLAIN`, `WITH ... SELECT`).
///
/// # Errors
/// Returns an error for any other statement.
pub fn check_readonly_sql(sql: &str) -> anyhow::Result<()> {
    let trimmed = sql.trim().to_uppercase();
    let allowed = trimmed.starts_with("SELECT")
        || trimmed.starts_with("PRAGMA")
        || trimmed.starts_with("EXPLAIN")
        || trimmed.starts_with("WITH");
    anyhow::ensure!(
        allowed,
        "statement refused: only SELECT, PRAGMA, EXPLAIN and WITH ... SELECT are accepted.\n\
         Received: {}",
        &sql[..sql.len().min(80)]
    );
    Ok(())
}

/// Controls bounded, JSON-safe execution of an arbitrary read-only SQLite query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqliteQueryOptions {
    pub limit: usize,
    pub hexadecimal_integer_columns: Vec<String>,
}

impl SqliteQueryOptions {
    /// Compatibility policy for reverse-engineering databases.
    #[must_use]
    pub fn re_database(limit: usize) -> Self {
        Self {
            limit,
            hexadecimal_integer_columns: [
                "vaddr",
                "from_addr",
                "to_addr",
                "addr",
                "base_addr",
                "va",
                "target_addr",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        }
    }
}

/// One bounded page returned by [`exec_readonly_sql_page`].
#[derive(Debug, Clone, PartialEq)]
pub struct SqliteQueryPage {
    pub rows: Vec<Value>,
    pub truncated: bool,
    pub columns: Vec<String>,
}

/// Executes a read-only statement and coerces SQLite values to transport-safe JSON.
pub fn exec_readonly_sql_page(
    connection: &Connection,
    sql: &str,
    options: SqliteQueryOptions,
) -> anyhow::Result<SqliteQueryPage> {
    use rusqlite::types::ValueRef;

    check_readonly_sql(sql)?;
    let mut statement = connection.prepare(sql)?;
    anyhow::ensure!(
        statement.readonly(),
        "SQLite rejected the query as non-read-only"
    );
    let columns = statement
        .column_names()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut cursor = statement.query([])?;
    let mut rows = Vec::new();
    let mut truncated = false;
    while let Some(row) = cursor.next()? {
        if rows.len() >= options.limit {
            truncated = true;
            break;
        }
        let mut object = serde_json::Map::new();
        for (index, column) in columns.iter().enumerate() {
            let value = match row.get_ref(index)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(value)
                    if options.hexadecimal_integer_columns.contains(column) =>
                {
                    Value::String(format!("0x{value:x}"))
                }
                ValueRef::Integer(value)
                    if (-(1_i64 << 53) + 1..=(1_i64 << 53) - 1).contains(&value) =>
                {
                    Value::Number(value.into())
                }
                ValueRef::Integer(value) => Value::String(value.to_string()),
                ValueRef::Real(value) => serde_json::Number::from_f64(value)
                    .map(Value::Number)
                    .unwrap_or(Value::Null),
                ValueRef::Text(value) => Value::String(String::from_utf8_lossy(value).into_owned()),
                ValueRef::Blob(value) => Value::String(format!("<blob {} bytes>", value.len())),
            };
            object.insert(column.clone(), value);
        }
        rows.push(Value::Object(object));
    }
    Ok(SqliteQueryPage {
        rows,
        truncated,
        columns,
    })
}

/// Bounded reverse-engineering function lookup used by transport adapters.
pub fn re_function_report(
    connection: &Connection,
    name: Option<&str>,
    vaddr: Option<i64>,
) -> anyhow::Result<Value> {
    let predicate = match (name, vaddr) {
        (Some(name), _) if !name.trim().is_empty() => {
            format!("name LIKE '%{}%'", name.replace('\'', "''"))
        }
        (_, Some(vaddr)) => format!("vaddr = {vaddr}"),
        _ => anyhow::bail!("re_function requires name or vaddr"),
    };
    let columns = "id, binary_id, vaddr, size, name, name_source, confidence, cc, n_args, subsystem, role, pagerank, ret_type, params, n_calls_in, n_calls_out, complexity";
    let page = exec_readonly_sql_page(
        connection,
        &format!(
            "SELECT {columns} FROM function WHERE {predicate} ORDER BY pagerank DESC, binary_id LIMIT 25"
        ),
        SqliteQueryOptions::re_database(25),
    )?;
    let mut result = json!({"query":{"name":name,"vaddr":vaddr.map(|v|format!("0x{v:x}"))},"total_matches":page.rows.len(),"matches":page.rows});
    if let Some(best) = result["matches"].as_array().and_then(|rows| rows.first()) {
        let binary_id = best["binary_id"]
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("function row has no binary_id"))?;
        let address = best["vaddr"]
            .as_str()
            .and_then(|v| v.strip_prefix("0x"))
            .and_then(|v| i64::from_str_radix(v, 16).ok())
            .ok_or_else(|| anyhow::anyhow!("function row has no vaddr"))?;
        let incoming=exec_readonly_sql_page(connection,&format!("SELECT x.from_addr, x.kind, f.name AS from_name FROM xref x LEFT JOIN function f ON f.vaddr = x.from_addr AND f.binary_id = x.binary_id WHERE x.to_addr = {address} AND x.binary_id = {binary_id} LIMIT 12"),SqliteQueryOptions::re_database(12))?.rows;
        let outgoing=exec_readonly_sql_page(connection,&format!("SELECT x.to_addr, x.kind, f.name AS to_name FROM xref x LEFT JOIN function f ON f.vaddr = x.to_addr AND f.binary_id = x.binary_id WHERE x.from_addr = {address} AND x.binary_id = {binary_id} LIMIT 12"),SqliteQueryOptions::re_database(12))?.rows;
        result["xrefs"] = json!({"of":best["name"].as_str().map_or_else(||format!("0x{address:x}"),str::to_owned),"incoming":incoming,"outgoing":outgoing});
    }
    Ok(result)
}

/// Aggregates the canonical RE coverage tables without exposing SQL to bindings.
pub fn re_coverage_report(connection: &Connection) -> anyhow::Result<Value> {
    let run = |sql, limit| {
        exec_readonly_sql_page(connection, sql, SqliteQueryOptions::re_database(limit))
            .map(|p| p.rows)
    };
    let latest = run(
        "SELECT ts, binary_id, total_funcs, named, classified, pct FROM coverage ORDER BY id DESC LIMIT 1",
        1,
    )?;
    let total = run("SELECT COUNT(*) AS n FROM function", 1)?;
    let per_binary = run(
        "SELECT binary_id, COUNT(*) AS rows_total, SUM(name IS NOT NULL) AS named FROM function GROUP BY binary_id ORDER BY binary_id",
        1_000,
    )?;
    Ok(
        json!({"latest":latest.into_iter().next(),"function_rows_total":total.first().and_then(|row|row["n"].as_i64()).unwrap_or(0),"per_binary":per_binary,"primary_binary_id":2}),
    )
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use serde_json::json;

    use super::{
        SqliteQueryOptions, exec_readonly_sql_page, re_coverage_report, re_function_report,
    };

    #[test]
    fn re_reports_preserve_mcp_shape_and_literal_name_search() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE function(id INTEGER, binary_id INTEGER, vaddr INTEGER, size INTEGER, name TEXT, name_source TEXT, confidence REAL, cc TEXT, n_args INTEGER, subsystem TEXT, role TEXT, pagerank REAL, ret_type TEXT, params TEXT, n_calls_in INTEGER, n_calls_out INTEGER, complexity INTEGER); CREATE TABLE xref(binary_id INTEGER, from_addr INTEGER, to_addr INTEGER, kind TEXT); CREATE TABLE coverage(id INTEGER, ts TEXT, binary_id INTEGER, total_funcs INTEGER, named INTEGER, classified INTEGER, pct REAL); INSERT INTO function VALUES(1,2,4096,4,'O''Brien',NULL,0.5,NULL,NULL,'menu',NULL,1.0,NULL,NULL,0,0,1); INSERT INTO coverage VALUES(1,'now',2,1,1,1,100.0);").unwrap();
        let function = re_function_report(&conn, Some("O'Brien"), None).unwrap();
        assert_eq!(function["total_matches"], 1);
        assert_eq!(function["matches"][0]["name"], "O'Brien");
        let coverage = re_coverage_report(&conn).unwrap();
        assert_eq!(coverage["function_rows_total"], 1);
        assert_eq!(coverage["primary_binary_id"], 2);
    }

    #[test]
    fn sqlite_query_page_preserves_transport_safe_values_and_truncation() {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        conn.execute_batch(
            "CREATE TABLE sample (
                vaddr INTEGER, safe INTEGER, big INTEGER, ratio REAL,
                name TEXT, payload BLOB, missing TEXT
             );
             INSERT INTO sample VALUES
                (5368709120, 9007199254740991, 9007199254740992, 1.5, 'first', X'0102', NULL),
                (5368709121, 2, 3, 2.5, 'second', X'', NULL);",
        )
        .expect("seed fixture");

        let page = exec_readonly_sql_page(
            &conn,
            "SELECT vaddr, safe, big, ratio, name, payload, missing FROM sample ORDER BY vaddr",
            SqliteQueryOptions::re_database(1),
        )
        .expect("read-only page");

        assert_eq!(
            page.columns,
            [
                "vaddr", "safe", "big", "ratio", "name", "payload", "missing"
            ]
        );
        assert_eq!(
            page.rows,
            vec![json!({
                "vaddr": "0x140000000",
                "safe": 9007199254740991_i64,
                "big": "9007199254740992",
                "ratio": 1.5,
                "name": "first",
                "payload": "<blob 2 bytes>",
                "missing": null
            })]
        );
        assert!(page.truncated);
        assert!(
            exec_readonly_sql_page(
                &conn,
                "DELETE FROM sample",
                SqliteQueryOptions::re_database(1)
            )
            .is_err()
        );
    }
}
