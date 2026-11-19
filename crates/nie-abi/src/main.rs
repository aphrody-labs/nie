//! `nie-abi-gen [--check] <nie-ffi crate dir>` writes (or with `--check`, verifies) the files
//! generated from `contract.json`: `include/nie_contract.h`, `src/generated.rs`,
//! `bindings/nie_contract.ts`, `openapi/nie_contract.openapi.json`.
//!
//! `nie-abi-gen --crlib [--check] <workspace root>` does the same for the per-crate C ABI of
//! `crlib.json`: `include/nie_<id>.h` and `<crate>/src/abi_generated.rs`.

use nie_abi::crlib;
use nie_abi::{emit_c_header, emit_openapi, emit_rust, emit_ts, load};
use std::path::Path;

fn sync(dir: &Path, files: Vec<(std::path::PathBuf, String)>, check: bool) -> usize {
    let mut stale = 0;
    for (rel, text) in files {
        let path = dir.join(&rel);
        if check {
            if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
                eprintln!("STALE {}", rel.display());
                stale += 1;
            }
        } else {
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(&path, text).expect("write");
            eprintln!("wrote {}", rel.display());
        }
    }
    stale
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let crlib_mode = args.first().is_some_and(|a| a == "--crlib");
    if crlib_mode {
        args.remove(0);
    }
    let check = args.first().is_some_and(|a| a == "--check");
    if check {
        args.remove(0);
    }
    let dir = args.first().expect(
        "usage: nie-abi-gen [--crlib] [--check] <nie-ffi crate dir | workspace root with --crlib>",
    );
    let dir = Path::new(dir);
    let stale = if crlib_mode {
        let manifest = crlib::load_manifest(crlib::MANIFEST_JSON);
        let files = crlib::generated_files(dir, &manifest).expect("abi.json");
        sync(dir, files, check)
    } else {
        let c = load();
        let files = vec![
            ("include/nie_contract.h".into(), emit_c_header(&c)),
            ("src/generated.rs".into(), emit_rust(&c)),
            ("bindings/nie_contract.ts".into(), emit_ts(&c)),
            ("openapi/nie_contract.openapi.json".into(), emit_openapi(&c)),
        ];
        sync(dir, files, check)
    };
    if stale > 0 {
        std::process::exit(1);
    }
}
