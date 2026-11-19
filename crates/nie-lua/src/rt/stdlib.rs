//! The Lua 5.2 standard library subset the game uses, plus the rest of the base library:
//! `base`, `string`, `table`, `math`, `coroutine`, `bit32`.

use super::heap::register_fns;
use super::state::{Args, Ctx, LResult, RustFn, Thrown};
use super::table::Table;
use super::value::{Value, tag};
use super::{bit32 as b32, lsort, lstr};

/// `true` when the error value is the fuel sentinel, which `pcall` must not swallow.
fn is_fuel_error(ctx: &Ctx) -> bool {
    ctx.err.tag == tag::STR
        // SAFETY: STR tag.
        && unsafe { ctx.err.str_bytes() } == super::FUEL_ERROR.as_bytes()
}

fn got(a: &Args, i: usize) -> &'static str {
    if i >= a.n { "no value" } else { a.get(i).type_name() }
}

fn check_num(ctx: &mut Ctx, a: &Args, i: usize, f: &str) -> LResult<f64> {
    let v = a.get(i);
    if let Some(n) = ctx.to_number(v) {
        if i < a.n {
            return Ok(n);
        }
    }
    ctx.arg_error(i, f, &format!("number expected, got {}", got(a, i)))
}

fn opt_num(ctx: &mut Ctx, a: &Args, i: usize, f: &str, d: f64) -> LResult<f64> {
    if a.get(i).is_nil() { Ok(d) } else { check_num(ctx, a, i, f) }
}

/// `luaL_checkinteger`: number truncated toward zero.
fn check_int(ctx: &mut Ctx, a: &Args, i: usize, f: &str) -> LResult<i64> {
    Ok(check_num(ctx, a, i, f)? as i64)
}

fn check_str(ctx: &mut Ctx, a: &Args, i: usize, f: &str) -> LResult<Value> {
    let v = a.get(i);
    if v.tag == tag::STR {
        return Ok(v);
    }
    if v.tag == tag::NUM {
        return Ok(ctx.to_str_raw(v));
    }
    ctx.arg_error(i, f, &format!("string expected, got {}", got(a, i)))
}

fn check_table(ctx: &mut Ctx, a: &Args, i: usize, f: &str) -> LResult<*mut Table> {
    let v = a.get(i);
    if v.tag == tag::TABLE {
        return Ok(v.bits as *mut Table);
    }
    ctx.arg_error(i, f, &format!("table expected, got {}", got(a, i)))
}

fn bytes_of(v: Value) -> Vec<u8> {
    // SAFETY: callers pass STR values.
    unsafe { v.str_bytes().to_vec() }
}

fn str_err<T>(ctx: &mut Ctx, e: &str) -> LResult<T> {
    ctx.throw_lib(e)
}

// ----------------------------------------------------------------------------- base

fn base_print(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    for i in 0..a.n {
        if i > 0 {
            ctx.out.push(b'\t');
        }
        let s = ctx.tostring(a.scratch(), a.get(i))?;
        // SAFETY: tostring returns strings.
        ctx.out.extend_from_slice(unsafe { s.str_bytes() });
    }
    ctx.out.push(b'\n');
    a.none()
}

fn base_type(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    if a.n == 0 {
        return ctx.arg_error(0, "type", "value expected");
    }
    let s = ctx.str(a.get(0).type_name().as_bytes());
    a.one(s)
}

fn base_tostring(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    if a.n == 0 {
        return ctx.arg_error(0, "tostring", "value expected");
    }
    let s = ctx.tostring(a.scratch(), a.get(0))?;
    a.one(s)
}

fn base_tonumber(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let v = a.get(0);
    if a.get(1).is_nil() {
        return match v.tag {
            tag::NUM => a.one(v),
            tag::STR => a.one(ctx.to_number(v).map_or(Value::NIL, Value::num)),
            _ => {
                if a.n == 0 {
                    return ctx.arg_error(0, "tonumber", "value expected");
                }
                a.one(Value::NIL)
            }
        };
    }
    let base = check_int(ctx, &a, 1, "tonumber")?;
    let s = check_str(ctx, &a, 0, "tonumber")?;
    if !(2..=36).contains(&base) {
        return ctx.arg_error(1, "tonumber", "base out of range");
    }
    let text = bytes_of(s);
    let t = String::from_utf8_lossy(&text);
    let t = t.trim_matches(|c: char| c.is_ascii_whitespace());
    let (neg, digits) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t),
    };
    if digits.is_empty() || !digits.chars().all(|c| c.is_digit(base as u32)) {
        return a.one(Value::NIL);
    }
    let mut n = 0.0_f64;
    for c in digits.chars() {
        n = n * base as f64 + f64::from(c.to_digit(base as u32).unwrap_or(0));
    }
    a.one(Value::num(if neg { -n } else { n }))
}

fn next_impl(ctx: &mut Ctx, a: &Args, name: &str) -> LResult<usize> {
    let t = check_table(ctx, a, 0, name)?;
    // SAFETY: live table.
    match unsafe { (*t).next(a.get(1)) } {
        None => ctx.throw_lib("invalid key to 'next'"),
        Some(None) => a.one(Value::NIL),
        Some(Some((k, v))) => a.many(ctx, &[k, v]),
    }
}

fn base_next(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    next_impl(ctx, &a, "next")
}

