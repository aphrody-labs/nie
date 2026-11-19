//! Behaviour behind each exported symbol, named exactly like the contract entry.

use nie_cfgsql::Dialect;

/// `rg_cfgbin_to_sql`
#[must_use]
pub fn rg_cfgbin_to_sql(data: &[u8], dialect: i32) -> Option<String> {
    let dialect = if dialect == 0 {
        Dialect::Postgres
    } else {
        Dialect::Sqlite
    };
    nie_cfgsql::from_bytes(data).ok().map(|d| d.to_sql(dialect))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn behaviours() {
        assert!(rg_cfgbin_to_sql(b"nope", 0).is_none());
    }
}
