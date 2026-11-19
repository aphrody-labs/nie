//! nie-link: the game installation as content-addressed pages between hosts.
//!
//! * [`scan`] turns an installation into a [`proto::Manifest`]: every file cut into fixed-size pages
//!   named by their BLAKE3 hash, immutable per Steam build id.
//! * [`server`] serves pages and the manifest over HTTP (`origin` on the host that owns the bytes,
//!   `cache` as a read-through tier); [`client::Router`] sends each page request to the host that
//!   holds it, the fastest first.
//! * [`link::Link`] is the client engine: manifest, verified on-disk [`store::PageStore`], routing,
//!   read-ahead and an access trace. On Windows `projfs` presents it as a real NTFS folder.
//! * [`verify`] and [`bench`] prove read integrity and measure cold and warm reads.

pub mod bench;
pub mod client;
pub mod config;
pub mod http;
pub mod hydrate;
pub mod link;
pub mod proto;
pub mod scan;
pub mod server;
pub mod steam;
pub mod store;
pub mod verify;

#[cfg(windows)]
pub mod projfs;
#[cfg(windows)]
pub mod startup;

mod fsio;

pub use proto::{Manifest, PageId};

/// Lower-case hex of a byte slice.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// Parse lower- or upper-case hex into exactly `N` bytes.
#[must_use]
pub fn unhex<const N: usize>(text: &str) -> Option<[u8; N]> {
    let text = text.as_bytes();
    if text.len() != N * 2 {
        return None;
    }
    let nibble = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    };
    let mut out = [0u8; N];
    for (i, pair) in text.as_chunks::<2>().0.iter().enumerate() {
        out[i] = nibble(pair[0])? << 4 | nibble(pair[1])?;
    }
    Some(out)
}