fn base_pairs(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let t = a.get(0);
    let mm = ctx.str(b"__pairs");
    let mm = ctx.metamethod(t, mm);
    if !mm.is_nil() {
        let r = ctx.call_n(a.scratch(), mm, &[t])?;
        let mut out = [Value::NIL; 3];
        for (i, v) in r.into_iter().take(3).enumerate() {
            out[i] = v;
        }
        return a.many(ctx, &out);
    }
    check_table(ctx, &a, 0, "pairs")?;
    let next = ctx.get_global("next");
    a.many(ctx, &[next, t, Value::NIL])
}

fn ipairs_iter(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let t = check_table(ctx, &a, 0, "ipairs")?;
    let i = check_int(ctx, &a, 1, "ipairs")? + 1;
    // SAFETY: live table.
    let v = unsafe { (*t).get_int(i) };
    if v.is_nil() {
        a.one(Value::NIL)
    } else {
        a.many(ctx, &[Value::num(i as f64), v])
    }
}

fn base_ipairs(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    check_table(ctx, &a, 0, "ipairs")?;
    let iter = ctx.new_rust_fn(ipairs_iter);
    a.many(ctx, &[iter, a.get(0), Value::num(0.0)])
}

fn base_rawget(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let t = check_table(ctx, &a, 0, "rawget")?;
    // SAFETY: live table.
    a.one(unsafe { (*t).get(a.get(1)) })
}

fn base_rawset(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let t = check_table(ctx, &a, 0, "rawset")?;
    ctx.raw_set(t, a.get(1), a.get(2))?;
    a.one(a.get(0))
}

fn base_rawequal(_ctx: &mut Ctx, a: Args) -> LResult<usize> {
    a.one(Value::bool(a.get(0).raw_eq(a.get(1))))
}

fn base_rawlen(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let v = a.get(0);
    match v.tag {
        // SAFETY: tag checked.
        tag::TABLE => a.one(Value::num(unsafe { (*(v.bits as *const Table)).len() } as f64)),
        tag::STR => a.one(Value::num(unsafe { v.str_bytes().len() } as f64)),
        _ => ctx.arg_error(0, "rawlen", "table or string expected"),
    }
}

fn base_setmetatable(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let t = check_table(ctx, &a, 0, "setmetatable")?;
    let m = a.get(1);
    if m.tag != tag::TABLE && !m.is_nil() {
        return ctx.arg_error(1, "setmetatable", "nil or table expected");
    }
    // SAFETY: live tables.
    unsafe {
        if !(*t).meta.is_null() && !(*(*t).meta).get_str(ctx.names.metatable).is_nil() {
            return ctx.throw_lib("cannot change a protected metatable");
        }
        (*t).meta = if m.is_nil() { std::ptr::null_mut() } else { m.bits as *mut Table };
    }
    a.one(a.get(0))
}

fn base_getmetatable(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let mt = ctx.metatable_of(a.get(0));
    if mt.is_null() {
        return a.one(Value::NIL);
    }
    // SAFETY: live table.
    let protected = unsafe { (*mt).get_str(ctx.names.metatable) };
    if !protected.is_nil() {
        return a.one(protected);
    }
    a.one(Value { tag: tag::TABLE, bits: mt as u64 })
}

fn base_select(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let first = a.get(0);
    if first.tag == tag::STR && unsafe { first.str_bytes() } == b"#" {
        return a.one(Value::num((a.n - 1) as f64));
    }
    let n = check_int(ctx, &a, 0, "select")?;
    let count = a.n as i64 - 1;
    let start = if n < 0 { count + n } else { n - 1 };
    if start < 0 || (n < 0 && -n > count) || n == 0 {
        return ctx.arg_error(0, "select", "index out of range");
    }
    let vals: Vec<Value> = (start..count).map(|i| a.get(1 + i as usize)).collect();
    a.many(ctx, &vals)
}

fn base_assert(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    if a.get(0).truthy() {
        let vals: Vec<Value> = (0..a.n).map(|i| a.get(i)).collect();
        return a.many(ctx, &vals);
    }
    if a.n > 1 {
        ctx.err = a.get(1);
        return Err(Thrown);
    }
    ctx.throw_lib("assertion failed!")
}

fn base_error(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let v = a.get(0);
    let level = if a.get(1).is_nil() { 1.0 } else { check_num(ctx, &a, 1, "error")? };
    // `luaL_where` is empty for stripped chunks, but the concatenation still turns a number
    // into a string.
    ctx.err = if matches!(v.tag, tag::STR | tag::NUM) && level > 0.0 { ctx.to_str_raw(v) } else { v };
    Err(Thrown)
}

fn base_pcall(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    if a.n == 0 {
        return ctx.arg_error(0, "pcall", "value expected");
    }
    // SAFETY: the callee and its arguments are already laid out at `func[1..]`.
    let r = unsafe { super::state::rt_call(ctx, a.func.add(1), (a.n - 1) as u32) };
    if r >= 0 {
        // SAFETY: slot 0 is the callee slot.
        unsafe { *a.func = Value::bool(true) };
        return Ok(r as usize + 1);
    }
    if is_fuel_error(ctx) {
        return Err(Thrown);
    }
    // SAFETY: closing at the callee slot keeps the stack consistent.
    ctx.close_upvals(unsafe { a.func.add(1) });
    let e = ctx.err;
    a.many(ctx, &[Value::bool(false), e])
}

