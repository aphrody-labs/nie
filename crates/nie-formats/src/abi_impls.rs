//! Behaviour behind each exported symbol, named exactly like the contract entry.

/// `nie_formats_detect`
#[must_use]
pub fn nie_formats_detect(data: &[u8]) -> Option<String> {
    Some(match crate::detect(data) {
        crate::FileFormat::Unknown => String::from("unknown"),
        format => format!("{format:?}").to_lowercase(),
    })
}

/// `nie_formats_crc32`
#[must_use]
pub fn nie_formats_crc32(data: &[u8]) -> u32 {
    crate::cfgbin::crc32(data)
}

/// `nie_formats_cfgbin_to_json`
#[must_use]
pub fn nie_formats_cfgbin_to_json(data: &[u8]) -> Option<String> {
    crate::cfgbin::to_tagged_json(data).map(|value| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn behaviours() {
        assert_eq!(nie_formats_detect(b"CPK \0\0\0\0").as_deref(), Some("cpk"));
        assert_eq!(nie_formats_detect(b"nope").as_deref(), Some("unknown"));
        assert_eq!(
            nie_formats_crc32(b"PlayerParam"),
            crate::cfgbin::crc32(b"PlayerParam")
        );
        assert!(nie_formats_cfgbin_to_json(b"nope").is_none());
    }
}
