//! A synthetic game for the tests of the explorer: a temporary directory laid out as an extracted
//! `data/` tree, with generated G4TX, cfg.bin, event and Lua files. No game byte is involved.

use std::path::Path;
use std::sync::Arc;

use nie_formats::cfgbin::{CfgEntry, Value, encode_t2b};
use nie_formats::g4tx_encode::{
    RegionAEcrire, TextureAEcrire, encode_dds_bgra8, encode_g4tx_multi_texture,
};
use nie_formats::vfs::Vfs;
use nie_lua::bytecode::{self, Chunk, Constant, Header, Prototype};

/// The generated game. The directory lives as long as the value.
pub struct Fixture {
    /// The temporary directory.
    pub dir: tempfile::TempDir,
    /// The mounted loose game.
    pub vfs: Arc<Vfs>,
}

fn write(root: &Path, path: &str, bytes: &[u8]) {
    let full = root.join(path);
    std::fs::create_dir_all(full.parent().expect("a parent")).expect("create directory");
    std::fs::write(full, bytes).expect("write fixture file");
}

fn entry(name: &str, variables: Vec<Value>) -> CfgEntry {
    CfgEntry {
        name: name.to_owned(),
        variables,
        children: Vec::new(),
    }
}

/// A `size` x `size` picture whose colour follows the position, as BGRA8 DDS.
fn gradient_dds(size: u32) -> Vec<u8> {
    let mut rgba = Vec::new();
    for y in 0..size {
        for x in 0..size {
            rgba.extend_from_slice(&[(x * 255 / size) as u8, (y * 255 / size) as u8, 128, 255]);
        }
    }
    encode_dds_bgra8(size, size, &rgba).expect("dds")
}

fn lua_chunk() -> Vec<u8> {
    let code = vec![1, 31 | (1 << 23)];
    let main = Prototype {
        line_defined: 0,
        last_line_defined: 0,
        num_params: 0,
        is_vararg: 2,
        max_stack_size: 2,
        code,
        constants: vec![Constant::String(b"hello".to_vec())],
        protos: Vec::new(),
        upvalues: Vec::new(),
        source: String::new(),
        line_info: Vec::new(),
        loc_vars: Vec::new(),
        upvalue_names: Vec::new(),
    };
    bytecode::encode(&Chunk {
        header: Header {
            version: 0x52,
            format: 0,
            little_endian: true,
            size_int: 4,
            size_size_t: 8,
            size_instruction: 4,
            size_number: 8,
            number_is_integral: false,
        },
        main,
    })
}

/// Builds the game and mounts it.
///
/// # Panics
/// On an I/O error in the temporary directory.
#[must_use]
pub fn game() -> Fixture {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().join("data");

    let words = |hello: &str| {
        encode_t2b(&[
            entry("WORD_INFO_0", vec![Value::Int(1), Value::String(hello.to_owned())]),
            entry("WORD_INFO_1", vec![Value::Int(2), Value::String("goal".to_owned())]),
            entry("WORD_LIST_0", vec![Value::Float(0.5)]),
        ])
    };
    write(&root, "common/text/en/words.cfg.bin", &words("hello"));
    write(&root, "common/text/fr/words.cfg.bin", &words("bonjour"));
    write(
        &root,
        "common/menu/ui_main.cfg.bin",
        &encode_t2b(&[
            entry("UI_ITEM_0", vec![Value::Int(10), Value::String("play".into())]),
            entry("UI_ITEM_1", vec![Value::Int(20), Value::String("quit".into())]),
        ]),
    );
    write(
        &root,
        "common/event_cfg/evt/ev_demo.cfg.bin",
        &encode_t2b(&[
            entry("EVENT_COMMAND_NUM_0", vec![Value::Int(2)]),
            entry(
                "EVENT_COMMAND_HEADER_0",
                vec![Value::Int(0), Value::Int(0x0FD0_4302), Value::Int(1)],
            ),
            entry("EVENT_COMMAND_ARGS_0", vec![Value::String("cut_01".into())]),
            entry(
                "EVENT_COMMAND_HEADER_1",
                vec![Value::Int(30), Value::Int(1000), Value::Int(1000)],
            ),
            entry("EVENT_COMMAND_ARGS_1", vec![]),
        ]),
    );
    write(&root, "common/script/intro.lua.bin", &lua_chunk());
    write(&root, "common/misc/readme.txt", b"Synthetic game.\nSecond line.\n");
    write(&root, "common/misc/blob.bin", &[0x00, 0x01, 0xfe, 0xff, 0x10, 0x20, 0x30, 0x40]);

    let big = gradient_dds(32);
    let small = gradient_dds(16);
    let atlas = encode_g4tx_multi_texture(
        &[TextureAEcrire {
            name: "icons",
            id: 0,
            width: 32,
            height: 32,
            dds: &big,
        }],
        &[
            RegionAEcrire {
                entry_index: 0,
                name: "icon_a",
                id: 1,
                x: 0,
                y: 0,
                width: 16,
                height: 16,
            },
            RegionAEcrire {
                entry_index: 0,
                name: "icon_b",
                id: 2,
                x: 16,
                y: 16,
                width: 16,
                height: 16,
            },
        ],
    )
    .expect("atlas");
    write(&root, "dx11/menu/icons.g4tx", &atlas);
    let plain = encode_g4tx_multi_texture(
        &[
            TextureAEcrire {
                name: "plain",
                id: 0,
                width: 16,
                height: 16,
                dds: &small,
            },
            TextureAEcrire {
                name: "plain_b",
                id: 1,
                width: 32,
                height: 32,
                dds: &big,
            },
        ],
        &[],
    )
    .expect("textures");
    write(&root, "dx11/menu/plain.g4tx", &plain);

    let mut vfs = Vfs::new();
    vfs.init_loose(&root).expect("mount the loose game");
    Fixture {
        dir,
        vfs: Arc::new(vfs),
    }
}