fn base_xpcall(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    if a.n < 2 {
        return ctx.arg_error(1, "xpcall", "value expected");
    }
    let (f, handler) = (a.get(0), a.get(1));
    // SAFETY: layout `[xpcall, f, handler, args...]`: reuse the handler slot for the callee.
    let r = unsafe {
        *a.func.add(2) = f;
        super::state::rt_call(ctx, a.func.add(2), (a.n - 2) as u32)
    };
    if r >= 0 {
        // SAFETY: results start at `func[2]`; move them down by one behind `true`.
        unsafe {
            *a.func = Value::bool(true);
            for i in 0..r as usize {
                *a.func.add(1 + i) = *a.func.add(2 + i);
            }
        }
        return Ok(r as usize + 1);
    }
    if is_fuel_error(ctx) {
        return Err(Thrown);
    }
    // SAFETY: as in pcall.
    ctx.close_upvals(unsafe { a.func.add(2) });
    let e = ctx.err;
    let h = ctx.call1(a.scratch(), handler, &[e])?;
    a.many(ctx, &[Value::bool(false), h])
}

fn base_collectgarbage(_ctx: &mut Ctx, a: Args) -> LResult<usize> {
    a.one(Value::num(0.0))
}

// --------------------------------------------------------------------------- string

fn s_len(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let s = check_str(ctx, &a, 0, "len")?;
    a.one(Value::num(bytes_of(s).len() as f64))
}

fn s_sub(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let s = check_str(ctx, &a, 0, "sub")?;
    let i = check_num(ctx, &a, 1, "sub")?;
    let j = opt_num(ctx, &a, 2, "sub", -1.0)?;
    // SAFETY: STR.
    let out = lstr::sub(unsafe { s.str_bytes() }, i, j).to_vec();
    let v = ctx.str(&out);
    a.one(v)
}

fn s_upper(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let s = check_str(ctx, &a, 0, "upper")?;
    let v = ctx.str(&lstr::upper(&bytes_of(s)));
    a.one(v)
}

fn s_lower(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let s = check_str(ctx, &a, 0, "lower")?;
    let v = ctx.str(&lstr::lower(&bytes_of(s)));
    a.one(v)
}

fn s_reverse(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let s = check_str(ctx, &a, 0, "reverse")?;
    let v = ctx.str(&lstr::reverse(&bytes_of(s)));
    a.one(v)
}

fn s_rep(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let s = check_str(ctx, &a, 0, "rep")?;
    let n = check_num(ctx, &a, 1, "rep")?;
    let sep = if a.get(2).is_nil() { None } else { Some(bytes_of(check_str(ctx, &a, 2, "rep")?)) };
    match lstr::rep(&bytes_of(s), n, sep.as_deref()) {
        Ok(out) => {
            let v = ctx.str(&out);
            a.one(v)
        }
        Err(e) => str_err(ctx, &e),
    }
}

fn s_byte(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let s = check_str(ctx, &a, 0, "byte")?;
    let i = opt_num(ctx, &a, 1, "byte", 1.0)?;
    let j = opt_num(ctx, &a, 2, "byte", i)?;
    match lstr::byte(&bytes_of(s), i, j) {
        Ok(v) => {
            let vals: Vec<Value> = v.into_iter().map(Value::num).collect();
            a.many(ctx, &vals)
        }
        Err(e) => str_err(ctx, &e),
    }
}

fn s_char(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let mut codes = Vec::with_capacity(a.n);
    for i in 0..a.n {
        codes.push(check_num(ctx, &a, i, "char")?);
    }
    match lstr::char_from(&codes) {
        Ok(out) => {
            let v = ctx.str(&out);
            a.one(v)
        }
        Err(e) => str_err(ctx, &e),
    }
}

fn capture_value(ctx: &mut Ctx, c: &lstr::Capture) -> Value {
    match c {
        lstr::Capture::Str(s) => ctx.str(s),
        lstr::Capture::Position(p) => Value::num(*p as f64),
        lstr::Capture::Unfinished => Value::NIL,
    }
}

fn init_index(init: f64, len: usize) -> usize {
    let n = init as i64;
    if n > 0 {
        (n - 1) as usize
    } else if n == 0 {
        0
    } else if (-n) as usize > len {
        0
    } else {
        len - (-n) as usize
    }
}

fn find_aux(ctx: &mut Ctx, a: Args, find: bool) -> LResult<usize> {
    let name = if find { "find" } else { "match" };
    let s = bytes_of(check_str(ctx, &a, 0, name)?);
    let p = bytes_of(check_str(ctx, &a, 1, name)?);
    let init = init_index(opt_num(ctx, &a, 2, name, 1.0)?, s.len());
    if init > s.len() {
        return a.one(Value::NIL);
    }
    if find && (a.get(3).truthy() || !lstr::has_specials(&p)) {
        return match lstr::find_plain(&s, &p, init) {
            Some((st, en)) => a.many(ctx, &[Value::num((st + 1) as f64), Value::num(en as f64)]),
            None => a.one(Value::NIL),
        };
    }
    match lstr::pat_find(&s, &p, init) {
        Err(e) => str_err(ctx, &e),
        Ok(None) => a.one(Value::NIL),
        Ok(Some(f)) => {
            let mut out = Vec::new();
            if find {
                out.push(Value::num((f.start + 1) as f64));
                out.push(Value::num(f.end as f64));
            }
            if f.captures.is_empty() && !find {
                let w = ctx.str(&s[f.start..f.end]);
                out.push(w);
            }
            for c in &f.captures {
                let v = capture_value(ctx, c);
                out.push(v);
            }
            a.many(ctx, &out)
        }
    }
}

