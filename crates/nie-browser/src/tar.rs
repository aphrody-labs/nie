//! A minimal reader for the ustar archives crates.io serves (`.crate` files, gzip-compressed).

use std::{collections::BTreeMap, io::Read};

use anyhow::{Result, bail};
use flate2::read::GzDecoder;

const BLOCK: usize = 512;

/// Regular files of a gzip-compressed tar archive, keyed by path without the leading directory.
///
/// # Errors
/// A truncated archive or a header that is not ustar.
pub fn read_crate(gz: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut raw = Vec::new();
    GzDecoder::new(gz).read_to_end(&mut raw)?;
    read_tar(&raw)
}

fn field(buf: &[u8]) -> &[u8] {
    let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
    &buf[..end]
}

fn octal(buf: &[u8]) -> Result<usize> {
    let s = std::str::from_utf8(field(buf))?.trim();
    if s.is_empty() {
        return Ok(0);
    }
    Ok(usize::from_str_radix(s, 8)?)
}

fn read_tar(raw: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    let mut at = 0;
    let mut long_name: Option<String> = None;
    while at + BLOCK <= raw.len() {
        let header = &raw[at..at + BLOCK];
        if header.iter().all(|b| *b == 0) {
            break;
        }
        let size = octal(&header[124..136])?;
        let kind = header[156];
        let mut name = String::from_utf8_lossy(field(&header[0..100])).into_owned();
        let prefix = field(&header[345..500]);
        if !prefix.is_empty() {
            name = format!("{}/{name}", String::from_utf8_lossy(prefix));
        }
        let body_start = at + BLOCK;
        let body_end = body_start + size;
        if body_end > raw.len() {
            bail!("tar entry {name} is truncated");
        }
        let body = &raw[body_start..body_end];
        match kind {
            b'L' => long_name = Some(String::from_utf8_lossy(field(body)).into_owned()),
            b'0' | 0 => {
                let full = long_name.take().unwrap_or(name);
                let rel = full.split_once('/').map_or(full.as_str(), |(_, rest)| rest);
                files.insert(rel.to_owned(), body.to_vec());
            }
            _ => {}
        }
        at = body_start + size.div_ceil(BLOCK) * BLOCK;
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, body: &[u8]) -> Vec<u8> {
        let mut h = vec![0u8; BLOCK];
        h[..name.len()].copy_from_slice(name.as_bytes());
        let size = format!("{:011o}", body.len());
        h[124..135].copy_from_slice(size.as_bytes());
        h[156] = b'0';
        let mut out = h;
        out.extend_from_slice(body);
        out.resize(out.len().div_ceil(BLOCK) * BLOCK, 0);
        out
    }

    #[test]
    fn reads_files_and_strips_the_top_directory() {
        let mut raw = entry("demo-1.0.0/src/lib.rs", b"fn main() {}\n");
        raw.extend(entry("demo-1.0.0/Cargo.toml", b"[package]\n"));
        raw.extend(vec![0u8; BLOCK * 2]);
        let files = read_tar(&raw).unwrap();
        assert_eq!(files["src/lib.rs"], b"fn main() {}\n");
        assert_eq!(files["Cargo.toml"], b"[package]\n");
    }

    #[test]
    fn truncated_archive_is_an_error() {
        let mut raw = entry("demo-1.0.0/a", &[1u8; 1000]);
        raw.truncate(BLOCK + 10);
        assert!(read_tar(&raw).is_err());
    }
}
