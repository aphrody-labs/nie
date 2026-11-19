//! Writes a synthetic game directory: an encrypted `data/cpk_list.cfg.bin`, one CPK archive under
//! `data/packs/` and 64 deterministic pseudo-random files under `data/common/*` and `data/dx11/*`.
//! No game byte is read or written; boot tests (Asterinas, QEMU) mount the result as the game.
//!
//! Usage: `cargo run -p nie-formats --example synthetic_game -- <output-root>`

use std::path::PathBuf;

use nie_formats::cfgbin::{CfgEntry, Value, encode_t2b};
use nie_formats::cpk::encrypt_cpk_list;
use nie_formats::cpk_encode::{CpkWriteEntry, encode_cpk};

const FILES: usize = 64;
const PACK: &str = "synthetic.cpk";

/// xorshift64*: a fixed seed per file keeps the fixture byte-identical across runs.
fn payload(index: usize) -> Vec<u8> {
    let len = 512 + (index * 977) % 8192;
    let mut state = 0x9E37_79B9_7F4A_7C15u64 ^ (index as u64 + 1).wrapping_mul(0xD6E8_FEB8_6659_FD93);
    (0..len)
        .map(|_| {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            (state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 56) as u8
        })
        .collect()
}

fn entry(index: usize) -> CpkWriteEntry {
    let (root, group) = if index.is_multiple_of(2) {
        ("data/common", ["menu", "chara", "sound", "text"][index / 2 % 4])
    } else {
        ("data/dx11", ["model", "texture", "shader", "motion"][index / 2 % 4])
    };
    CpkWriteEntry {
        directory: format!("{root}/{group}"),
        filename: format!("file_{index:03}.bin"),
        data: payload(index),
    }
}

fn main() {
    let root = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("usage: synthetic_game <output-root>"),
    );
    let files: Vec<CpkWriteEntry> = (0..FILES).map(entry).collect();

    let packs = root.join("data/packs");
    std::fs::create_dir_all(&packs).expect("create data/packs");
    std::fs::write(packs.join(PACK), encode_cpk(&files).expect("encode cpk")).expect("write cpk");

    let children: Vec<CfgEntry> = files
        .iter()
        .map(|file| CfgEntry {
            name: "CPK_ITEM".to_owned(),
            variables: vec![
                Value::String(format!("{}/", file.directory)),
                Value::String(file.filename.clone()),
                Value::String(String::new()),
                Value::String(PACK.to_owned()),
                Value::Int(i32::try_from(file.data.len()).expect("file size fits i32")),
            ],
            children: Vec::new(),
        })
        .collect();
    let list = CfgEntry {
        name: "CPK_LIST".to_owned(),
        variables: vec![Value::Int(i32::try_from(children.len()).expect("count fits i32"))],
        children,
    };
    std::fs::write(
        root.join("data/cpk_list.cfg.bin"),
        encrypt_cpk_list(&encode_t2b(&[list])),
    )
    .expect("write cpk_list.cfg.bin");
    println!("{FILES} files in {}", root.display());
}