fn s_find(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    find_aux(ctx, a, true)
}

fn s_match(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    find_aux(ctx, a, false)
}

fn gmatch_iter(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    // State table bound as data: { s, p, pos }.
    let st = a.data();
    // SAFETY: the state is a table created by `s_gmatch`.
    let t = unsafe { &mut *(st.bits as *mut Table) };
    let (s, p) = (bytes_of(t.get_int(1)), bytes_of(t.get_int(2)));
    let pos = t.get_int(3).as_num() as usize;
    if pos > s.len() {
        return a.one(Value::NIL);
    }
    match lstr::pat_find(&s, &p, pos) {
        Err(e) => str_err(ctx, &e),
        Ok(None) => {
            t.set(Value::num(3.0), Value::num((s.len() + 1) as f64));
            a.one(Value::NIL)
        }
        Ok(Some(f)) => {
            let next = if f.end == f.start { f.end + 1 } else { f.end };
            t.set(Value::num(3.0), Value::num(next as f64));
            let mut out = Vec::new();
            if f.captures.is_empty() {
                out.push(ctx.str(&s[f.start..f.end]));
            }
            for c in &f.captures {
                let v = capture_value(ctx, c);
                out.push(v);
            }
            a.many(ctx, &out)
        }
    }
}

fn s_gmatch(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let s = check_str(ctx, &a, 0, "gmatch")?;
    let p = check_str(ctx, &a, 1, "gmatch")?;
    let st = ctx.new_table(3, 0);
    // SAFETY: fresh table.
    unsafe {
        let t = &mut *(st.bits as *mut Table);
        t.set(Value::num(1.0), s);
        t.set(Value::num(2.0), p);
        t.set(Value::num(3.0), Value::num(0.0));
    }
    let f = ctx.new_rust_closure(gmatch_iter, st);
    a.one(f)
}

fn s_gsub(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let s = bytes_of(check_str(ctx, &a, 0, "gsub")?);
    let p = bytes_of(check_str(ctx, &a, 1, "gsub")?);
    let repl = a.get(2);
    if !matches!(repl.tag, tag::NUM | tag::STR | tag::TABLE | tag::FUNC) {
        return ctx.arg_error(2, "gsub", "string/function/table expected");
    }
    let max_n = if a.get(3).is_nil() {
        None
    } else {
        let n = check_num(ctx, &a, 3, "gsub")?;
        Some(if n < 0.0 { 0 } else { n as usize })
    };
    let repl_str = if matches!(repl.tag, tag::NUM | tag::STR) {
        Some(bytes_of(ctx.to_str_raw(repl)))
    } else {
        None
    };
    let scratch = a.scratch();
    let mut thrown = false;
    let res = lstr::gsub(&s, &p, max_n, &mut |whole, caps| {
        if let Some(r) = &repl_str {
            return lstr::expand_repl(r, whole, caps).map(lstr::ReplOut::Bytes);
        }
        if caps.iter().any(|c| matches!(c, lstr::Capture::Unfinished)) {
            return Err("unfinished capture".to_string());
        }
        let first = match caps.first() {
            Some(c) => capture_value(ctx, c),
            None => ctx.str(whole),
        };
        let v = if repl.tag == tag::TABLE {
            match ctx.index(scratch, repl, first) {
                Ok(v) => v,
                Err(Thrown) => {
                    thrown = true;
                    return Err(String::new());
                }
            }
        } else {
            let args: Vec<Value> = if caps.is_empty() {
                vec![ctx.str(whole)]
            } else {
                caps.iter().map(|c| capture_value(ctx, c)).collect()
            };
            match ctx.call1(scratch, repl, &args) {
                Ok(v) => v,
                Err(Thrown) => {
                    thrown = true;
                    return Err(String::new());
                }
            }
        };
        match v.tag {
            tag::NIL => Ok(lstr::ReplOut::Keep),
            tag::BOOL if v.bits == 0 => Ok(lstr::ReplOut::Keep),
            tag::STR | tag::NUM => Ok(lstr::ReplOut::Bytes(bytes_of(ctx.to_str_raw(v)))),
            _ => Err(format!("invalid replacement value (a {})", v.type_name())),
        }
    });
    if thrown {
        return Err(Thrown);
    }
    match res {
        Ok((out, n)) => {
            let v = ctx.str(&out);
            a.many(ctx, &[v, Value::num(n as f64)])
        }
        Err(e) => str_err(ctx, &e),
    }
}

