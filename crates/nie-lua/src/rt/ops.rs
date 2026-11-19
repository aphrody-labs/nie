//! Entry points that compiled code calls through [`Ctx::helpers`].
//!
//! Every helper is `unsafe extern "C"` and reports failure as a non-zero status (or `-1` for
//! count-returning ones) with the error value stored in [`Ctx::err`]. Operands arrive as
//! pointers into the frame or the constant pool; they are read before any write because the
//! destination may alias an operand.

use super::heap::Closure;
use super::state::{Ctx, HELPER_SLOTS, Thrown, arith_num, rt_call_inner};
use super::table::Table;
use super::value::{Value, tag};

/// Helper indices shared by the code generators.
pub mod id {
    /// `rt_call`.
    pub const CALL: usize = 0;
    /// `rt_gettable`.
    pub const GETTABLE: usize = 1;
    /// `rt_settable`.
    pub const SETTABLE: usize = 2;
    /// `rt_gettabup`.
    pub const GETTABUP: usize = 3;
    /// `rt_settabup`.
    pub const SETTABUP: usize = 4;
    /// `rt_arith`.
    pub const ARITH: usize = 5;
    /// `rt_eq`.
    pub const EQ: usize = 6;
    /// `rt_lt`.
    pub const LT: usize = 7;
    /// `rt_le`.
    pub const LE: usize = 8;
    /// `rt_len`.
    pub const LEN: usize = 9;
    /// `rt_concat`.
    pub const CONCAT: usize = 10;
    /// `rt_closure`.
    pub const CLOSURE: usize = 11;
    /// `rt_close`.
    pub const CLOSE: usize = 12;
    /// `rt_newtable`.
    pub const NEWTABLE: usize = 13;
    /// `rt_setlist`.
    pub const SETLIST: usize = 14;
    /// `rt_self`.
    pub const SELF: usize = 15;
    /// `rt_forprep`.
    pub const FORPREP: usize = 16;
    /// `rt_tforcall`.
    pub const TFORCALL: usize = 17;
    /// `rt_vararg`.
    pub const VARARG: usize = 18;
    /// `rt_vprologue`.
    pub const VPROLOGUE: usize = 19;
    /// `rt_nilfill`.
    pub const NILFILL: usize = 20;
    /// `rt_out_of_stack`.
    pub const OUT_OF_STACK: usize = 21;
    /// `rt_out_of_fuel`.
    pub const OUT_OF_FUEL: usize = 22;
    /// `rt_fixresults`.
    pub const FIXRESULTS: usize = 23;
    /// `rt_annotate`.
    pub const ANNOTATE: usize = 24;
    /// Number of helpers.
    pub const COUNT: usize = 25;
}

/// Names of the helpers in index order, as used by the Rust source backend.
pub const HELPER_NAMES: [&str; id::COUNT] = [
    "rt_call_op",
    "rt_gettable",
    "rt_settable",
    "rt_gettabup",
    "rt_settabup",
    "rt_arith",
    "rt_eq",
    "rt_lt",
    "rt_le",
    "rt_len",
    "rt_concat",
    "rt_closure",
    "rt_close",
    "rt_newtable",
    "rt_setlist",
    "rt_self",
    "rt_forprep",
    "rt_tforcall",
    "rt_vararg",
    "rt_vprologue",
    "rt_nilfill",
    "rt_out_of_stack",
    "rt_out_of_fuel",
    "rt_fixresults",
    "rt_annotate",
];

const _: () = assert!(id::COUNT <= HELPER_SLOTS);

