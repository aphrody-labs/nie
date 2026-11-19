//! Rust source backend: renders the typed IR as a reviewable Rust module.
//!
//! Each prototype becomes an `unsafe extern "C"` function with the same signature, frame layout
//! and helper calls as the Cranelift backend; control flow is a `loop`/`match` over block
//! numbers. The output compiles against `nie_lua::rt`.

use std::fmt::Write as _;

use super::ir::{Cond, Count, Func, Op, Opnd, Term};
use crate::bytecode::Constant;
use crate::rt::state::arith;

fn reg(r: u8) -> String {
    format!("base.add({r})")
}

fn opnd(o: Opnd) -> String {
    match o {
        Opnd::R(r) => reg(r),
        Opnd::K(k) => format!("k.add({k})"),
    }
}

fn num_of(f: &Func, o: Opnd) -> String {
    if let Opnd::K(k) = o {
        if let Constant::Number(n) = f.consts[k as usize] {
            return format!("f64::from_bits({:#x}_u64)", n.to_bits());
        }
    }
    format!("(*{}).as_num()", opnd(o))
}

fn both_num(f: &Func, b: Opnd, c: Opnd) -> String {
    let is = |o: Opnd| -> Option<String> {
        if let Opnd::K(k) = o {
            if matches!(f.consts[k as usize], Constant::Number(_)) {
                return None;
            }
        }
        Some(format!("(*{}).tag == tag::NUM", opnd(o)))
    };
    match (is(b), is(c)) {
        (None, None) => "true".to_string(),
        (Some(x), None) | (None, Some(x)) => x,
        (Some(x), Some(y)) => format!("{x} && {y}"),
    }
}

fn count_expr(c: Count, from: &str) -> String {
    match c {
        Count::Fixed(n) => format!("{n}_u32"),
        Count::ToTop => format!("(top.offset_from({from}) as u32)"),
    }
}