fn s_format(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let fmt = bytes_of(check_str(ctx, &a, 0, "format")?);
    let scratch = a.scratch();
    let mut thrown = false;
    let res = lstr::format(&fmt, &mut |i, want| {
        let idx = i + 1;
        match want {
            lstr::Want::Present => {
                if idx >= a.n {
                    Err(ctx.arg_message(idx, "format", "no value"))
                } else {
                    Ok(lstr::FmtArg::Num(0.0))
                }
            }
            lstr::Want::Number => match ctx.to_number(a.get(idx)).filter(|_| idx < a.n) {
                Some(n) => Ok(lstr::FmtArg::Num(n)),
                None => Err(ctx.arg_message(
                    idx,
                    "format",
                    &format!("number expected, got {}", got(&a, idx)),
                )),
            },
            lstr::Want::Str | lstr::Want::Any => {
                if idx >= a.n {
                    return Err(ctx.arg_message(idx, "format", "no value"));
                }
                match ctx.tostring(scratch, a.get(idx)) {
                    Ok(v) => Ok(lstr::FmtArg::Str(bytes_of(v))),
                    Err(Thrown) => {
                        thrown = true;
                        Err(String::new())
                    }
                }
            }
        }
    });
    if thrown {
        return Err(Thrown);
    }
    match res {
        Ok(out) => {
            let v = ctx.str(&out);
            a.one(v)
        }
        Err(e) => str_err(ctx, &e),
    }
}

// ---------------------------------------------------------------------------- table

fn t_insert(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let t = check_table(ctx, &a, 0, "insert")?;
    // SAFETY: live table.
    let e = unsafe { (*t).len() } as i64 + 1;
    let (pos, val) = match a.n {
        2 => (e, a.get(1)),
        3 => {
            let pos = check_int(ctx, &a, 1, "insert")?;
            if !(1..=e).contains(&pos) {
                return ctx.arg_error(1, "insert", "position out of bounds");
            }
            for i in (pos + 1..=e).rev() {
                // SAFETY: live table.
                unsafe {
                    let v = (*t).get_int(i - 1);
                    (*t).set(Value::num(i as f64), v);
                }
            }
            (pos, a.get(2))
        }
        _ => return ctx.throw_lib("wrong number of arguments to 'insert'"),
    };
    ctx.raw_set(t, Value::num(pos as f64), val)?;
    a.none()
}

fn t_remove(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let t = check_table(ctx, &a, 0, "remove")?;
    // SAFETY: live table.
    let size = unsafe { (*t).len() } as i64;
    let mut pos = if a.get(1).is_nil() { size } else { check_int(ctx, &a, 1, "remove")? };
    if pos != size && !(1..=size + 1).contains(&pos) {
        return ctx.arg_error(1, "remove", "position out of bounds");
    }
    // SAFETY: live table.
    let result = unsafe { (*t).get_int(pos) };
    while pos < size {
        // SAFETY: live table.
        unsafe {
            let v = (*t).get_int(pos + 1);
            (*t).set(Value::num(pos as f64), v);
        }
        pos += 1;
    }
    // SAFETY: live table.
    unsafe { (*t).set(Value::num(pos as f64), Value::NIL) };
    a.one(result)
}

fn t_concat(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let t = check_table(ctx, &a, 0, "concat")?;
    let sep = if a.get(1).is_nil() { Vec::new() } else { bytes_of(check_str(ctx, &a, 1, "concat")?) };
    // SAFETY: live table.
    let len = unsafe { (*t).len() } as i64;
    let i = if a.get(2).is_nil() { 1 } else { check_int(ctx, &a, 2, "concat")? };
    let j = if a.get(3).is_nil() { len } else { check_int(ctx, &a, 3, "concat")? };
    let mut out = Vec::new();
    for k in i..=j {
        // SAFETY: live table.
        let v = unsafe { (*t).get_int(k) };
        if !matches!(v.tag, tag::STR | tag::NUM) {
            return ctx.throw_lib(&format!(
                "invalid value ({}) at index {k} in table for 'concat'",
                v.type_name()
            ));
        }
        out.extend_from_slice(&bytes_of(ctx.to_str_raw(v)));
        if k != j {
            out.extend_from_slice(&sep);
        }
    }
    let v = ctx.str(&out);
    a.one(v)
}

fn t_unpack(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let t = check_table(ctx, &a, 0, "unpack")?;
    let i = if a.get(1).is_nil() { 1 } else { check_int(ctx, &a, 1, "unpack")? };
    // SAFETY: live table.
    let j = if a.get(2).is_nil() { (unsafe { (*t).len() }) as i64 } else { check_int(ctx, &a, 2, "unpack")? };
    if i > j {
        return a.none();
    }
    let n = (j - i + 1) as usize;
    if n >= (1 << 24) {
        return ctx.throw_lib("too many results to unpack");
    }
    // SAFETY: live table.
    let vals: Vec<Value> = (i..=j).map(|k| unsafe { (*t).get_int(k) }).collect();
    a.many(ctx, &vals)
}

fn t_pack(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let t = ctx.new_table(a.n, 1);
    let tp = t.bits as *mut Table;
    for i in 0..a.n {
        // SAFETY: fresh table.
        unsafe { (*tp).set(Value::num((i + 1) as f64), a.get(i)) };
    }
    let k = ctx.str(b"n");
    // SAFETY: fresh table.
    unsafe { (*tp).set(k, Value::num(a.n as f64)) };
    a.one(t)
}

