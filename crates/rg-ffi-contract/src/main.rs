//! `rg-ffi-gen [--check] <rg-ffi crate dir>`: writes (or with `--check`, verifies) the generated
//! files: `include/rg_ffi.h`, `src/generated.rs`, `bindings/rg_ffi.ts`, `openapi/rg_ffi.openapi.json`.

use rg_ffi_contract::{emit_c_header, emit_openapi, emit_rust, emit_ts, load};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let check = args.first().is_some_and(|a| a == "--check");
    if check {
        args.remove(0);
    }
    let dir = args
        .first()
        .expect("usage: rg-ffi-gen [--check] <rg-ffi crate dir>");
    let c = load();
    let files = [
        ("include/rg_ffi.h", emit_c_header(&c)),
        ("src/generated.rs", emit_rust(&c)),
        ("bindings/rg_ffi.ts", emit_ts(&c)),
        ("openapi/rg_ffi.openapi.json", emit_openapi(&c)),
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
