//! Behaviour behind each exported symbol, named exactly like the contract entry.

use nie_cfgsql::Dialect;

/// `rg_sitemap_encode_path`
#[must_use]
pub fn rg_sitemap_encode_path(path: &str) -> Option<String> {
    Some(nie_sitemap::encode_path(path))
}

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
        assert_eq!(
            rg_sitemap_encode_path("a b/c&d").as_deref(),
            Some("a%20b/c%26d")
        );
        assert!(rg_cfgbin_to_sql(b"nope", 0).is_none());
    }
}