/// Fills the helper table of a context.
pub fn install_helpers(ctx: &mut Ctx) {
    let h = &mut ctx.helpers;
    h[id::CALL] = rt_call_op as *const () as usize;
    h[id::GETTABLE] = rt_gettable as *const () as usize;
    h[id::SETTABLE] = rt_settable as *const () as usize;
    h[id::GETTABUP] = rt_gettabup as *const () as usize;
    h[id::SETTABUP] = rt_settabup as *const () as usize;
    h[id::ARITH] = rt_arith as *const () as usize;
    h[id::EQ] = rt_eq as *const () as usize;
    h[id::LT] = rt_lt as *const () as usize;
    h[id::LE] = rt_le as *const () as usize;
    h[id::LEN] = rt_len as *const () as usize;
    h[id::CONCAT] = rt_concat as *const () as usize;
    h[id::CLOSURE] = rt_closure as *const () as usize;
    h[id::CLOSE] = rt_close as *const () as usize;
    h[id::NEWTABLE] = rt_newtable as *const () as usize;
    h[id::SETLIST] = rt_setlist as *const () as usize;
    h[id::SELF] = rt_self as *const () as usize;
    h[id::FORPREP] = rt_forprep as *const () as usize;
    h[id::TFORCALL] = rt_tforcall as *const () as usize;
    h[id::VARARG] = rt_vararg as *const () as usize;
    h[id::VPROLOGUE] = rt_vprologue as *const () as usize;
    h[id::NILFILL] = rt_nilfill as *const () as usize;
    h[id::OUT_OF_STACK] = rt_out_of_stack as *const () as usize;
    h[id::OUT_OF_FUEL] = rt_out_of_fuel as *const () as usize;
    h[id::FIXRESULTS] = rt_fixresults as *const () as usize;
    h[id::ANNOTATE] = rt_annotate as *const () as usize;
}

#[inline]
fn status(r: Result<(), Thrown>) -> i32 {
    i32::from(r.is_err())
}

/// `dst = obj[key]`.
///
/// # Safety
/// All pointers must be valid slots; `top` the frame top of the caller.
pub unsafe extern "C" fn rt_gettable(
    ctx: *mut Ctx,
    top: *mut Value,
    obj: *const Value,
    key: *const Value,
    dst: *mut Value,
) -> i32 {
    // SAFETY: caller contract.
    unsafe {
        (*ctx).lua_op = true;
        let (o, k) = (*obj, *key);
        if o.tag == tag::TABLE {
            let t = &*(o.bits as *const Table);
            let v = t.get(k);
            if !v.is_nil() || t.meta.is_null() {
                *dst = v;
                return 0;
            }
        }
        match (*ctx).index(top, o, k) {
            Ok(v) => {
                *dst = v;
                0
            }
            Err(Thrown) => 1,
        }
    }
}

/// `obj[key] = val`.
///
/// # Safety
/// As [`rt_gettable`].
pub unsafe extern "C" fn rt_settable(
    ctx: *mut Ctx,
    top: *mut Value,
    obj: *const Value,
    key: *const Value,
    val: *const Value,
) -> i32 {
    // SAFETY: caller contract.
    unsafe {
        (*ctx).lua_op = true;
        status((*ctx).set_index(top, *obj, *key, *val))
    }
}

unsafe fn upvalue(cl: *const Closure, up: u32) -> Value {
    // SAFETY: caller contract: `up < nup`.
    unsafe { *(**(*cl).upvals.add(up as usize)).v }
}

/// `dst = UpValue[up][key]` (global reads).
///
/// # Safety
/// As [`rt_gettable`]; `cl` the running closure.
pub unsafe extern "C" fn rt_gettabup(
    ctx: *mut Ctx,
    top: *mut Value,
    cl: *const Closure,
    up: u32,
    key: *const Value,
    dst: *mut Value,
) -> i32 {
    // SAFETY: caller contract.
    unsafe {
        (*ctx).lua_op = true;
        let o = upvalue(cl, up);
        let k = *key;
        if o.tag == tag::TABLE {
            let t = &*(o.bits as *const Table);
            let v = if k.tag == tag::STR { t.get_str(k) } else { t.get(k) };
            if !v.is_nil() || t.meta.is_null() {
                *dst = v;
                return 0;
            }
        }
        match (*ctx).index(top, o, k) {
            Ok(v) => {
                *dst = v;
                0
            }
            Err(Thrown) => 1,
        }
    }
}

/// `UpValue[up][key] = val` (global writes).
///
/// # Safety
/// As [`rt_gettabup`].
pub unsafe extern "C" fn rt_settabup(
    ctx: *mut Ctx,
    top: *mut Value,
    cl: *const Closure,
    up: u32,
    key: *const Value,
    val: *const Value,
) -> i32 {
    // SAFETY: caller contract.
    unsafe {
        (*ctx).lua_op = true;
        status((*ctx).set_index(top, upvalue(cl, up), *key, *val))
    }
}