fn t_sort(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let t = check_table(ctx, &a, 0, "sort")?;
    let cmp = a.get(1);
    if !cmp.is_nil() && cmp.tag != tag::FUNC {
        return ctx.arg_error(1, "sort", &format!("function expected, got {}", got(&a, 1)));
    }
    // SAFETY: live table.
    let n = unsafe { (*t).len() };
    // SAFETY: live table.
    let mut items: Vec<Value> = (1..=n).map(|k| unsafe { (*t).get_int(k as i64) }).collect();
    let scratch = a.scratch();
    let invalid = std::cell::Cell::new(false);
    let r = lsort::sort(
        &mut items,
        &mut |x, y| {
            if cmp.is_nil() {
                ctx.less_than(scratch, *x, *y)
            } else {
                ctx.call1(scratch, cmp, &[*x, *y]).map(Value::truthy)
            }
        },
        || {
            invalid.set(true);
            Thrown
        },
    );
    if r.is_err() {
        if invalid.get() {
            return ctx.throw_lib("invalid order function for sorting");
        }
        return Err(Thrown);
    }
    for (k, v) in items.into_iter().enumerate() {
        // SAFETY: live table.
        unsafe { (*t).set(Value::num((k + 1) as f64), v) };
    }
    a.none()
}

// ------------------------------------------------------------------------------ math

macro_rules! math1 {
    ($name:ident, $lua:literal, $f:expr) => {
        fn $name(ctx: &mut Ctx, a: Args) -> LResult<usize> {
            let x = check_num(ctx, &a, 0, $lua)?;
            let f: fn(f64) -> f64 = $f;
            a.one(Value::num(f(x)))
        }
    };
}

math1!(m_abs, "abs", f64::abs);
math1!(m_acos, "acos", f64::acos);
math1!(m_asin, "asin", f64::asin);
math1!(m_atan, "atan", f64::atan);
math1!(m_ceil, "ceil", f64::ceil);
math1!(m_cos, "cos", f64::cos);
math1!(m_cosh, "cosh", f64::cosh);
math1!(m_deg, "deg", |x| x * (180.0 / std::f64::consts::PI));
math1!(m_exp, "exp", f64::exp);
math1!(m_floor, "floor", f64::floor);
math1!(m_rad, "rad", |x| x * (std::f64::consts::PI / 180.0));
math1!(m_sin, "sin", f64::sin);
math1!(m_sinh, "sinh", f64::sinh);
math1!(m_sqrt, "sqrt", f64::sqrt);
math1!(m_tan, "tan", f64::tan);
math1!(m_tanh, "tanh", f64::tanh);

fn m_atan2(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let (y, x) = (check_num(ctx, &a, 0, "atan2")?, check_num(ctx, &a, 1, "atan2")?);
    a.one(Value::num(y.atan2(x)))
}

fn m_fmod(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let (x, y) = (check_num(ctx, &a, 0, "fmod")?, check_num(ctx, &a, 1, "fmod")?);
    a.one(Value::num(x % y))
}

fn m_pow(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let (x, y) = (check_num(ctx, &a, 0, "pow")?, check_num(ctx, &a, 1, "pow")?);
    a.one(Value::num(x.powf(y)))
}

fn m_ldexp(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let (x, e) = (check_num(ctx, &a, 0, "ldexp")?, check_int(ctx, &a, 1, "ldexp")?);
    a.one(Value::num(x * 2.0_f64.powi(e.clamp(-2200, 2200) as i32)))
}

fn m_frexp(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let x = check_num(ctx, &a, 0, "frexp")?;
    if x == 0.0 || !x.is_finite() {
        return a.many(ctx, &[Value::num(x), Value::num(0.0)]);
    }
    let mut e = x.abs().log2().floor() as i32 + 1;
    let mut m = x / 2.0_f64.powi(e);
    if m.abs() >= 1.0 {
        m /= 2.0;
        e += 1;
    } else if m.abs() < 0.5 {
        m *= 2.0;
        e -= 1;
    }
    a.many(ctx, &[Value::num(m), Value::num(f64::from(e))])
}

fn m_modf(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let x = check_num(ctx, &a, 0, "modf")?;
    let ip = if x >= 0.0 { x.floor() } else { x.ceil() };
    let frac = if x.is_infinite() { 0.0 } else { x - ip };
    a.many(ctx, &[Value::num(ip), Value::num(frac)])
}

fn m_log(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let x = check_num(ctx, &a, 0, "log")?;
    if a.get(1).is_nil() {
        return a.one(Value::num(x.ln()));
    }
    let base = check_num(ctx, &a, 1, "log")?;
    a.one(Value::num(if base == 2.0 {
        x.log2()
    } else if base == 10.0 {
        x.log10()
    } else {
        x.ln() / base.ln()
    }))
}

fn m_minmax(ctx: &mut Ctx, a: Args, name: &str, max: bool) -> LResult<usize> {
    let mut best = check_num(ctx, &a, 0, name)?;
    for i in 1..a.n {
        let v = check_num(ctx, &a, i, name)?;
        if (max && v > best) || (!max && v < best) {
            best = v;
        }
    }
    a.one(Value::num(best))
}

fn m_max(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    m_minmax(ctx, a, "max", true)
}

fn m_min(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    m_minmax(ctx, a, "min", false)
}

fn m_random(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    // Deterministic xorshift: scripts must not depend on the value.
    let r = ctx.next_random();
    match a.n {
        0 => a.one(Value::num(r)),
        1 => {
            let u = check_num(ctx, &a, 0, "random")?.floor();
            if u < 1.0 {
                return ctx.arg_error(0, "random", "interval is empty");
            }
            a.one(Value::num((r * u).floor() + 1.0))
        }
        _ => {
            let (l, u) = (check_num(ctx, &a, 0, "random")?.floor(), check_num(ctx, &a, 1, "random")?.floor());
            if l > u {
                return ctx.arg_error(1, "random", "interval is empty");
            }
            a.one(Value::num((r * (u - l + 1.0)).floor() + l))
        }
    }
}

