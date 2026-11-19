//! A synthetic installation: `data/cpk_list.cfg.bin` and `data/packs/*.cpk` built from bytes the
//! caller owns. Tests, fixtures and the browser self-test mount it exactly like the game's; no
//! game byte is involved.
//!
//! The packs are enciphered with the key their file name derives (the game's envelope) and
//! stored uncompressed.

use crate::cpk::{decrypt_block, key_from_filename};
use crate::cpk_encode::{CpkWriteEntry, encode_cpk};
use crate::cpk_list::{CpkListEntry, encode};
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// One logical file of a synthetic installation.
#[derive(Debug, Clone)]
pub struct SyntheticFile {
    /// Logical path (`data/common/...`).
    pub path: String,
    /// Content.
    pub bytes: Vec<u8>,
}

/// Builds the installation files, keyed by their path relative to the installation root.
///
/// Files are dealt round-robin to `packs` packs (at least one); a file whose path starts with
/// `data/loose/` is declared with an empty pack and stored loose, as the game's intro videos are.
///
/// # Errors
/// A pack cannot be encoded.
pub fn installation(
    files: &[SyntheticFile],
    packs: usize,
) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let packs = packs.max(1);
    let mut out = BTreeMap::new();
    let mut per_pack: Vec<Vec<CpkWriteEntry>> = (0..packs).map(|_| Vec::new()).collect();
    let mut list = Vec::with_capacity(files.len());
    let mut dealt = 0usize;
    for file in files {
        let size = u32::try_from(file.bytes.len()).map_err(|_| "file over 4 GiB".to_owned())?;
        if let Some(relative) = file.path.strip_prefix("data/loose/") {
            out.insert(format!("data/loose/{relative}"), file.bytes.clone());
            list.push(CpkListEntry {
                path: file.path.clone(),
                pack: String::new(),
                size,
            });
            continue;
        }
        let slot = dealt % packs;
        dealt += 1;
        let (directory, filename) = file
            .path
            .rsplit_once('/')
            .map_or(("", file.path.as_str()), |(d, f)| (d, f));
        per_pack[slot].push(CpkWriteEntry {
            filename: filename.into(),
            directory: directory.into(),
            data: file.bytes.clone(),
        });
        list.push(CpkListEntry {
            path: file.path.clone(),
            pack: pack_name(slot),
            size,
        });
    }
    for (slot, entries) in per_pack.iter().enumerate() {
        if entries.is_empty() {
            continue;
        }
        let name = pack_name(slot);
        let mut bytes = encode_cpk(entries)?;
        decrypt_block(&mut bytes, 0, key_from_filename(&name));
        out.insert(format!("data/packs/{name}"), bytes);
    }
    out.insert("data/cpk_list.cfg.bin".into(), encode(&list));
    Ok(out)
}

fn pack_name(slot: usize) -> String {
    format!("synthetic_{slot:03}.cpk")
}

/// Deterministic pseudo-random bytes (xorshift64*), for fixtures of any size.
#[must_use]
pub fn pseudo_random(seed: u64, len: usize) -> Vec<u8> {
    let mut state = seed | 1;
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        let word = state.wrapping_mul(0x2545_F491_4F6C_DD1D);
        out.extend_from_slice(&word.to_le_bytes());
    }
    out.truncate(len);
    out
}

/// `count` files over a few folders, sizes from empty to a few hundred KiB.
#[must_use]
pub fn sample_files(count: usize) -> Vec<SyntheticFile> {
    const FOLDERS: [&str; 4] = [
        "data/common/menu",
        "data/common/text/en",
        "data/dx11/model",
        "data/common/gamedata",
    ];
    (0..count)
        .map(|i| {
            let len = match i % 5 {
                0 => 0,
                1 => 17,
                2 => 4_096,
                3 => 65_537,
                _ => 300_001,
            };
            let path = if i % 11 == 10 {
                format!("data/loose/clip_{i:03}.usm")
            } else {
                format!("{}/file_{i:03}.bin", FOLDERS[i % FOLDERS.len()])
            };
            SyntheticFile {
                path,
                bytes: pseudo_random(i as u64 + 1, len),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::byte_source::{FsTree, MemoryTree};
    use crate::source_vfs::SourceVfs;
    use crate::vfs::Vfs;
    use crate::vfs_read::VfsRead;
    use futures_lite::future::block_on;

    #[test]
    fn async_mount_equals_native_mount_on_every_file() {
        let files = sample_files(50);
        let installed = installation(&files, 4).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut tree = MemoryTree::new();
        for (path, bytes) in &installed {
            let target = dir.path().join(path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, bytes).unwrap();
            tree.insert(path.clone(), bytes.clone());
        }

        let mut native = Vfs::new();
        native.init(dir.path().join("data")).unwrap();
        let from_memory = block_on(SourceVfs::mount(tree)).unwrap();
        let from_disk = block_on(SourceVfs::mount(FsTree::new(dir.path()))).unwrap();
        assert_eq!(from_memory.len(), 50);
        assert_eq!(from_memory.pack_count(), 4);

        for file in &files {
            let expect = blake3_free(&file.bytes);
            let native_bytes = native.read(&file.path).unwrap();
            assert_eq!(native_bytes, file.bytes, "native {}", file.path);
            assert_eq!(blake3_free(&native_bytes), expect);
            assert_eq!(block_on(from_memory.read(&file.path)).unwrap(), file.bytes);
            assert_eq!(block_on(from_disk.read(&file.path)).unwrap(), file.bytes);
        }
    }

    #[test]
    fn run_fetches_what_the_decoder_asks_for() {
        let files = sample_files(12);
        let mut tree = MemoryTree::new();
        for (path, bytes) in installation(&files, 2).unwrap() {
            tree.insert(path, bytes);
        }
        let vfs = block_on(SourceVfs::mount(tree)).unwrap();
        let memory = vfs.memory();
        let total = block_on(vfs.run(&memory, |m| {
            files
                .iter()
                .filter(|f| f.path.contains("/menu/") || f.path.contains("/text/"))
                .map(|f| m.read(&f.path).map_or(0, |b| b.len()))
                .sum::<usize>()
        }));
        let expect: usize = files
            .iter()
            .filter(|f| f.path.contains("/menu/") || f.path.contains("/text/"))
            .map(|f| f.bytes.len())
            .sum();
        assert_eq!(total, expect);
        assert!(memory.loaded() > 0);
        // An unknown path is absent, not an endless loop.
        let missing = block_on(vfs.run(&memory, |m| m.read("data/none").is_err()));
        assert!(missing);
    }

    /// Length-and-sum digest: enough to compare two reads without a hash dependency here.
    fn blake3_free(bytes: &[u8]) -> (usize, u64) {
        (
            bytes.len(),
            bytes
                .iter()
                .fold(0u64, |a, b| a.wrapping_mul(31).wrapping_add(u64::from(*b))),
        )
    }
}