/// `dst = b op c` with coercions and metamethods (`op` per [`super::state::arith`]).
///
/// # Safety
/// As [`rt_gettable`].
pub unsafe extern "C" fn rt_arith(
    ctx: *mut Ctx,
    top: *mut Value,
    op: u32,
    dst: *mut Value,
    b: *const Value,
    c: *const Value,
) -> i32 {
    // SAFETY: caller contract.
    unsafe {
        let (x, y) = (*b, *c);
        if x.tag == tag::NUM && y.tag == tag::NUM {
            *dst = Value::num(arith_num(op, x.as_num(), y.as_num()));
            return 0;
        }
        (*ctx).lua_op = true;
        match (*ctx).arith(top, op, x, y) {
            Ok(v) => {
                *dst = v;
                0
            }
            Err(Thrown) => 1,
        }
    }
}

/// `b == c`: 1, 0, or -1 on error.
///
/// # Safety
/// As [`rt_gettable`].
pub unsafe extern "C" fn rt_eq(
    ctx: *mut Ctx,
    top: *mut Value,
    b: *const Value,
    c: *const Value,
) -> i32 {
    // SAFETY: caller contract.
    unsafe {
        match (*ctx).equals(top, *b, *c) {
            Ok(r) => i32::from(r),
            Err(Thrown) => -1,
        }
    }
}

/// `b < c`: 1, 0, or -1 on error.
///
/// # Safety
/// As [`rt_gettable`].
pub unsafe extern "C" fn rt_lt(
    ctx: *mut Ctx,
    top: *mut Value,
    b: *const Value,
    c: *const Value,
) -> i32 {
    // SAFETY: caller contract.
    unsafe {
        match (*ctx).less_than(top, *b, *c) {
            Ok(r) => i32::from(r),
            Err(Thrown) => -1,
        }
    }
}

/// `b <= c`: 1, 0, or -1 on error.
///
/// # Safety
/// As [`rt_gettable`].
pub unsafe extern "C" fn rt_le(
    ctx: *mut Ctx,
    top: *mut Value,
    b: *const Value,
    c: *const Value,
) -> i32 {
    // SAFETY: caller contract.
    unsafe {
        match (*ctx).less_equal(top, *b, *c) {
            Ok(r) => i32::from(r),
            Err(Thrown) => -1,
        }
    }
}

/// `dst = #src`.
///
/// # Safety
/// As [`rt_gettable`].
pub unsafe extern "C" fn rt_len(
    ctx: *mut Ctx,
    top: *mut Value,
    dst: *mut Value,
    src: *const Value,
) -> i32 {
    // SAFETY: caller contract.
    unsafe {
        (*ctx).lua_op = true;
        match (*ctx).length(top, *src) {
            Ok(v) => {
                *dst = v;
                0
            }
            Err(Thrown) => 1,
        }
    }
}

/// `dst = first[0] .. first[1] .. ... .. first[count-1]`.
///
/// # Safety
/// As [`rt_gettable`]; `first` points at `count` initialised slots.
pub unsafe extern "C" fn rt_concat(
    ctx: *mut Ctx,
    top: *mut Value,
    first: *const Value,
    count: u32,
    dst: *mut Value,
) -> i32 {
    // SAFETY: caller contract.
    unsafe {
        (*ctx).lua_op = true;
        let vals = std::slice::from_raw_parts(first, count as usize).to_vec();
        match (*ctx).concat(top, &vals) {
            Ok(v) => {
                *dst = v;
                0
            }
            Err(Thrown) => 1,
        }
    }
}

/// Instantiates nested prototype `idx` of the running closure into `dst`.
///
/// # Safety
/// `cl` is the running Lua closure, `base` its frame base.
pub unsafe extern "C" fn rt_closure(
    ctx: *mut Ctx,
    cl: *const Closure,
    idx: u32,
    base: *mut Value,
    dst: *mut Value,
) -> i32 {
    // SAFETY: caller contract.
    unsafe {
        let c = &mut *ctx;
        let child = (&(*(*cl).proto).children)[idx as usize];
        let mut ups = Vec::with_capacity((*child).upvals.len());
        for &(in_stack, index) in &(*child).upvals {
            ups.push(if in_stack {
                c.find_upval(base.add(index as usize))
            } else {
                *(*cl).upvals.add(index as usize)
            });
        }
        let nc = c.heap.new_lua_closure(child, ups);
        *dst = Value {
            tag: tag::FUNC,
            bits: nc as u64,
        };
        0
    }
}

