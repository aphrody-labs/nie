//! Coverage run of the native AOT pipeline over a directory of Lua 5.2 chunks, with a
//! differential check against the PUC-Rio oracle (mlua).
//!
//! `cargo run --release -p nie-lua --features aot,vm --example aot_corpus -- <dir> [--hosts <file>] [--rust-out <dir>] [--limit N]`
//!
//! Per chunk: lowering and compilation of every prototype, then the main chunk and every global
//! function it defines are run on both sides with recording host stubs; return values, `print`
//! output, host-call traces and error messages must match. Generated Rust (when `--rust-out` is
//! given) must stay outside the repository: it derives from game scripts.
//!
//! The report is aggregate JSON on stdout; per-chunk lines go to stderr.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use nie_lua::aot;
use nie_lua::bytecode::{self, Prototype};
use nie_lua::rt::host::{install_recording_hosts, render};
use nie_lua::rt::lstr::number_to_string;
use nie_lua::rt::state::Ctx;
use nie_lua::rt::stdlib::install_stdlib;
use nie_lua::rt::table::Table;
use nie_lua::rt::value::{Value, tag};

const NATIVE_FUEL: i64 = 400_000;
const ORACLE_INSTRUCTIONS: u32 = 20_000_000;
const ARG_SETS: &[&[&str]] = &[&[], &["1", "2", "3"], &["'a'", "'b'"]];

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn count_protos(p: &Prototype) -> usize {
    1 + p.protos.iter().map(count_protos).sum::<usize>()
}

fn fnv(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3))
}

/// One observable event; both sides produce the same vocabulary.
type Events = Vec<String>;

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

fn norm_err(msg: &str) -> String {
    msg.split("\nstack traceback").next().unwrap_or(msg).trim_end().to_string()
}

/// Outcome of running one chunk on one side.
struct Side {
    events: Events,
    budget_hit: bool,
}

fn args_values_native(ctx: &mut Ctx, set: &[&str]) -> Vec<Value> {
    set.iter()
        .map(|s| match s.parse::<f64>() {
            Ok(n) => Value::num(n),
            Err(_) => ctx.str(s.trim_matches('\'').as_bytes()),
        })
        .collect()
}

fn err_text(v: Value) -> String {
    if v.tag == tag::STR {
        // SAFETY: STR tag with a live context in the caller.
        norm_err(&String::from_utf8_lossy(unsafe { v.str_bytes() }))
    } else {
        render(v)
    }
}

fn run_native(bc: &[u8], hosts: &[String], rust_out: Option<&Path>, name: &str) -> Result<Side, String> {
    let chunk = bytecode::parse(bc).map_err(|e| format!("parse: {e}"))?;
    if let Some(dir) = rust_out {
        let mut funcs = Vec::new();
        let mut stack = vec![&chunk.main];
        while let Some(p) = stack.pop() {
            funcs.push(aot::ir::lower(p).map_err(|e| e.to_string())?);
            stack.extend(p.protos.iter().rev());
        }
        let _ = std::fs::write(dir.join(format!("{name}.rs")), aot::rustgen::render_module(&funcs, name));
    }
    let jit = aot::compile_jit(&chunk).map_err(|e| format!("compile: {e}"))?;
    let mut ctx = Ctx::new();
    install_stdlib(&mut ctx);
    install_recording_hosts(&mut ctx, hosts.iter().map(String::as_str));
    let before: BTreeSet<Vec<u8>> = global_names(&ctx);
    let main = jit.load(&mut ctx);
    ctx.fuel = NATIVE_FUEL;
    let mut events = Events::new();
    let mut budget_hit = false;
    let mut record = |ctx: &mut Ctx, label: &str, r: Result<Vec<Value>, Value>, events: &mut Events| {
        let line = match r {
            Ok(v) => format!("{label} => [{}]", v.iter().map(|x| render(*x)).collect::<Vec<_>>().join(", ")),
            Err(e) => {
                let t = err_text(e);
                if t == nie_lua::rt::FUEL_ERROR {
                    budget_hit = true;
                }
                format!("{label} !! {t}")
            }
        };
        events.push(line);
        events.append(&mut ctx.trace);
    };
    let r = ctx.run(main, &[]);
    record(&mut ctx, "main", r, &mut events);
    let mut names: Vec<(Vec<u8>, Value)> = global_entries(&ctx)
        .into_iter()
        .filter(|(k, v)| !before.contains(k) && v.tag == tag::FUNC)
        .collect();
    names.sort_by(|a, b| a.0.cmp(&b.0));
    for (k, f) in names {
        for set in ARG_SETS {
            ctx.fuel = NATIVE_FUEL;
            let args = args_values_native(&mut ctx, set);
            let r = ctx.run(f, &args);
            let label = format!("{}({})", String::from_utf8_lossy(&k), set.join(","));
            record(&mut ctx, &label, r, &mut events);
        }
    }
    events.push(format!("out={:?}", String::from_utf8_lossy(&ctx.out)));
    Ok(Side { events, budget_hit })
}