fn op(f: &Func, o: &Op, pc: u32, out: &mut String) {
    let site = format!("{{ rt_annotate(ctx, cl, {pc}); return -1; }}");
    let err = site.as_str();
    match *o {
        Op::Move { a, b } => {
            let _ = writeln!(out, "        *{} = *{};", reg(a), reg(b));
        }
        Op::LoadK { a, k } => {
            let _ = writeln!(out, "        *{} = *k.add({k});", reg(a));
        }
        Op::LoadBool { a, v } => {
            let _ = writeln!(out, "        *{} = Value::bool({v});", reg(a));
        }
        Op::LoadNil { a, n } => {
            for i in 0..=n {
                let _ = writeln!(out, "        *{} = Value::NIL;", reg(a + i as u8));
            }
        }
        Op::GetUpval { a, u } => {
            let _ = writeln!(out, "        *{} = *(**(*cl).upvals.add({u})).v;", reg(a));
        }
        Op::SetUpval { a, u } => {
            let _ = writeln!(out, "        *(**(*cl).upvals.add({u})).v = *{};", reg(a));
        }
        Op::GetTabUp { a, u, key } => {
            let _ = writeln!(
                out,
                "        if rt_gettabup(ctx, ftop, cl, {u}, {}, {}) != 0 {{ {err} }}",
                opnd(key),
                reg(a)
            );
        }
        Op::SetTabUp { u, key, val } => {
            let _ = writeln!(
                out,
                "        if rt_settabup(ctx, ftop, cl, {u}, {}, {}) != 0 {{ {err} }}",
                opnd(key),
                opnd(val)
            );
        }
        Op::GetTable { a, b, key } => {
            let _ = writeln!(
                out,
                "        if rt_gettable(ctx, ftop, {}, {}, {}) != 0 {{ {err} }}",
                reg(b),
                opnd(key),
                reg(a)
            );
        }
        Op::SetTable { a, key, val } => {
            let _ = writeln!(
                out,
                "        if rt_settable(ctx, ftop, {}, {}, {}) != 0 {{ {err} }}",
                reg(a),
                opnd(key),
                opnd(val)
            );
        }
        Op::NewTable { a, narr, nhash } => {
            let _ = writeln!(out, "        rt_newtable(ctx, {}, {narr}, {nhash});", reg(a));
        }
        Op::SelfOp { a, b, key } => {
            let _ = writeln!(
                out,
                "        if rt_self(ctx, ftop, {}, {}, {}) != 0 {{ {err} }}",
                reg(a),
                reg(b),
                opnd(key)
            );
        }
        Op::Arith { op, a, b, c, nums } => {
            let dst = reg(a);
            let slow = format!(
                "if rt_arith(ctx, ftop, {op}, {dst}, {}, {}) != 0 {{ {err} }}",
                opnd(b),
                opnd(c)
            );
            if op == arith::POW {
                let _ = writeln!(out, "        {slow}");
            } else if nums {
                let _ = writeln!(
                    out,
                    "        *{dst} = Value::num(arith_num({op}, {}, {}));",
                    num_of(f, b),
                    num_of(f, c)
                );
            } else {
                let _ = writeln!(
                    out,
                    "        if {} {{ *{dst} = Value::num(arith_num({op}, {}, {})); }} else {{ {slow} }}",
                    both_num(f, b, c),
                    num_of(f, b),
                    num_of(f, c)
                );
            }
        }
        Op::Unm { a, b, num } => {
            if num {
                let _ = writeln!(out, "        *{} = Value::num(-(*{}).as_num());", reg(a), reg(b));
            } else {
                let _ = writeln!(
                    out,
                    "        if rt_arith(ctx, ftop, {}, {}, {}, {}) != 0 {{ {err} }}",
                    arith::UNM,
                    reg(a),
                    reg(b),
                    reg(b)
                );
            }
        }
        Op::Not { a, b } => {
            let _ = writeln!(out, "        *{} = Value::bool(!(*{}).truthy());", reg(a), reg(b));
        }
        Op::Len { a, b } => {
            let _ = writeln!(
                out,
                "        if rt_len(ctx, ftop, {}, {}) != 0 {{ {err} }}",
                reg(a),
                reg(b)
            );
        }
        Op::Concat { a, b, c } => {
            let _ = writeln!(
                out,
                "        if rt_concat(ctx, ftop, {}, {}, {}) != 0 {{ {err} }}",
                reg(b),
                c - b + 1,
                reg(a)
            );
        }
        Op::Close { from } => {
            let _ = writeln!(out, "        rt_close(ctx, {});", reg(from));
        }
        Op::Closure { a, idx } => {
            let _ = writeln!(
                out,
                "        if rt_closure(ctx, cl, {idx}, base, {}) != 0 {{ {err} }}",
                reg(a)
            );
        }
        Op::Call { a, args, rets } => {
            let func = reg(a);
            let nargs = match args {
                Count::Fixed(n) => format!("{n}_u32"),
                Count::ToTop => format!("(top.offset_from({func}) as u32 - 1)"),
            };
            let _ = writeln!(out, "        let n = rt_call_op(ctx, {func}, {nargs}, cl, {pc});");
            let _ = writeln!(out, "        if n < 0 {{ {err} }}");
            match rets {
                Count::ToTop => {
                    let _ = writeln!(out, "        top = {func}.add(n as usize);");
                }
                Count::Fixed(0) => {}
                Count::Fixed(k) => {
                    let _ = writeln!(
                        out,
                        "        if (n as u32) < {k} {{ rt_fixresults({func}, n as u32, {k}); }}"
                    );
                }
            }
        }
        Op::SetList { a, n, start } => {
            let first = reg(a + 1);
            let _ = writeln!(
                out,
                "        rt_setlist(ctx, {}, {first}, {}, {start});",
                reg(a),
                count_expr(n, &first)
            );
        }
        Op::VarArg { a, n } => {
            let dst = reg(a);
            let want = match n {
                Count::Fixed(k) => format!("{k}"),
                Count::ToTop => "-1".to_string(),
            };
            let _ = writeln!(out, "        let c = rt_vararg(ctx, {dst}, vptr, nvar, {want});");
            let _ = writeln!(out, "        if c < 0 {{ {err} }}");
            if n == Count::ToTop {
                let _ = writeln!(out, "        top = {dst}.add(c as usize);");
            }
        }
        Op::ForPrep { a } => {
            let _ = writeln!(out, "        if rt_forprep(ctx, {}) != 0 {{ {err} }}", reg(a));
        }
        Op::TForCall { a, n } => {
            let _ = writeln!(out, "        if rt_tforcall(ctx, {}, {n}) != 0 {{ {err} }}", reg(a));
        }
    }
}