/// Closes open upvalues at or above `level`.
///
/// # Safety
/// `level` points into the running thread's stack.
pub unsafe extern "C" fn rt_close(ctx: *mut Ctx, level: *mut Value) {
    // SAFETY: caller contract.
    unsafe { (*ctx).close_upvals(level) }
}

/// `dst = {}` with size hints.
///
/// # Safety
/// `dst` is a valid slot.
pub unsafe extern "C" fn rt_newtable(ctx: *mut Ctx, dst: *mut Value, narr: u32, nhash: u32) {
    // SAFETY: caller contract.
    unsafe { *dst = (*ctx).new_table(narr as usize, nhash as usize) }
}

/// `tbl[start + i] = first[i]` for `i < n` (`OP_SETLIST`).
///
/// # Safety
/// `tbl` holds a table; `first` points at `n` initialised slots.
pub unsafe extern "C" fn rt_setlist(
    _ctx: *mut Ctx,
    tbl: *const Value,
    first: *const Value,
    n: u32,
    start: u32,
) {
    // SAFETY: caller contract.
    unsafe {
        let t = &mut *((*tbl).bits as *mut Table);
        let vals = std::slice::from_raw_parts(first, n as usize);
        t.set_array_from(start as usize + 1, vals);
    }
}

/// `ra[1] = obj; ra[0] = obj[key]`.
///
/// # Safety
/// As [`rt_gettable`]; `ra` points at two consecutive slots.
pub unsafe extern "C" fn rt_self(
    ctx: *mut Ctx,
    top: *mut Value,
    ra: *mut Value,
    obj: *const Value,
    key: *const Value,
) -> i32 {
    // SAFETY: caller contract.
    unsafe {
        (*ctx).lua_op = true;
        let (o, k) = (*obj, *key);
        *ra.add(1) = o;
        match (*ctx).index(top, o, k) {
            Ok(v) => {
                *ra = v;
                0
            }
            Err(Thrown) => 1,
        }
    }
}

/// Normalises the three loop slots to numbers and performs `ra -= step`.
///
/// # Safety
/// `ra` points at three consecutive slots.
pub unsafe extern "C" fn rt_forprep(ctx: *mut Ctx, ra: *mut Value) -> i32 {
    // SAFETY: caller contract.
    unsafe {
        let c = &mut *ctx;
        let names = ["initial value", "limit", "step"];
        let mut n = [0.0; 3];
        for i in 0..3 {
            match c.to_number(*ra.add(i)) {
                Some(x) => n[i] = x,
                None => {
                    let _ = c.throw::<()>(&format!("'for' {} must be a number", names[i]));
                    return 1;
                }
            }
        }
        *ra = Value::num(n[0] - n[2]);
        *ra.add(1) = Value::num(n[1]);
        *ra.add(2) = Value::num(n[2]);
        0
    }
}

/// Generic `for` call: `ra[3..3+nres] = ra[0](ra[1], ra[2])`.
///
/// # Safety
/// `ra` points at the generic-for control slots with room for the results.
pub unsafe extern "C" fn rt_tforcall(ctx: *mut Ctx, ra: *mut Value, nres: u32) -> i32 {
    // SAFETY: caller contract.
    unsafe {
        (*ctx).lua_op = true;
        *ra.add(3) = *ra;
        *ra.add(4) = *ra.add(1);
        *ra.add(5) = *ra.add(2);
        (*ctx).site_cl = std::ptr::null();
        let n = rt_call_inner(ctx, ra.add(3), 2);
        if n < 0 {
            return 1;
        }
        for i in n as usize..nres as usize {
            *ra.add(3 + i) = Value::NIL;
        }
        0
    }
}

