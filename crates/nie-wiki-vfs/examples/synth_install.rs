//! Writes a synthetic installation (`data/cpk_list.cfg.bin` and `data/packs/*.cpk`) holding the
//! wiki test tables plus pseudo-random files, and the BLAKE3 of every logical file as read
//! back through the native VFS.
//!
//! ```text
//! cargo run -p nie-wiki-vfs --features testing --example synth_install -- <out-dir> [files]
//! ```
//!
//! `<out-dir>/install` is a folder to mount (`NIE_GAME_SOURCE=<out-dir>/install`);
//! `<out-dir>/native.blake3.json` maps each logical path to its digest, for the browser
//! differential test. No game byte is involved.

use std::collections::BTreeMap;
use std::path::PathBuf;

use nie_formats::synthetic::{SyntheticFile, installation, pseudo_random};
use nie_formats::vfs::Vfs;

fn main() {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().expect("usage: synth_install <out-dir> [files]"));
    let total: usize = args.next().map_or(50, |n| n.parse().expect("file count"));

    let scratch = out.join("scratch");
    std::fs::create_dir_all(&scratch).unwrap();
    let wiki = nie_wiki_vfs::testing::installation(&scratch);
    let mut files: Vec<SyntheticFile> = wiki
        .iter_overlays()
        .map(|(path, disk)| SyntheticFile {
            path: path.to_owned(),
            bytes: std::fs::read(disk).unwrap(),
        })
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    std::fs::remove_dir_all(&scratch).unwrap();

    let mut extra = 0usize;
    while files.len() < total {
        let len = [0, 17, 4096, 65_537, 300_001][extra % 5];
        files.push(SyntheticFile {
            path: format!("data/common/synthetic/file_{extra:03}.bin"),
            bytes: pseudo_random(extra as u64 + 1, len),
        });
        extra += 1;
    }

    let install = out.join("install");
    for (path, bytes) in installation(&files, 6).unwrap() {
        let target = install.join(&path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, bytes).unwrap();
    }

    let mut native = Vfs::new();
    native.init(install.join("data")).unwrap();
    let digests: BTreeMap<String, String> = files
        .iter()
        .map(|file| {
            let bytes = native.read(&file.path).unwrap();
            assert_eq!(bytes, file.bytes, "native read of {}", file.path);
            (file.path.clone(), blake3::hash(&bytes).to_hex().to_string())
        })
        .collect();
    std::fs::write(
        out.join("native.blake3.json"),
        serde_json::to_string_pretty(&digests).unwrap(),
    )
    .unwrap();
    println!("{} files in {}", digests.len(), install.display());
}