fn cond_expr(f: &Func, c: &Cond, out: &mut String) -> String {
    let err = "return -1;";
    match *c {
        Cond::Test { a, expect } => format!("(*{}).truthy() == {expect}", reg(a)),
        Cond::Eq { b, c, expect, nums } => {
            let _ = nums;
            let _ = writeln!(
                out,
                "        let r = if {} {{ {} == {} }} else {{ let r = rt_eq(ctx, ftop, {}, {}); if r < 0 {{ {err} }} r != 0 }};",
                both_num(f, b, c),
                num_of(f, b),
                num_of(f, c),
                opnd(b),
                opnd(c)
            );
            format!("r == {expect}")
        }
        Cond::Lt { b, c, expect, .. } => order_expr(f, true, b, c, expect, out),
        Cond::Le { b, c, expect, .. } => order_expr(f, false, b, c, expect, out),
    }
}

fn order_expr(f: &Func, lt: bool, b: Opnd, c: Opnd, expect: bool, out: &mut String) -> String {
    let (sym, helper) = if lt { ("<", "rt_lt") } else { ("<=", "rt_le") };
    let _ = writeln!(
        out,
        "        let r = if {} {{ {} {sym} {} }} else {{ let r = {helper}(ctx, ftop, {}, {}); if r < 0 {{ return -1; }} r != 0 }};",
        both_num(f, b, c),
        num_of(f, b),
        num_of(f, c),
        opnd(b),
        opnd(c)
    );
    format!("r == {expect}")
}

fn edge(f: &Func, from: usize, to: usize, out: &mut String, indent: &str) {
    if f.blocks[to].pc <= f.blocks[from].pc {
        let _ = writeln!(
            out,
            "{indent}(*ctx).fuel -= 1; if (*ctx).fuel < 0 {{ rt_out_of_fuel(ctx); return -1; }}"
        );
    }
    let _ = writeln!(out, "{indent}bb = {to}; continue;");
}