/// Copies varargs to `dst`: all of them when `want < 0` (returns the count), else `want` values
/// padded with nil. Returns -1 on stack overflow.
///
/// # Safety
/// `src` points at `nvar` varargs; `dst` at room for the copy.
pub unsafe extern "C" fn rt_vararg(
    ctx: *mut Ctx,
    dst: *mut Value,
    src: *const Value,
    nvar: u32,
    want: i32,
) -> i32 {
    // SAFETY: caller contract.
    unsafe {
        let n = if want < 0 { nvar as usize } else { want as usize };
        if (*ctx).ensure_stack(dst, n + 1).is_err() {
            return -1;
        }
        for i in 0..n {
            *dst.add(i) = if i < nvar as usize {
                *src.add(i)
            } else {
                Value::NIL
            };
        }
        n as i32
    }
}

/// Prologue of vararg functions: moves the fixed parameters above the extra arguments and
/// returns the new frame base, or null on stack overflow.
///
/// # Safety
/// `func[1..=nargs]` are the arguments.
pub unsafe extern "C" fn rt_vprologue(
    ctx: *mut Ctx,
    func: *mut Value,
    nargs: u32,
    nparams: u32,
    maxstack: u32,
) -> *mut Value {
    // SAFETY: caller contract.
    unsafe {
        let base = func.add(1 + nargs as usize);
        if (*ctx)
            .ensure_stack(base, maxstack as usize + 1)
            .is_err()
        {
            return std::ptr::null_mut();
        }
        for i in 0..nparams as usize {
            if i < nargs as usize {
                *base.add(i) = *func.add(1 + i);
                *func.add(1 + i) = Value::NIL;
            } else {
                *base.add(i) = Value::NIL;
            }
        }
        base
    }
}

/// Sets `base[from..to]` to nil.
///
/// # Safety
/// The range is inside the frame.
pub unsafe extern "C" fn rt_nilfill(base: *mut Value, from: u32, to: u32) {
    // SAFETY: caller contract.
    unsafe {
        for i in from..to {
            *base.add(i as usize) = Value::NIL;
        }
    }
}

/// Raises the stack overflow error. Returns 1.
///
/// # Safety
/// `ctx` is live.
pub unsafe extern "C" fn rt_out_of_stack(ctx: *mut Ctx) -> i32 {
    // SAFETY: caller contract.
    unsafe {
        let _ = (*ctx).throw::<()>("stack overflow");
    }
    1
}

/// Raises the budget error. Returns 1.
///
/// # Safety
/// `ctx` is live.
pub unsafe extern "C" fn rt_out_of_fuel(ctx: *mut Ctx) -> i32 {
    // SAFETY: caller contract.
    unsafe {
        let c = &mut *ctx;
        c.fuel = i64::MAX / 2;
        c.err = c.heap.intern(super::FUEL_ERROR.as_bytes());
    }
    1
}

/// Pads `ra[got..want]` with nil after a call returned fewer results than wanted.
///
/// # Safety
/// The range is inside the frame.
pub unsafe extern "C" fn rt_fixresults(ra: *mut Value, got: u32, want: u32) {
    // SAFETY: caller contract.
    unsafe {
        for i in got..want {
            *ra.add(i as usize) = Value::NIL;
        }
    }
}

/// Entry of `OP_CALL` from compiled code: calls like [`rt_call`] with type errors attributed
/// to the Lua operation.
///
/// # Safety
/// As [`rt_call`].
pub unsafe extern "C" fn rt_call_op(
    ctx: *mut Ctx,
    func: *mut Value,
    nargs: u32,
    cl: *const Closure,
    pc: u32,
) -> i32 {
    // SAFETY: caller contract.
    unsafe {
        (*ctx).lua_op = true;
        (*ctx).site_cl = cl;
        (*ctx).site_pc = pc;
        rt_call_inner(ctx, func, nargs)
    }
}

/// Appends the variable description of operation `pc` to a pending type error.
///
/// # Safety
/// `cl` is the running Lua closure.
pub unsafe extern "C" fn rt_annotate(ctx: *mut Ctx, cl: *const Closure, pc: u32) {
    // SAFETY: caller contract.
    unsafe {
        let c = &mut *ctx;
        if c.pending.is_none() {
            return;
        }
        let proto = &*(*cl).proto;
        let Ok(i) = proto.sites.binary_search_by_key(&pc, |s| s.0) else {
            c.pending = None;
            return;
        };
        let desc = proto.sites[i].1.get(c.blame as usize).and_then(Option::clone);
        match desc {
            Some(d) => c.annotate(&d),
            None => c.pending = None,
        }
    }
}
