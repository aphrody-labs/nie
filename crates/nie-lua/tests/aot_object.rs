//! Ahead-of-time artifact test: every fixture is compiled to a relocatable object, linked into a
//! shared library with the system linker, loaded back and run. The result must equal the
//! in-process JIT run. Unix only: it needs `cc`.
#![cfg(all(feature = "aot", feature = "vm", unix))]

use std::path::PathBuf;
use std::process::Command;

use nie_lua::aot;
use nie_lua::bytecode;
use nie_lua::rt::host::{install_recording_hosts, render};
use nie_lua::rt::state::{Ctx, NativeFn};
use nie_lua::rt::stdlib::install_stdlib;

include!("aot_fixtures.in");

const HOSTS: &[&str] = &["HostA", "HostB", "SetSoccerParam", "WaitFrame", "OpenWindow"];

fn run_main(ctx: &mut Ctx, f: nie_lua::rt::value::Value) -> String {
    match ctx.run(f, &[]) {
        Ok(v) => format!(
            "ok {:?} out={:?} trace={:?}",
            v.into_iter().map(render).collect::<Vec<_>>(),
            String::from_utf8_lossy(&ctx.out),
            ctx.trace
        ),
        Err(e) => {
            // SAFETY: error values of these fixtures are strings alive in `ctx`.
            let t = if e.tag == nie_lua::rt::value::tag::STR {
                String::from_utf8_lossy(unsafe { e.str_bytes() }).into_owned()
            } else {
                render(e)
            };
            format!("err {t} out={:?} trace={:?}", String::from_utf8_lossy(&ctx.out), ctx.trace)
        }
    }
}

fn fresh() -> Box<Ctx> {
    let mut ctx = Ctx::new();
    install_stdlib(&mut ctx);
    install_recording_hosts(&mut ctx, HOSTS.iter().copied());
    ctx
}

#[test]
fn shared_objects_match_the_jit() {
    let dir = std::env::temp_dir().join(format!("nie-lua-aot-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let lua = mlua::Lua::new();
    let mut failures = Vec::new();
    for (i, (name, src)) in FIXTURES.iter().enumerate() {
        let bc = lua.load(*src).set_name("=fixture").into_function().expect("compiles").dump(true);
        let mut chunk = bytecode::parse(&bc).expect("parse");
        bytecode::strip_debug(&mut chunk);

        let jit = aot::compile_jit(&chunk).expect("jit");
        let mut ctx = fresh();
        let f = jit.load(&mut ctx);
        let expected = run_main(&mut ctx, f);

        let art = aot::compile_object(&chunk, &format!("fx{i}")).expect("object");
        let obj: PathBuf = dir.join(format!("fx{i}.o"));
        let so: PathBuf = dir.join(format!("fx{i}.so"));
        std::fs::write(&obj, &art.object).expect("write object");
        let status = Command::new("cc")
            .args(["-shared", "-o"])
            .arg(&so)
            .arg(&obj)
            .arg("-lm")
            .status()
            .expect("cc");
        assert!(status.success(), "link failed for {name}");
        // SAFETY: loading the library we just linked from our own object.
        let lib = unsafe { libloading::Library::new(&so) }.expect("dlopen");
        // SAFETY: symbols exported by `compile_object`.
        let (funcs, n, desc, desc_len, sites, sites_len) = unsafe {
            let sym = |n: &[u8]| -> *const u8 { *lib.get::<*const u8>(n).expect("symbol") };
            (
                sym(b"nie_lua_funcs\0").cast::<NativeFn>(),
                *sym(b"nie_lua_nfuncs\0").cast::<u64>() as usize,
                sym(b"nie_lua_desc\0"),
                *sym(b"nie_lua_desc_len\0").cast::<u64>() as usize,
                sym(b"nie_lua_sites\0"),
                *sym(b"nie_lua_sites_len\0").cast::<u64>() as usize,
            )
        };
        assert_eq!(n, art.functions);
        // SAFETY: the blobs are `desc_len` / `sites_len` bytes of read-only data.
        let (desc, sites) = unsafe {
            (
                std::slice::from_raw_parts(desc, desc_len),
                std::slice::from_raw_parts(sites, sites_len),
            )
        };
        let stripped = bytecode::parse(desc).expect("descriptor");
        let sites = aot::decode_sites(sites).expect("sites");
        // SAFETY: `n` entries of the exported table.
        let table: Vec<NativeFn> = (0..n).map(|k| unsafe { *funcs.add(k) }).collect();
        let mut ctx = fresh();
        let f = aot::load_image(&mut ctx, &stripped, &table, &sites);
        let got = run_main(&mut ctx, f);
        if got != expected {
            failures.push(format!("--- {name}\nexpected: {expected}\n     got: {got}"));
        }
        drop(lib);
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{} fixtures differ:\n{}", failures.len(), failures.join("\n"));
}