/// Renders one function.
#[must_use]
pub fn render_func(f: &Func, name: &str) -> String {
    let mut s = String::new();
    let frame = f.max_stack;
    let _ = writeln!(s, "pub unsafe extern \"C\" fn {name}(ctx: *mut Ctx, func: *mut Value, nargs: u32) -> i32 {{");
    let _ = writeln!(s, "    unsafe {{");
    let _ = writeln!(s, "        let cl = (*func).bits as *const Closure;");
    let _ = writeln!(s, "        let k = (*cl).k;");
    if f.is_vararg {
        let _ = writeln!(s, "        let base = rt_vprologue(ctx, func, nargs, {}, {frame});", f.num_params);
        let _ = writeln!(s, "        if base.is_null() {{ rt_out_of_stack(ctx); return -1; }}");
        let _ = writeln!(s, "        let nvar = nargs.saturating_sub({});", f.num_params);
        let _ = writeln!(s, "        let vptr = func.add({}) as *const Value;", 1 + f.num_params);
    } else {
        let _ = writeln!(s, "        let base = func.add(1);");
        let _ = writeln!(s, "        if base.add({frame}) > (*ctx).stack_limit {{ rt_out_of_stack(ctx); return -1; }}");
        if f.num_params > 0 {
            let _ = writeln!(s, "        if nargs < {} {{ rt_nilfill(base, nargs, {}); }}", f.num_params, f.num_params);
        }
    }
    let _ = writeln!(s, "        let ftop = base.add({frame});");
    let _ = writeln!(s, "        let mut top: *mut Value = std::ptr::null_mut();");
    let _ = writeln!(s, "        (*ctx).fuel -= 1; if (*ctx).fuel < 0 {{ rt_out_of_fuel(ctx); return -1; }}");
    let _ = writeln!(s, "        let mut bb: usize = 0;");
    let _ = writeln!(s, "        loop {{ match bb {{");
    for (i, b) in f.blocks.iter().enumerate() {
        let _ = writeln!(s, "    {i} => {{ // pc {}", b.pc);
        for (o, pc) in b.ops.iter().zip(&b.pcs) {
            op(f, o, *pc, &mut s);
        }
        match b.term {
            Term::Jmp(t) => edge(f, i, t, &mut s, "        "),
            Term::Br { cond, then, els } => {
                let expr = cond_expr(f, &cond, &mut s);
                let _ = writeln!(s, "        if {expr} {{");
                edge(f, i, then, &mut s, "            ");
                let _ = writeln!(s, "        }} else {{");
                edge(f, i, els, &mut s, "            ");
                let _ = writeln!(s, "        }}");
            }
            Term::ForLoop { a, body, exit } => {
                let _ = writeln!(s, "        let step = (*{}).as_num();", reg(a + 2));
                let _ = writeln!(s, "        let next = (*{}).as_num() + step;", reg(a));
                let _ = writeln!(s, "        (*{}).bits = next.to_bits();", reg(a));
                let _ = writeln!(
                    s,
                    "        if if step > 0.0 {{ next <= (*{}).as_num() }} else {{ (*{}).as_num() <= next }} {{",
                    reg(a + 1),
                    reg(a + 1)
                );
                let _ = writeln!(s, "            *{} = Value::num(next);", reg(a + 3));
                edge(f, i, body, &mut s, "            ");
                let _ = writeln!(s, "        }} else {{");
                edge(f, i, exit, &mut s, "            ");
                let _ = writeln!(s, "        }}");
            }
            Term::TForLoop { a, body, exit } => {
                let _ = writeln!(s, "        if !(*{}).is_nil() {{", reg(a + 1));
                let _ = writeln!(s, "            *{} = *{};", reg(a), reg(a + 1));
                edge(f, i, body, &mut s, "            ");
                let _ = writeln!(s, "        }} else {{");
                edge(f, i, exit, &mut s, "            ");
                let _ = writeln!(s, "        }}");
            }
            Term::Return { a, n } => {
                if f.captures_locals {
                    let _ = writeln!(s, "        rt_close(ctx, base);");
                }
                let src = reg(a);
                let _ = writeln!(s, "        let cnt = {};", count_expr(n, &src));
                let _ = writeln!(s, "        for i in 0..cnt as usize {{ *func.add(i) = *{src}.add(i); }}");
                let _ = writeln!(s, "        return cnt as i32;");
            }
        }
        let _ = writeln!(s, "    }}");
    }
    let _ = writeln!(s, "    _ => return -1,");
    let _ = writeln!(s, "        }} }}");
    let _ = writeln!(s, "    }}");
    let _ = writeln!(s, "}}");
    s
}

/// Renders a module for a whole chunk: one function per prototype (pre-order) and the `FUNCS`
/// table binding them for [`crate::aot::load_image`].
#[must_use]
pub fn render_module(funcs: &[Func], chunk_name: &str) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "//! Generated from `{chunk_name}` by nie-lua aot (Rust backend). Do not edit.");
    let _ = writeln!(s, "#![allow(unused_unsafe, unused_variables, unused_mut, clippy::all)]");
    let _ = writeln!(s, "use nie_lua::rt::heap::Closure;");
    let _ = writeln!(s, "use nie_lua::rt::ops::*;");
    let _ = writeln!(s, "use nie_lua::rt::state::{{Ctx, NativeFn, arith_num}};");
    let _ = writeln!(s, "use nie_lua::rt::value::{{Value, tag}};");
    for (i, f) in funcs.iter().enumerate() {
        s.push('\n');
        s.push_str(&render_func(f, &format!("f{i}")));
    }
    let _ = writeln!(s, "\npub static FUNCS: [NativeFn; {}] = [", funcs.len());
    for i in 0..funcs.len() {
        let _ = writeln!(s, "    f{i},");
    }
    let _ = writeln!(s, "];");
    s
}