fn m_randomseed(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let s = check_num(ctx, &a, 0, "randomseed")?;
    ctx.seed_random(s as u64);
    a.none()
}

// -------------------------------------------------------------------------- coroutine

fn check_thread(ctx: &mut Ctx, a: &Args, f: &str) -> LResult<*mut super::state::Thread> {
    let v = a.get(0);
    if v.tag == tag::THREAD {
        return Ok(v.bits as *mut super::state::Thread);
    }
    ctx.arg_error(0, f, &format!("coroutine expected, got {}", got(a, 0)))
}

fn co_create(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let f = a.get(0);
    if f.tag != tag::FUNC {
        return ctx.arg_error(0, "create", &format!("function expected, got {}", got(&a, 0)));
    }
    let th = ctx.new_coroutine(f);
    a.one(Value { tag: tag::THREAD, bits: th as u64 })
}

fn co_resume(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let th = check_thread(ctx, &a, "resume")?;
    let args: Vec<Value> = (1..a.n).map(|i| a.get(i)).collect();
    match ctx.resume(th, &args) {
        Ok(vals) => {
            let mut out = vec![Value::bool(true)];
            out.extend(vals);
            a.many(ctx, &out)
        }
        Err(e) => {
            if is_fuel_error(ctx) || e.tag == tag::STR && unsafe { e.str_bytes() } == super::FUEL_ERROR.as_bytes() {
                ctx.err = e;
                return Err(Thrown);
            }
            a.many(ctx, &[Value::bool(false), e])
        }
    }
}

fn co_yield(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let vals: Vec<Value> = (0..a.n).map(|i| a.get(i)).collect();
    let back = ctx.yield_now(&vals)?;
    a.many(ctx, &back)
}

fn co_status(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let th = check_thread(ctx, &a, "status")?;
    // SAFETY: live thread.
    let s = match unsafe { (*th).status } {
        super::state::CoStatus::Suspended => "suspended",
        super::state::CoStatus::Running => "running",
        super::state::CoStatus::Normal => "normal",
        super::state::CoStatus::Dead => "dead",
    };
    let v = ctx.str(s.as_bytes());
    a.one(v)
}

fn co_running(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let main = ctx.cur == ctx.main;
    a.many(ctx, &[Value { tag: tag::THREAD, bits: ctx.cur as u64 }, Value::bool(main)])
}

fn wrap_call(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let th = a.data().bits as *mut super::state::Thread;
    let args: Vec<Value> = (0..a.n).map(|i| a.get(i)).collect();
    match ctx.resume(th, &args) {
        Ok(vals) => a.many(ctx, &vals),
        Err(e) => {
            ctx.err = e;
            Err(Thrown)
        }
    }
}

fn co_wrap(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let f = a.get(0);
    if f.tag != tag::FUNC {
        return ctx.arg_error(0, "wrap", &format!("function expected, got {}", got(&a, 0)));
    }
    let th = ctx.new_coroutine(f);
    let w = ctx.new_rust_closure(wrap_call, Value { tag: tag::THREAD, bits: th as u64 });
    a.one(w)
}

// ---------------------------------------------------------------------------- bit32

fn bit_args(ctx: &mut Ctx, a: &Args, f: &str) -> LResult<Vec<f64>> {
    (0..a.n).map(|i| check_num(ctx, a, i, f)).collect()
}

macro_rules! bit_variadic {
    ($name:ident, $lua:literal, $f:path) => {
        fn $name(ctx: &mut Ctx, a: Args) -> LResult<usize> {
            let v = bit_args(ctx, &a, $lua)?;
            a.one(Value::num(f64::from($f(&v))))
        }
    };
}

bit_variadic!(bit_band, "band", b32::band);
bit_variadic!(bit_bor, "bor", b32::bor);
bit_variadic!(bit_bxor, "bxor", b32::bxor);

fn bit_btest(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let v = bit_args(ctx, &a, "btest")?;
    a.one(Value::bool(b32::btest(&v)))
}

macro_rules! bit_unary {
    ($name:ident, $lua:literal, $f:path) => {
        fn $name(ctx: &mut Ctx, a: Args) -> LResult<usize> {
            let x = check_num(ctx, &a, 0, $lua)?;
            a.one(Value::num(f64::from($f(x))))
        }
    };
}

bit_unary!(bit_bnot, "bnot", b32::bnot);

macro_rules! bit_shift {
    ($name:ident, $lua:literal, $f:path) => {
        fn $name(ctx: &mut Ctx, a: Args) -> LResult<usize> {
            let x = check_num(ctx, &a, 0, $lua)?;
            let d = check_num(ctx, &a, 1, $lua)?;
            a.one(Value::num(f64::from($f(x, d))))
        }
    };
}

bit_shift!(bit_lshift, "lshift", b32::lshift);
bit_shift!(bit_rshift, "rshift", b32::rshift);
bit_shift!(bit_arshift, "arshift", b32::arshift);
bit_shift!(bit_lrotate, "lrotate", b32::lrotate);
bit_shift!(bit_rrotate, "rrotate", b32::rrotate);

