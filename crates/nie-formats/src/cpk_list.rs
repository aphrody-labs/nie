//! The logical index of the installation: `data/cpk_list.cfg.bin`.
//!
//! One decoder for every host. The native [`crate::vfs::Vfs`] and the asynchronous
//! [`crate::source_vfs::SourceVfs`] (browser, remote trees) both index the game through
//! [`parse`], so a path resolves to the same pack on every platform.

use crate::FormatError;
use crate::cfgbin::{Value, cfgbin_parse};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// `/`-separated path without leading, trailing-empty or duplicate separators.
#[must_use]
pub fn canonical_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for part in path.split(['/', '\\']).filter(|p| !p.is_empty()) {
        if !out.is_empty() {
            out.push('/');
        }
        out.push_str(part);
    }
    out
}

/// Join a TOC directory and file name the one canonical way (the CPK reader, the
/// `cpk_list.cfg.bin` index and the manifest must agree, or lookups miss).
#[must_use]
pub fn join_canonical(directory: &str, filename: &str) -> String {
    if directory.is_empty() {
        return canonical_path(filename);
    }
    canonical_path(&format!("{directory}/{filename}"))
}

/// One logical file declared by the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpkListEntry {
    /// Canonical `/`-separated logical path (`data/common/...`).
    pub path: String,
    /// Name of the pack under `packs/` that stores it; empty for a loose file.
    pub pack: String,
    /// Extracted size in bytes as the index declares it.
    pub size: u32,
}

/// Decrypts and decodes the index.
///
/// Two envelopes exist: AES-256-CBC (recent Steam builds, key and IV reversed from the game
/// binary) and the fixed-key Viola envelope of older dumps. Each result is validated by the
/// cfg.bin parser, and the first valid one wins: only one of them deciphers a given file.
///
/// # Errors
/// The bytes are neither envelope.
pub fn parse(data: &[u8]) -> Result<Vec<CpkListEntry>, FormatError> {
    let cfg = crate::cpk::decrypt_cpk_list(data)
        .ok()
        .and_then(|aes| cfgbin_parse(&aes).ok())
        .or_else(|| {
            let mut viola = data.to_vec();
            crate::cpk::decrypt_block(&mut viola, 0, crate::cpk::VIOLA_FIXED_KEY);
            cfgbin_parse(&viola).ok()
        })
        .ok_or(FormatError::Corrupt(
            "echec de parsing du cpk_list.cfg.bin (ni AES-256-CBC ni clé Viola)",
        ))?;

    let mut out = Vec::new();
    for child in cfg.entries.iter().flat_map(|root| &root.children) {
        if child.variables.len() < 5 {
            continue;
        }
        let (Value::String(directory), Value::String(filename), Value::String(pack)) =
            (&child.variables[0], &child.variables[1], &child.variables[3])
        else {
            continue;
        };
        // `Value::Int` is an `i32`: a negative size is corrupt, not a 4 GiB file.
        let size = match child.variables.get(4) {
            Some(Value::Int(v)) => u32::try_from(*v).unwrap_or(0),
            _ => 0,
        };
        out.push(CpkListEntry {
            path: join_canonical(directory, filename),
            pack: pack.clone(),
            size,
        });
    }
    Ok(out)
}

/// Encodes and encrypts an index: the inverse of [`parse`], for synthetic installations.
#[must_use]
pub fn encode(entries: &[CpkListEntry]) -> Vec<u8> {
    use crate::cfgbin::{CfgEntry, encode_t2b};
    let children = entries
        .iter()
        .map(|entry| {
            let (directory, filename) = entry
                .path
                .rsplit_once('/')
                .unwrap_or(("", entry.path.as_str()));
            CfgEntry {
                name: String::from("FILE"),
                variables: alloc::vec![
                    Value::String(directory.into()),
                    Value::String(filename.into()),
                    Value::Int(0),
                    Value::String(entry.pack.clone()),
                    Value::Int(i32::try_from(entry.size).unwrap_or(i32::MAX)),
                ],
                children: Vec::new(),
            }
        })
        .collect();
    let root = CfgEntry {
        name: String::from("CPK_LIST_BEG"),
        variables: Vec::new(),
        children,
    };
    crate::cpk::encrypt_cpk_list(&encode_t2b(&[root]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_round_trips_through_the_cipher() {
        let entries = alloc::vec![
            CpkListEntry {
                path: "data/common/a/one.bin".into(),
                pack: "p0.cpk".into(),
                size: 7,
            },
            CpkListEntry {
                path: "data/dx11/two.bin".into(),
                pack: "p1.cpk".into(),
                size: 9,
            },
        ];
        assert_eq!(parse(&encode(&entries)).unwrap(), entries);
    }
}
