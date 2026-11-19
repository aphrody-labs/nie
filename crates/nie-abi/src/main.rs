//! `nie-abi-gen [--check] <nie-ffi crate dir>`: writes (or with `--check`, verifies) the generated
//! files: `include/nie_contract.h`, `src/generated.rs`, `bindings/nie_contract.ts`, `openapi/nie_contract.openapi.json`.

use nie_abi::{emit_c_header, emit_openapi, emit_rust, emit_ts, load};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let check = args.first().is_some_and(|a| a == "--check");
    if check {
        args.remove(0);
    }
    let dir = args
        .first()
        .expect("usage: nie-abi-gen [--check] <nie-ffi crate dir>");
    let c = load();
    let files = [
        ("include/nie_contract.h", emit_c_header(&c)),
        ("src/generated.rs", emit_rust(&c)),
        ("bindings/nie_contract.ts", emit_ts(&c)),
        ("openapi/nie_contract.openapi.json", emit_openapi(&c)),
    ];
    let mut stale = 0;
    for (rel, text) in files {
        let path = std::path::Path::new(dir).join(rel);
        if check {
            if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
                eprintln!("STALE {rel}");
                stale += 1;
            }
        } else {
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(&path, text).expect("write");
            eprintln!("wrote {rel}");
        }
    }
    if stale > 0 {
        std::process::exit(1);
    }
}