fn bit_extract(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let n = check_num(ctx, &a, 0, "extract")?;
    let f = check_num(ctx, &a, 1, "extract")?;
    let w = if a.get(2).is_nil() { None } else { Some(check_num(ctx, &a, 2, "extract")?) };
    match b32::extract(n, f, w) {
        Ok(v) => a.one(Value::num(f64::from(v))),
        Err(e) => ctx.throw_lib(&e),
    }
}

fn bit_replace(ctx: &mut Ctx, a: Args) -> LResult<usize> {
    let n = check_num(ctx, &a, 0, "replace")?;
    let v = check_num(ctx, &a, 1, "replace")?;
    let f = check_num(ctx, &a, 2, "replace")?;
    let w = if a.get(3).is_nil() { None } else { Some(check_num(ctx, &a, 3, "replace")?) };
    match b32::replace(n, v, f, w) {
        Ok(r) => a.one(Value::num(f64::from(r))),
        Err(e) => ctx.throw_lib(&e),
    }
}

// ----------------------------------------------------------------------------- install

fn lib(ctx: &mut Ctx, name: &str, fns: &[(&str, RustFn)]) -> *mut Table {
    let t = ctx.heap.new_table(0, fns.len());
    register_fns(ctx, t, fns);
    ctx.set_global(name, Value { tag: tag::TABLE, bits: t as u64 });
    t
}

/// Installs the standard library into the global table.
pub fn install_stdlib(ctx: &mut Ctx) {
    let g = ctx.globals;
    register_fns(
        ctx,
        g,
        &[
            ("print", base_print),
            ("type", base_type),
            ("tostring", base_tostring),
            ("tonumber", base_tonumber),
            ("next", base_next),
            ("pairs", base_pairs),
            ("ipairs", base_ipairs),
            ("rawget", base_rawget),
            ("rawset", base_rawset),
            ("rawequal", base_rawequal),
            ("rawlen", base_rawlen),
            ("setmetatable", base_setmetatable),
            ("getmetatable", base_getmetatable),
            ("select", base_select),
            ("assert", base_assert),
            ("error", base_error),
            ("pcall", base_pcall),
            ("xpcall", base_xpcall),
            ("collectgarbage", base_collectgarbage),
        ],
    );
    let globals_v = Value { tag: tag::TABLE, bits: g as u64 };
    ctx.set_global("_G", globals_v);
    let string = lib(
        ctx,
        "string",
        &[
            ("len", s_len),
            ("sub", s_sub),
            ("upper", s_upper),
            ("lower", s_lower),
            ("reverse", s_reverse),
            ("rep", s_rep),
            ("byte", s_byte),
            ("char", s_char),
            ("find", s_find),
            ("match", s_match),
            ("gmatch", s_gmatch),
            ("gsub", s_gsub),
            ("format", s_format),
        ],
    );
    let meta = ctx.heap.new_table(0, 1);
    let idx = ctx.names.index;
    // SAFETY: fresh tables.
    unsafe { (*meta).set(idx, Value { tag: tag::TABLE, bits: string as u64 }) };
    ctx.string_meta = meta;
    lib(
        ctx,
        "table",
        &[
            ("insert", t_insert),
            ("remove", t_remove),
            ("concat", t_concat),
            ("unpack", t_unpack),
            ("pack", t_pack),
            ("sort", t_sort),
        ],
    );
    let math = lib(
        ctx,
        "math",
        &[
            ("abs", m_abs),
            ("acos", m_acos),
            ("asin", m_asin),
            ("atan", m_atan),
            ("atan2", m_atan2),
            ("ceil", m_ceil),
            ("cos", m_cos),
            ("cosh", m_cosh),
            ("deg", m_deg),
            ("exp", m_exp),
            ("floor", m_floor),
            ("fmod", m_fmod),
            ("frexp", m_frexp),
            ("ldexp", m_ldexp),
            ("log", m_log),
            ("max", m_max),
            ("min", m_min),
            ("modf", m_modf),
            ("pow", m_pow),
            ("rad", m_rad),
            ("random", m_random),
            ("randomseed", m_randomseed),
            ("sin", m_sin),
            ("sinh", m_sinh),
            ("sqrt", m_sqrt),
            ("tan", m_tan),
            ("tanh", m_tanh),
        ],
    );
    for (k, v) in [("pi", std::f64::consts::PI), ("huge", f64::INFINITY)] {
        let key = ctx.str(k.as_bytes());
        // SAFETY: fresh table.
        unsafe { (*math).set(key, Value::num(v)) };
    }
    lib(
        ctx,
        "coroutine",
        &[
            ("create", co_create),
            ("resume", co_resume),
            ("yield", co_yield),
            ("status", co_status),
            ("running", co_running),
            ("wrap", co_wrap),
        ],
    );
    lib(
        ctx,
        "bit32",
        &[
            ("band", bit_band),
            ("bor", bit_bor),
            ("bxor", bit_bxor),
            ("btest", bit_btest),
            ("bnot", bit_bnot),
            ("lshift", bit_lshift),
            ("rshift", bit_rshift),
            ("arshift", bit_arshift),
            ("lrotate", bit_lrotate),
            ("rrotate", bit_rrotate),
            ("extract", bit_extract),
            ("replace", bit_replace),
        ],
    );
}
