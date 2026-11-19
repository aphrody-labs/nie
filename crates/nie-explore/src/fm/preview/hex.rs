//! The hex view, the fallback of every previewer.

use super::{Body, Built};

/// Bytes kept for the hex view; the renderer scrolls inside them.
pub const HEX_BYTES: usize = 64 * 1024;

/// The first [`HEX_BYTES`] bytes of the file.
pub(super) fn view(data: &[u8]) -> Built {
    (
        Body::Hex {
            bytes: data[..data.len().min(HEX_BYTES)].to_vec(),
            total: data.len(),
        },
        vec![("view".to_owned(), "hex".to_owned())],
        false,
    )
}

/// A hex view that says why the structured view is missing.
pub(super) fn message_with_hex(data: &[u8], why: &str) -> Built {
    let (body, mut meta, cached) = view(data);
    meta.push(("note".to_owned(), why.to_owned()));
    (body, meta, cached)
}

/// One row of a hex dump: offset, up to 16 bytes, ASCII gutter.
#[must_use]
pub fn row(offset: usize, bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut line = format!("{offset:08x}  ");
    for i in 0..16 {
        match bytes.get(i) {
            Some(b) => {
                let _ = write!(line, "{b:02x} ");
            }
            None => line.push_str("   "),
        }
        if i == 7 {
            line.push(' ');
        }
    }
    line.push(' ');
    line.extend(
        bytes
            .iter()
            .map(|b| if (0x20..0x7f).contains(b) { char::from(*b) } else { '.' }),
    );
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_align_and_show_ascii() {
        let line = row(0x10, b"G4TX\0\x01hello");
        assert!(line.starts_with("00000010  47 34 54 58 00 01 68 65  6c 6c 6f "));
        assert!(line.ends_with("G4TX..hello"));
    }
}
