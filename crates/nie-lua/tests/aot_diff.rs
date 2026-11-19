//! Differential test: every synthetic fixture runs through the native AOT pipeline and through
//! the PUC-Rio Lua 5.2.4 oracle (mlua), from the same stripped bytecode. Return values, `print`
//! output, host-call traces and error messages must be identical.
#![cfg(all(feature = "aot", feature = "vm", not(target_arch = "wasm32")))]

use std::cell::RefCell;
use std::rc::Rc;

use nie_lua::aot;
use nie_lua::bytecode;
use nie_lua::rt::host::{install_recording_hosts, render};
use nie_lua::rt::lstr::number_to_string;
use nie_lua::rt::state::Ctx;
use nie_lua::rt::stdlib::install_stdlib;

/// Names registered as recording host stubs on both sides.
const HOSTS: &[&str] = &["HostA", "HostB", "SetSoccerParam", "WaitFrame", "OpenWindow"];

#[derive(Debug, PartialEq)]
struct Outcome {
    rets: Vec<String>,
    out: String,
    trace: Vec<String>,
    err: Option<String>,
}

fn strip_position(msg: &str) -> String {
    let msg = msg.split("\nstack traceback").next().unwrap_or(msg);
    msg.trim_end().to_string()
}

fn oracle_bytecode(lua: &mlua::Lua, src: &str) -> Vec<u8> {
    let bc = lua
        .load(src)
        .set_name("=fixture")
        .into_function()
        .expect("fixture compiles")
        .dump(true);
    // Lua 5.2 cannot strip on dump: strip like `luac -s`, as the game's chunks are.
    let mut chunk = bytecode::parse(&bc).expect("parse");
    bytecode::strip_debug(&mut chunk);
    bytecode::encode(&chunk)
}

fn render_mlua(v: &mlua::Value) -> String {
    match v {
        mlua::Value::Nil => "nil".into(),
        mlua::Value::Boolean(b) => b.to_string(),
        mlua::Value::Integer(i) => String::from_utf8_lossy(&number_to_string(*i as f64)).into_owned(),
        mlua::Value::Number(n) => String::from_utf8_lossy(&number_to_string(*n)).into_owned(),
        mlua::Value::String(s) => format!("{:?}", String::from_utf8_lossy(&s.as_bytes())),
        mlua::Value::Table(_) => "table".into(),
        mlua::Value::Function(_) => "function".into(),
        mlua::Value::Thread(_) => "thread".into(),
        _ => "userdata".into(),
    }
}

fn run_oracle(bc: &[u8]) -> Outcome {
    let lua = mlua::Lua::new();
    let trace = Rc::new(RefCell::new(Vec::<String>::new()));
    let out = Rc::new(RefCell::new(Vec::<u8>::new()));
    let g = lua.globals();
    for name in ["io", "os", "package", "require", "dofile", "loadfile", "load", "loadstring", "debug", "string_dump"] {
        let _ = g.set(name, mlua::Value::Nil);
    }
    for name in HOSTS {
        let t = trace.clone();
        let n = (*name).to_string();
        let f = lua
            .create_function(move |_, args: mlua::MultiValue| {
                let parts: Vec<String> = args.iter().map(render_mlua).collect();
                t.borrow_mut().push(format!("{n}({})", parts.join(", ")));
                Ok(())
            })
            .expect("host stub");
        g.set(*name, f).expect("set host");
    }
    {
        let o = out.clone();
        let print = lua
            .create_function(move |lua, args: mlua::MultiValue| {
                let tostring: mlua::Function = lua.globals().get("tostring")?;
                let mut buf = o.borrow_mut();
                for (i, a) in args.into_iter().enumerate() {
                    if i > 0 {
                        buf.push(b'\t');
                    }
                    let s: mlua::LuaString = tostring.call(a)?;
                    buf.extend_from_slice(&s.as_bytes());
                }
                buf.push(b'\n');
                Ok(())
            })
            .expect("print");
        g.set("print", print).expect("set print");
    }
    let f = lua
        .load(bc)
        .set_mode(mlua::chunk::ChunkMode::Binary)
        .into_function()
        .expect("oracle loads bytecode");
    let r: mlua::Result<mlua::MultiValue> = f.call(());
    let (rets, err) = match r {
        Ok(vals) => (vals.iter().map(render_mlua).collect(), None),
        Err(e) => {
            let msg = match &e {
                mlua::Error::RuntimeError(m) => m.clone(),
                mlua::Error::CallbackError { cause, .. } => cause.to_string(),
                other => other.to_string(),
            };
            (Vec::new(), Some(strip_position(&msg)))
        }
    };
    let out = String::from_utf8_lossy(&out.borrow()).into_owned();
    let trace = trace.borrow().clone();
    Outcome { rets, out, trace, err }
}

fn run_native(bc: &[u8]) -> Outcome {
    let chunk = bytecode::parse(bc).expect("parse");
    let jit = aot::compile_jit(&chunk).expect("compile");
    let mut ctx = Ctx::new();
    install_stdlib(&mut ctx);
    install_recording_hosts(&mut ctx, HOSTS.iter().copied());
    let f = jit.load(&mut ctx);
    let r = ctx.run(f, &[]);
    let (rets, err) = match r {
        Ok(vals) => (vals.into_iter().map(render).collect(), None),
        Err(e) => (Vec::new(), Some(strip_position(&render_err(e)))),
    };
    Outcome {
        rets,
        out: String::from_utf8_lossy(&ctx.out).into_owned(),
        trace: ctx.trace.clone(),
        err,
    }
}

fn render_err(v: nie_lua::rt::value::Value) -> String {
    if v.tag == nie_lua::rt::value::tag::STR {
        // SAFETY: STR tag; the context is still alive in the caller.
        String::from_utf8_lossy(unsafe { v.str_bytes() }).into_owned()
    } else {
        render(v)
    }
}

/// Runs a source fixture on both sides and returns the two outcomes.
fn both(src: &str) -> (Outcome, Outcome) {
    let lua = mlua::Lua::new();
    let bc = oracle_bytecode(&lua, src);
    (run_native(&bc), run_oracle(&bc))
}

include!("aot_fixtures.in");

#[test]
fn fixtures_match_the_oracle() {
    let mut failures = Vec::new();
    for (name, src) in FIXTURES {
        if std::env::var("NIE_LUA_ONLY").is_ok_and(|o| o != *name) {
            continue;
        }
        if std::env::var_os("NIE_LUA_TRACE").is_some() {
            eprintln!("fixture {name}");
        }
        let (native, oracle) = both(src);
        if native != oracle {
            failures.push(format!("--- {name}\nnative: {native:#?}\noracle: {oracle:#?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} fixtures differ:\n{}",
        failures.len(),
        FIXTURES.len(),
        failures.join("\n")
    );
}
