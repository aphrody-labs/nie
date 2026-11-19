//! The browser app of nie, embedded in the binary.
//!
//! `cargo xtask app-pack` builds the app (the `nie-wasm` module with its WebGPU viewer, the
//! `wasm-bindgen` glue, the page) into content-hashed files; this crate carries them with
//! `include_bytes!` so `nie serve` ships the server and the client as one artifact and needs
//! no other file, no Node and no CDN.
//!
//! Every file but the entry page has the BLAKE3 prefix of its content in its name and is served
//! `immutable`; the entry page names those files and is revalidated on every load.

#![forbid(unsafe_code)]

/// One embedded file.
#[derive(Debug, Clone, Copy)]
pub struct Asset {
    /// File name, flat (`app.3f2a9c1d.js`).
    pub name: &'static str,
    /// Media type.
    pub content_type: &'static str,
    /// True when the name carries the content hash, so the file never changes.
    pub immutable: bool,
    /// The file.
    pub bytes: &'static [u8],
    /// The same file compressed with brotli, when it pays.
    pub brotli: Option<&'static [u8]>,
}

include!(concat!(env!("OUT_DIR"), "/bundle.rs"));

/// True when a bundle is embedded.
#[must_use]
pub fn is_embedded() -> bool {
    !ENTRY.is_empty()
}

/// The entry page, when a bundle is embedded.
#[must_use]
pub fn entry() -> Option<&'static Asset> {
    get(ENTRY)
}

/// The file called `name`.
#[must_use]
pub fn get(name: &str) -> Option<&'static Asset> {
    FILES.iter().find(|asset| asset.name == name)
}

/// Every embedded file.
#[must_use]
pub fn files() -> &'static [Asset] {
    FILES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bundle_names_its_entry_and_only_files_it_holds() {
        if is_embedded() {
            assert!(entry().is_some());
        }
        for asset in files() {
            assert!(!asset.bytes.is_empty() || asset.name.ends_with(".map"));
            assert!(!asset.name.contains('/'));
        }
    }
}