fn global_entries(ctx: &Ctx) -> Vec<(Vec<u8>, Value)> {
    let mut out = Vec::new();
    // SAFETY: globals is a live table.
    let t: &Table = unsafe { &*ctx.globals };
    let mut key = Value::NIL;
    while let Some(Some((k, v))) = t.next(key) {
        if k.tag == tag::STR {
            // SAFETY: STR tag.
            out.push((unsafe { k.str_bytes() }.to_vec(), v));
        }
        key = k;
    }
    out
}

fn global_names(ctx: &Ctx) -> BTreeSet<Vec<u8>> {
    global_entries(ctx).into_iter().map(|(k, _)| k).collect()
}

fn run_oracle(bc: &[u8], hosts: &[String]) -> Result<Side, String> {
    let lua = mlua::Lua::new();
    let events = Rc::new(RefCell::new(Events::new()));
    let out = Rc::new(RefCell::new(Vec::<u8>::new()));
    let g = lua.globals();
    for n in ["io", "os", "package", "require", "dofile", "loadfile", "load", "loadstring", "debug"] {
        let _ = g.set(n, mlua::Value::Nil);
    }
    for name in hosts {
        let ev = events.clone();
        let n = name.clone();
        let f = lua
            .create_function(move |_, args: mlua::MultiValue| {
                let parts: Vec<String> = args.iter().map(render_mlua).collect();
                ev.borrow_mut().push(format!("{n}({})", parts.join(", ")));
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        g.set(name.as_str(), f).map_err(|e| e.to_string())?;
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
            .map_err(|e| e.to_string())?;
        g.set("print", print).map_err(|e| e.to_string())?;
    }
    let budget = Rc::new(RefCell::new(false));
    let before: BTreeSet<Vec<u8>> = g
        .pairs::<mlua::Value, mlua::Value>()
        .flatten()
        .filter_map(|(k, _)| k.as_string().map(|s| s.as_bytes().to_vec()))
        .collect();
    let f = lua
        .load(bc)
        .set_mode(mlua::chunk::ChunkMode::Binary)
        .into_function()
        .map_err(|e| format!("oracle load: {e}"))?;
    let counter = Rc::new(RefCell::new(0u32));
    let install_hook = |lua: &mlua::Lua| {
        *counter.borrow_mut() = 0;
        let (c, b) = (counter.clone(), budget.clone());
        let triggers = mlua::HookTriggers::new().every_nth_instruction(1000);
        let _ = lua.set_hook(triggers, move |_, _| {
            let mut n = c.borrow_mut();
            *n += 1000;
            if *n > ORACLE_INSTRUCTIONS / 10 {
                *b.borrow_mut() = true;
                return Err(mlua::Error::runtime(nie_lua::rt::FUEL_ERROR));
            }
            Ok(mlua::VmState::Continue)
        });
    };
    let mut lines = Events::new();
    let mut record = |label: &str, r: mlua::Result<mlua::MultiValue>, lines: &mut Events| {
        let line = match r {
            Ok(v) => format!("{label} => [{}]", v.iter().map(render_mlua).collect::<Vec<_>>().join(", ")),
            Err(e) => {
                let m = match &e {
                    mlua::Error::RuntimeError(m) => m.clone(),
                    mlua::Error::CallbackError { cause, .. } => cause.to_string(),
                    o => o.to_string(),
                };
                format!("{label} !! {}", norm_err(&m))
            }
        };
        lines.push(line);
        lines.append(&mut events.borrow_mut());
    };
    install_hook(&lua);
    record("main", f.call(()), &mut lines);
    let mut funcs: Vec<(Vec<u8>, mlua::Function)> = g
        .pairs::<mlua::Value, mlua::Value>()
        .flatten()
        .filter_map(|(k, v)| match (k, v) {
            (mlua::Value::String(s), mlua::Value::Function(f)) if !before.contains(&s.as_bytes().to_vec()) => {
                Some((s.as_bytes().to_vec(), f))
            }
            _ => None,
        })
        .collect();
    funcs.sort_by(|a, b| a.0.cmp(&b.0));
    for (k, f) in funcs {
        for set in ARG_SETS {
            install_hook(&lua);
            let args: mlua::MultiValue = set
                .iter()
                .map(|s| match s.parse::<f64>() {
                    Ok(n) => mlua::Value::Number(n),
                    Err(_) => mlua::Value::String(lua.create_string(s.trim_matches('\'')).expect("string")),
                })
                .collect();
            let label = format!("{}({})", String::from_utf8_lossy(&k), set.join(","));
            record(&label, f.call(args), &mut lines);
        }
    }
    lines.push(format!("out={:?}", String::from_utf8_lossy(&out.borrow())));
    let budget_hit = *budget.borrow();
    Ok(Side { events: lines, budget_hit })
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().expect("usage: aot_corpus <dir> [--hosts file] [--rust-out dir] [--limit N]");
    let (mut hosts_file, mut rust_out, mut limit) = (None, None, usize::MAX);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--hosts" => hosts_file = args.next(),
            "--rust-out" => rust_out = args.next().map(PathBuf::from),
            "--limit" => limit = args.next().and_then(|s| s.parse().ok()).unwrap_or(usize::MAX),
            _ => {}
        }
    }
    let hosts: Vec<String> = hosts_file
        .and_then(|f| std::fs::read_to_string(f).ok())
        .map(|s| s.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect())
        .unwrap_or_default();
    if let Some(d) = &rust_out {
        let _ = std::fs::create_dir_all(d);
    }
    let mut files = Vec::new();
    walk(Path::new(&dir), &mut files);
    files.sort();
    files.truncate(limit);

    let (mut parsed, mut lowered, mut compiled, mut same, mut diff, mut budget, mut oracle_err) =
        (0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    let mut protos_total = 0usize;
    let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
    let mut failures: Vec<serde_json::Value> = Vec::new();
    for (i, file) in files.iter().enumerate() {
        let Ok(bc) = std::fs::read(file) else { continue };
        let rel = file.strip_prefix(&dir).unwrap_or(file).to_string_lossy().replace('\\', "/");
        let hash = format!("{:016x}", fnv(&bc));
        let Ok(chunk) = bytecode::parse(&bc) else {
            *reasons.entry("parse".into()).or_default() += 1;
            failures.push(serde_json::json!({"chunk": rel, "hash": hash, "stage": "parse"}));
            continue;
        };
        parsed += 1;
        protos_total += count_protos(&chunk.main);
        let stem = format!("c{i:04}");
        let native = run_native(&bc, &hosts, rust_out.as_deref(), &stem);
        let side = match native {
            Ok(s) => {
                lowered += 1;
                compiled += 1;
                s
            }
            Err(e) => {
                let stage = e.split(':').next().unwrap_or("native").to_string();
                *reasons.entry(stage.clone()).or_default() += 1;
                failures.push(serde_json::json!({"chunk": rel, "hash": hash, "stage": stage, "error": e}));
                eprintln!("[{i}] FAIL {rel}: {e}");
                continue;
            }
        };
        let oracle = match run_oracle(&bc, &hosts) {
            Ok(o) => o,
            Err(e) => {
                oracle_err += 1;
                eprintln!("[{i}] ORACLE-ERR {rel}: {e}");
                continue;
            }
        };
        if side.budget_hit || oracle.budget_hit {
            budget += 1;
            continue;
        }
        if side.events == oracle.events {
            same += 1;
        } else {
            diff += 1;
            let first = side
                .events
                .iter()
                .zip(&oracle.events)
                .position(|(a, b)| a != b)
                .unwrap_or_else(|| side.events.len().min(oracle.events.len()));
            let n = side.events.get(first).cloned().unwrap_or_default();
            let o = oracle.events.get(first).cloned().unwrap_or_default();
            let cut = |s: String| s.chars().take(300).collect::<String>();
            failures.push(serde_json::json!({
                "chunk": rel, "hash": hash, "stage": "differential", "event": first,
                "native": cut(n), "oracle": cut(o),
            }));
            eprintln!("[{i}] DIFF {rel} at event {first}");
        }
    }
    let report = serde_json::json!({
        "chunks": files.len(),
        "parsed": parsed,
        "prototypes": protos_total,
        "lowered": lowered,
        "compiled": compiled,
        "differentialSame": same,
        "differentialDiff": diff,
        "budgetExcluded": budget,
        "oracleErrors": oracle_err,
        "failureStages": reasons,
        "failures": failures,
    });
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
