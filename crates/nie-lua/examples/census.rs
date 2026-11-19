//! Census of a directory of Lua 5.2 chunks: counts, sizes, headers, opcodes, globals.
//!
//! `cargo run -p nie-lua --example census -- <dir> [--json]`
//!
//! Prints aggregate statistics only; no chunk content leaves the machine.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use nie_lua::bytecode::{self, OPCODE_NAMES, Prototype};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Library member usage: `GETTABUP A _ENV "lib"` followed by `GETTABLE|SELF X A "member"`.
fn members(p: &Prototype, env: bool, out: &mut BTreeMap<String, u64>) {
    let key = |rk: u32| match p.constants.get((rk & 0xff) as usize) {
        Some(bytecode::Constant::String(s)) if rk & 0x100 != 0 => {
            Some(String::from_utf8_lossy(s).into_owned())
        }
        _ => None,
    };
    if env {
        for (pc, raw) in p.code.iter().enumerate() {
            if raw & 0x3f != 6 {
                continue;
            }
            let (a, c) = ((raw >> 6) & 0xff, (raw >> 14) & 0x1ff);
            let Some(lib) = key(c) else { continue };
            if !matches!(
                lib.as_str(),
                "coroutine" | "math" | "string" | "table" | "os" | "io" | "bit32" | "debug"
            ) {
                continue;
            }
            for next in p.code.iter().skip(pc + 1).take(4) {
                let (op, b, c2) = (next & 0x3f, (next >> 23) & 0x1ff, (next >> 14) & 0x1ff);
                if (op == 7 || op == 12)
                    && b == a
                    && let Some(m) = key(c2)
                {
                    *out.entry(format!("{lib}.{m}")).or_default() += 1;
                    break;
                }
            }
        }
    }
    for child in &p.protos {
        members(child, env, out);
    }
}

fn visit(p: &Prototype, ops: &mut [u64; 64], depth: usize, max_depth: &mut usize, consts: &mut u64) {
    *max_depth = (*max_depth).max(depth);
    *consts += p.constants.len() as u64;
    for raw in &p.code {
        ops[(raw & 0x3f) as usize] += 1;
    }
    for child in &p.protos {
        visit(child, ops, depth + 1, max_depth, consts);
    }
}

fn main() {
    let dir = std::env::args().nth(1).expect("usage: census <dir>");
    let mut files = Vec::new();
    walk(Path::new(&dir), &mut files);
    files.sort();

    let (mut chunks, mut bytes, mut failures) = (0u64, 0u64, 0u64);
    let mut headers: BTreeMap<String, u64> = BTreeMap::new();
    let mut ops = [0u64; 64];
    let (mut protos, mut insns, mut consts, mut max_depth) = (0u64, 0u64, 0u64, 0usize);
    let (mut stripped, mut with_debug) = (0u64, 0u64);
    let (mut vararg_protos, mut max_stack, mut max_insns) = (0u64, 0u8, 0usize);
    let mut globals: BTreeMap<String, u64> = BTreeMap::new();
    let mut libs: BTreeMap<String, u64> = BTreeMap::new();
    let mut sizes: Vec<usize> = Vec::new();
    let mut errors: BTreeMap<String, u64> = BTreeMap::new();

    for file in &files {
        let Ok(data) = std::fs::read(file) else {
            continue;
        };
        bytes += data.len() as u64;
        sizes.push(data.len());
        match bytecode::parse(&data) {
            Ok(chunk) => {
                chunks += 1;
                let h = &chunk.header;
                *headers
                    .entry(format!(
                        "v{:#x} fmt{} le={} int={} size_t={} insn={} num={} integral={}",
                        h.version,
                        h.format,
                        h.little_endian,
                        h.size_int,
                        h.size_size_t,
                        h.size_instruction,
                        h.size_number,
                        h.number_is_integral
                    ))
                    .or_default() += 1;
                protos += 1 + chunk.main.total_protos() as u64;
                insns += chunk.main.total_instructions() as u64;
                max_insns = max_insns.max(chunk.main.total_instructions());
                if chunk.main.line_info.is_empty() {
                    stripped += 1;
                } else {
                    with_debug += 1;
                }
                visit(&chunk.main, &mut ops, 0, &mut max_depth, &mut consts);
                let mut stack = vec![&chunk.main];
                while let Some(p) = stack.pop() {
                    vararg_protos += u64::from(p.is_vararg != 0);
                    max_stack = max_stack.max(p.max_stack_size);
                    stack.extend(p.protos.iter());
                }
                members(&chunk.main, true, &mut libs);
                for read in bytecode::global_reads(&chunk) {
                    *globals.entry(read.name_utf8().into_owned()).or_default() += 1;
                }
            }
            Err(error) => {
                failures += 1;
                *errors.entry(error.to_string()).or_default() += 1;
            }
        }
    }

    sizes.sort_unstable();
    let median = sizes.get(sizes.len() / 2).copied().unwrap_or(0);
    let opcodes: BTreeMap<&str, u64> = OPCODE_NAMES
        .iter()
        .enumerate()
        .map(|(i, name)| (*name, ops[i]))
        .collect();
    let unknown_ops: u64 = ops[OPCODE_NAMES.len()..].iter().sum();
    let report = serde_json::json!({
        "files": files.len(),
        "chunks": chunks,
        "parseFailures": failures,
        "parseErrors": errors,
        "bytes": bytes,
        "medianBytes": median,
        "maxBytes": sizes.last().copied().unwrap_or(0),
        "headers": headers,
        "protos": protos,
        "instructions": insns,
        "maxChunkInstructions": max_insns,
        "constants": consts,
        "maxNesting": max_depth,
        "maxStackSize": max_stack,
        "varargProtos": vararg_protos,
        "stripped": stripped,
        "withDebug": with_debug,
        "unknownOpcodeInstructions": unknown_ops,
        "opcodes": opcodes,
        "libraryMembers": libs,
        "distinctGlobals": globals.len(),
        "globals": globals,
    });
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
