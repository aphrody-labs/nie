//! Interpreter-free execution state: threads, call machinery, metamethods and the semantic core
//! of every Lua operation that compiled code reaches through the helper table.

use corosensei::stack::DefaultStack;
use corosensei::{Coroutine, CoroutineResult, Yielder};

use super::heap::{Closure, Heap, ProtoRt, UpVal};
use super::lstr;
use super::table::Table;
use super::value::{LStr, Value, tag};

/// Signature of every callable: compiled Lua functions, library functions and host functions.
///
/// `func` points at the slot holding the callee; arguments are at `func[1..=nargs]`. The callee
/// writes its results from `func[0]` upward and returns their count, or `-1` after storing the
/// error value in [`Ctx::err`].
pub type NativeFn = unsafe extern "C" fn(ctx: *mut Ctx, func: *mut Value, nargs: u32) -> i32;

/// A library or host function written in Rust.
pub type RustFn = fn(&mut Ctx, Args) -> LResult<usize>;

/// Marker for a thrown Lua error; the value is in [`Ctx::err`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thrown;

/// Result of an operation that may raise a Lua error.
pub type LResult<T> = Result<T, Thrown>;

/// Number of slots reserved for the helper table of [`Ctx`].
pub const HELPER_SLOTS: usize = 64;

/// Extra stack slots every function may assume above its frame (like `LUA_MINSTACK`).
pub const STACK_SLACK: usize = 24;

/// Argument window handed to Rust functions.
#[derive(Clone, Copy)]
pub struct Args {
    /// The callee slot; results are written from here.
    pub func: *mut Value,
    /// Number of arguments.
    pub n: usize,
}

impl Args {
    /// Argument `i` (0-based), `nil` when absent.
    #[inline]
    #[must_use]
    pub fn get(&self, i: usize) -> Value {
        if i < self.n {
            // SAFETY: arguments live at `func[1..=n]`.
            unsafe { *self.func.add(1 + i) }
        } else {
            Value::NIL
        }
    }

    /// The value bound to the running native closure.
    #[must_use]
    pub fn data(&self) -> Value {
        // SAFETY: `func[0]` is the running closure.
        unsafe { (*((*self.func).bits as *const Closure)).data }
    }

    /// First free slot above the arguments, for nested calls.
    #[inline]
    #[must_use]
    pub fn scratch(&self) -> *mut Value {
        // SAFETY: still inside the stack by the slack contract.
        unsafe { self.func.add(1 + self.n) }
    }

    /// Returns no value.
    #[inline]
    #[allow(clippy::unnecessary_wraps)]
    pub fn none(&self) -> LResult<usize> {
        Ok(0)
    }

    /// Returns one value.
    #[inline]
    #[allow(clippy::unnecessary_wraps)]
    pub fn one(&self, v: Value) -> LResult<usize> {
        // SAFETY: `func[0]` is the callee slot, always writable.
        unsafe { *self.func = v };
        Ok(1)
    }

    /// Returns several values.
    pub fn many(&self, ctx: &mut Ctx, vals: &[Value]) -> LResult<usize> {
        ctx.ensure_stack(self.func, vals.len())?;
        for (i, v) in vals.iter().enumerate() {
            // SAFETY: bounds ensured above.
            unsafe { *self.func.add(i) = *v };
        }
        Ok(vals.len())
    }
}

/// Status of a coroutine.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CoStatus {
    /// Created or yielded.
    Suspended,
    /// Currently running.
    Running,
    /// Resumed another coroutine.
    Normal,
    /// Finished or failed.
    Dead,
}

type Co = Coroutine<(), (), bool, DefaultStack>;

/// A Lua thread: value stack, open upvalues and, for coroutines, a native stack.
pub struct Thread {
    /// Value stack; never reallocated.
    pub stack: Box<[Value]>,
    /// Open upvalues sorted by stack address.
    pub open: Vec<*mut UpVal>,
    /// Coroutine status.
    pub status: CoStatus,
    /// Native recursion bound.
    pub depth: u32,
    /// Bound for `depth`.
    pub depth_limit: u32,
    /// Values travelling across resume and yield.
    pub transfer: Vec<Value>,
    co: Option<Co>,
    yielder: *const Yielder<(), ()>,
    /// Thread that resumed this one.
    pub resumer: *mut Thread,
}

impl Thread {
    fn new(slots: usize, depth_limit: u32) -> Box<Self> {
        Box::new(Self {
            stack: vec![Value::NIL; slots].into_boxed_slice(),
            open: Vec::new(),
            status: CoStatus::Suspended,
            depth: 0,
            depth_limit,
            transfer: Vec::new(),
            co: None,
            yielder: std::ptr::null(),
            resumer: std::ptr::null_mut(),
        })
    }

    /// Releases the native stack without unwinding: compiled frames carry no unwind tables.
    pub fn release(&mut self) {
        if let Some(mut co) = self.co.take() {
            if !co.done() {
                // SAFETY: the suspended frames are abandoned, never resumed.
                unsafe { co.force_reset() };
            }
        }
    }

    fn limit(&mut self) -> *mut Value {
        // SAFETY: one-past-the-end pointer of the stack allocation minus the slack.
        unsafe {
            self.stack
                .as_mut_ptr()
                .add(self.stack.len().saturating_sub(STACK_SLACK))
        }
    }
}

/// Names of metamethods, interned once.
pub struct Names {
    /// `__index`.
    pub index: Value,
    /// `__newindex`.
    pub newindex: Value,
    /// `__call`.
    pub call: Value,
    /// `__eq`.
    pub eq: Value,
    /// `__lt`.
    pub lt: Value,
    /// `__le`.
    pub le: Value,
    /// `__len`.
    pub len: Value,
    /// `__concat`.
    pub concat: Value,
    /// `__unm`.
    pub unm: Value,
    /// Arithmetic: `__add`, `__sub`, `__mul`, `__div`, `__mod`, `__pow`.
    pub arith: [Value; 6],
    /// `__tostring`.
    pub tostring: Value,
    /// `__metatable`.
    pub metatable: Value,
}

/// Arithmetic operator codes shared by compiled code and helpers.
pub mod arith {
    /// `+`.
    pub const ADD: u32 = 0;
    /// `-`.
    pub const SUB: u32 = 1;
    /// `*`.
    pub const MUL: u32 = 2;
    /// `/`.
    pub const DIV: u32 = 3;
    /// `%`.
    pub const MOD: u32 = 4;
    /// `^`.
    pub const POW: u32 = 5;
    /// Unary minus.
    pub const UNM: u32 = 6;
}

/// Floating point `%` of Lua 5.2: `a - floor(a / b) * b`.
#[inline]
#[must_use]
pub fn lua_mod(a: f64, b: f64) -> f64 {
    a - (a / b).floor() * b
}

/// Applies an arithmetic operator to two numbers.
#[inline]
#[must_use]
pub fn arith_num(op: u32, a: f64, b: f64) -> f64 {
    match op {
        arith::ADD => a + b,
        arith::SUB => a - b,
        arith::MUL => a * b,
        arith::DIV => a / b,
        arith::MOD => lua_mod(a, b),
        arith::POW => a.powf(b),
        _ => -a,
    }
}

/// Execution context of one script run.
#[repr(C)]
pub struct Ctx {
    /// Helper entry points called by generated code (must stay the first field).
    pub helpers: [usize; HELPER_SLOTS],
    /// Remaining back-edge and call budget; negative aborts the run.
    pub fuel: i64,
    /// Highest usable stack slot of the running thread.
    pub stack_limit: *mut Value,
    /// Objects of the run.
    pub heap: Heap,
    /// The global table.
    pub globals: *mut Table,
    /// Metatable shared by every string.
    pub string_meta: *mut Table,
    /// Value of the error being thrown.
    pub err: Value,
    /// The main thread.
    pub main: *mut Thread,
    /// The running thread.
    pub cur: *mut Thread,
    /// Interned metamethod names.
    pub names: Names,
    /// Text written by `print` and friends.
    pub out: Vec<u8>,
    /// Observable events (host calls) in order.
    pub trace: Vec<String>,
    /// Set while a Lua operation (not a native function) is executing, so that its type
    /// errors carry the variable description.
    pub lua_op: bool,
    /// Operand index the pending type error blames.
    pub blame: u32,
    /// A type error raised by a Lua operation waits for its site description: the operation
    /// and the type name of the faulty value.
    pub pending: Option<(String, &'static str)>,
    /// Closure and bytecode index of the Lua call that is running a native function, or null.
    pub site_cl: *const Closure,
    /// See [`Ctx::site_cl`].
    pub site_pc: u32,
    rng: u64,
}

/// Depth bound of the main thread.
const MAIN_DEPTH: u32 = 12_000;
/// Value stack of the main thread, in slots.
const MAIN_SLOTS: usize = 1 << 20;
/// Value stack of a coroutine, in slots.
const CO_SLOTS: usize = 1 << 15;
/// Native stack of a coroutine.
const CO_NATIVE: usize = 2 << 20;
/// Native stack of the main run.
const MAIN_NATIVE: usize = 128 << 20;
/// Depth bound of a coroutine.
const CO_DEPTH: u32 = 900;

impl Ctx {
    /// A context with an empty global table and the helper table installed.
    #[must_use]
    pub fn new() -> Box<Self> {
        let mut heap = Heap::new();
        let n = |h: &mut Heap, s: &str| h.intern(s.as_bytes());
        let names = Names {
            index: n(&mut heap, "__index"),
            newindex: n(&mut heap, "__newindex"),
            call: n(&mut heap, "__call"),
            eq: n(&mut heap, "__eq"),
            lt: n(&mut heap, "__lt"),
            le: n(&mut heap, "__le"),
            len: n(&mut heap, "__len"),
            concat: n(&mut heap, "__concat"),
            unm: n(&mut heap, "__unm"),
            arith: [
                n(&mut heap, "__add"),
                n(&mut heap, "__sub"),
                n(&mut heap, "__mul"),
                n(&mut heap, "__div"),
                n(&mut heap, "__mod"),
                n(&mut heap, "__pow"),
            ],
            tostring: n(&mut heap, "__tostring"),
            metatable: n(&mut heap, "__metatable"),
        };
        let globals = heap.new_table(0, 64);
        let mut main = Thread::new(MAIN_SLOTS, MAIN_DEPTH);
        main.status = CoStatus::Running;
        let limit = main.limit();
        let main_ptr: *mut Thread = &mut *main;
        heap.threads.push(main);
        let mut ctx = Box::new(Self {
            helpers: [0; HELPER_SLOTS],
            fuel: i64::MAX,
            stack_limit: limit,
            heap,
            globals,
            string_meta: std::ptr::null_mut(),
            err: Value::NIL,
            main: main_ptr,
            cur: main_ptr,
            names,
            out: Vec::new(),
            trace: Vec::new(),
            lua_op: false,
            blame: 0,
            pending: None,
            site_cl: std::ptr::null(),
            site_pc: 0,
            rng: 0x2545_f491_4f6c_dd1d,
        });
        super::ops::install_helpers(&mut ctx);
        ctx
    }

    /// Next value of the deterministic generator behind `math.random`, in `[0, 1)`.
    pub fn next_random(&mut self) -> f64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        (x >> 11) as f64 / (1_u64 << 53) as f64
    }

    /// Seeds the generator behind `math.random`.
    pub fn seed_random(&mut self, seed: u64) {
        self.rng = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
    }

    /// Interns a string.
    pub fn str(&mut self, s: &[u8]) -> Value {
        self.heap.intern(s)
    }

    /// A fresh table value.
    pub fn new_table(&mut self, narr: usize, nhash: usize) -> Value {
        Value {
            tag: tag::TABLE,
            bits: self.heap.new_table(narr, nhash) as u64,
        }
    }

    /// A native function value wrapping `f`.
    pub fn new_rust_fn(&mut self, f: RustFn) -> Value {
        self.new_rust_closure(f, Value::NIL)
    }

    /// A native function value wrapping `f` with bound data.
    pub fn new_rust_closure(&mut self, f: RustFn, data: Value) -> Value {
        let c = self.heap.new_native(rust_trampoline, Some(f), data);
        Value {
            tag: tag::FUNC,
            bits: c as u64,
        }
    }

    /// Sets a global.
    pub fn set_global(&mut self, name: &str, v: Value) {
        let k = self.heap.intern(name.as_bytes());
        // SAFETY: globals is a live table.
        unsafe { (*self.globals).set(k, v) };
    }

    /// Reads a global.
    pub fn get_global(&mut self, name: &str) -> Value {
        let k = self.heap.intern(name.as_bytes());
        // SAFETY: globals is a live table.
        unsafe { (*self.globals).get_str(k) }
    }

    /// Throws a runtime error raised by the VM semantics (`luaG_runerror` shape).
    pub fn throw<T>(&mut self, msg: &str) -> LResult<T> {
        let text = format!("{}{msg}", super::ERROR_POS);
        self.err = self.heap.intern(text.as_bytes());
        Err(Thrown)
    }

    /// Throws an error raised by a library function (`luaL_error` shape).
    pub fn throw_lib<T>(&mut self, msg: &str) -> LResult<T> {
        self.err = self.heap.intern(msg.as_bytes());
        Err(Thrown)
    }

    /// Text of `luaL_argerror` for argument `i` (0-based) of native `fname`.
    pub fn arg_message(&mut self, i: usize, fname: &str, msg: &str) -> String {
        let (name, method) = self.native_name(fname);
        let mut narg = i + 1;
        if method {
            narg -= 1;
            if narg == 0 {
                return format!("calling '{name}' on bad self ({msg})");
            }
        }
        format!("bad argument #{narg} to '{name}' ({msg})")
    }

    /// Throws `bad argument #i to 'name' (msg)` like `luaL_argerror`.
    pub fn arg_error<T>(&mut self, i: usize, fname: &str, msg: &str) -> LResult<T> {
        let text = self.arg_message(i, fname, msg);
        self.throw_lib(&text)
    }

    /// Checks that `n` slots above `from` exist.
    pub fn ensure_stack(&mut self, from: *mut Value, n: usize) -> LResult<()> {
        // SAFETY: pointer arithmetic only, compared against the thread limit.
        if unsafe { from.add(n) } > self.stack_limit {
            return self.throw("stack overflow");
        }
        Ok(())
    }

    // ------------------------------------------------------------------ type helpers

    /// Metatable of a value, null when none.
    #[must_use]
    pub fn metatable_of(&self, v: Value) -> *mut Table {
        match v.tag {
            // SAFETY: tables and strings are live heap objects.
            tag::TABLE => unsafe { (*(v.bits as *mut Table)).meta },
            tag::STR => self.string_meta,
            _ => std::ptr::null_mut(),
        }
    }

    /// Metamethod `name` of `v`, `nil` when absent.
    #[must_use]
    pub fn metamethod(&self, v: Value, name: Value) -> Value {
        let mt = self.metatable_of(v);
        if mt.is_null() {
            Value::NIL
        } else {
            // SAFETY: live table.
            unsafe { (*mt).get_str(name) }
        }
    }

    /// `tonumber` coercion used by arithmetic.
    #[must_use]
    pub fn to_number(&self, v: Value) -> Option<f64> {
        match v.tag {
            tag::NUM => Some(v.as_num()),
            // SAFETY: STR tag.
            tag::STR => lstr::str_to_number(unsafe { v.str_bytes() }),
            _ => None,
        }
    }

    /// `tostring` without metamethods, as used by concatenation and `print`.
    pub fn to_str_raw(&mut self, v: Value) -> Value {
        match v.tag {
            tag::STR => v,
            tag::NUM => {
                let s = lstr::number_to_string(v.as_num());
                self.heap.intern(&s)
            }
            tag::NIL => self.heap.intern(b"nil"),
            tag::BOOL => self.heap.intern(if v.bits != 0 { b"true" } else { b"false" }),
            _ => {
                let s = format!("{}: 0x{:014x}", v.type_name(), v.bits);
                self.heap.intern(s.as_bytes())
            }
        }
    }

    /// `tostring` with `__tostring`.
    pub fn tostring(&mut self, scratch: *mut Value, v: Value) -> LResult<Value> {
        let mm = self.metamethod(v, self.names.tostring);
        if !mm.is_nil() {
            let r = self.call1(scratch, mm, &[v])?;
            if r.tag != tag::STR {
                return self.throw_lib("'__tostring' must return a string");
            }
            return Ok(r);
        }
        Ok(self.to_str_raw(v))
    }

    // ------------------------------------------------------------------ calls

    /// Calls `f(args)` using `scratch` as the callee slot and returns the first result.
    pub fn call1(&mut self, scratch: *mut Value, f: Value, args: &[Value]) -> LResult<Value> {
        let n = self.call_at(scratch, f, args)?;
        // SAFETY: results were written from `scratch[0]`.
        Ok(if n == 0 { Value::NIL } else { unsafe { *scratch } })
    }

    /// Calls `f(args)` and returns every result.
    pub fn call_n(&mut self, scratch: *mut Value, f: Value, args: &[Value]) -> LResult<Vec<Value>> {
        let n = self.call_at(scratch, f, args)?;
        // SAFETY: results were written from `scratch[0]`.
        Ok((0..n).map(|i| unsafe { *scratch.add(i) }).collect())
    }

    /// Places `f` and `args` at `scratch` and calls; results start at `scratch[0]`.
    pub fn call_at(&mut self, scratch: *mut Value, f: Value, args: &[Value]) -> LResult<usize> {
        self.ensure_stack(scratch, args.len() + 2)?;
        // SAFETY: bounds ensured.
        unsafe {
            *scratch = f;
            for (i, a) in args.iter().enumerate() {
                *scratch.add(1 + i) = *a;
            }
        }
        // SAFETY: `scratch` is a valid callee slot.
        let n = unsafe { rt_call(self, scratch, args.len() as u32) };
        if n < 0 { Err(Thrown) } else { Ok(n as usize) }
    }

    // ------------------------------------------------------------------ operations

    /// `t[k]` with metamethods.
    pub fn index(&mut self, scratch: *mut Value, obj: Value, key: Value) -> LResult<Value> {
        let mut o = obj;
        for _ in 0..100 {
            let mm = if o.tag == tag::TABLE {
                // SAFETY: TABLE tag.
                let t = unsafe { &*(o.bits as *const Table) };
                let v = t.get(key);
                if !v.is_nil() {
                    return Ok(v);
                }
                if t.meta.is_null() {
                    return Ok(Value::NIL);
                }
                // SAFETY: metatable is live.
                let mm = unsafe { (*t.meta).get_str(self.names.index) };
                if mm.is_nil() {
                    return Ok(Value::NIL);
                }
                mm
            } else {
                let mm = self.metamethod(o, self.names.index);
                if mm.is_nil() {
                    return self.type_error(o, "index", 0);
                }
                mm
            };
            if mm.tag == tag::FUNC {
                return self.call1(scratch, mm, &[o, key]);
            }
            o = mm;
        }
        self.throw("loop in gettable")
    }

    /// `t[k] = v` with metamethods.
    pub fn set_index(
        &mut self,
        scratch: *mut Value,
        obj: Value,
        key: Value,
        val: Value,
    ) -> LResult<()> {
        let mut o = obj;
        for _ in 0..100 {
            let mm = if o.tag == tag::TABLE {
                let t = o.bits as *mut Table;
                // SAFETY: TABLE tag.
                let tr = unsafe { &mut *t };
                let mm = if tr.meta.is_null() {
                    Value::NIL
                } else {
                    // SAFETY: live metatable.
                    unsafe { (*tr.meta).get_str(self.names.newindex) }
                };
                if mm.is_nil() || !tr.get(key).is_nil() {
                    return self.raw_set(t, key, val);
                }
                mm
            } else {
                let mm = self.metamethod(o, self.names.newindex);
                if mm.is_nil() {
                    return self.type_error(o, "index", 0);
                }
                mm
            };
            if mm.tag == tag::FUNC {
                self.call_at(scratch, mm, &[o, key, val])?;
                return Ok(());
            }
            o = mm;
        }
        self.throw("loop in settable")
    }

    /// Raw set with key validation.
    pub fn raw_set(&mut self, t: *mut Table, key: Value, val: Value) -> LResult<()> {
        if key.tag == tag::NIL {
            return self.throw("table index is nil");
        }
        if key.tag == tag::NUM && key.as_num().is_nan() {
            return self.throw("table index is NaN");
        }
        // SAFETY: caller passes a live table.
        unsafe { (*t).set(key, val) };
        Ok(())
    }

    /// `attempt to <op> a <type> value`; `blame` is the index of the faulty operand among the
    /// operands described by the operation's site.
    pub fn type_error<T>(&mut self, v: Value, op: &str, blame: u32) -> LResult<T> {
        self.pending = self.lua_op.then(|| (op.to_string(), v.type_name()));
        self.blame = blame;
        self.throw(&format!("attempt to {op} a {} value", v.type_name()))
    }

    /// Rewrites a pending type error with the variable description of its site, in the form of
    /// Lua 5.2: `attempt to index local 'x' (a nil value)`.
    pub fn annotate(&mut self, desc: &str) {
        if let Some((op, ty)) = self.pending.take() {
            let text = format!("{}attempt to {op} {desc} (a {ty} value)", super::ERROR_POS);
            self.err = self.heap.intern(text.as_bytes());
        }
    }

    /// Name of the running native function as `luaL_argerror` reports it: from the Lua call site
    /// when called directly from compiled code, else the qualified global name.
    pub fn native_name(&mut self, fname: &str) -> (String, bool) {
        if !self.site_cl.is_null() {
            // SAFETY: the site closure is live while its frame runs.
            let proto = unsafe { &*(*self.site_cl).proto };
            if let Ok(i) = proto.sites.binary_search_by_key(&self.site_pc, |s| s.0) {
                if let Some(Some(desc)) = proto.sites[i].1.first() {
                    if let Some((kind, rest)) = desc.split_once(' ') {
                        let name = rest.trim_matches('\'').to_string();
                        return (name, kind == "method");
                    }
                }
            }
        }
        for lib in ["string", "table", "math", "coroutine", "bit32"] {
            let key = self.heap.intern(lib.as_bytes());
            // SAFETY: globals is a live table.
            let t = unsafe { (*self.globals).get_str(key) };
            if t.tag == tag::TABLE {
                let k = self.heap.intern(fname.as_bytes());
                // SAFETY: TABLE tag.
                if unsafe { (*(t.bits as *const Table)).get_str(k) }.tag == tag::FUNC {
                    return (format!("{lib}.{fname}"), false);
                }
            }
        }
        (format!("_G.{fname}"), false)
    }

    /// Binary arithmetic with coercions and metamethods.
    pub fn arith(&mut self, scratch: *mut Value, op: u32, a: Value, b: Value) -> LResult<Value> {
        if let (Some(x), Some(y)) = (self.to_number(a), self.to_number(b)) {
            return Ok(Value::num(arith_num(op, x, y)));
        }
        let name = if op == arith::UNM {
            self.names.unm
        } else {
            self.names.arith[op as usize]
        };
        let mut mm = self.metamethod(a, name);
        if mm.is_nil() {
            mm = self.metamethod(b, name);
        }
        if mm.is_nil() {
            // Blame the second operand when the first one is a number (luaG_aritherror).
            let (blame, which) = if self.to_number(a).is_some() { (b, 1) } else { (a, 0) };
            return self.type_error(blame, "perform arithmetic on", which);
        }
        self.call1(scratch, mm, &[a, b])
    }

    /// `a == b` with `__eq`.
    pub fn equals(&mut self, scratch: *mut Value, a: Value, b: Value) -> LResult<bool> {
        if a.tag != b.tag {
            return Ok(false);
        }
        if a.raw_eq(b) {
            return Ok(true);
        }
        if a.tag != tag::TABLE {
            return Ok(false);
        }
        let m1 = self.metamethod(a, self.names.eq);
        let mm = if m1.is_nil() {
            Value::NIL
        } else if self.metatable_of(a) == self.metatable_of(b) {
            m1
        } else {
            let m2 = self.metamethod(b, self.names.eq);
            if m1.raw_eq(m2) { m1 } else { Value::NIL }
        };
        if mm.is_nil() {
            return Ok(false);
        }
        Ok(self.call1(scratch, mm, &[a, b])?.truthy())
    }

    /// `a < b`.
    pub fn less_than(&mut self, scratch: *mut Value, a: Value, b: Value) -> LResult<bool> {
        if a.tag == tag::NUM && b.tag == tag::NUM {
            return Ok(a.as_num() < b.as_num());
        }
        if a.tag == tag::STR && b.tag == tag::STR {
            // SAFETY: STR tags.
            return Ok(unsafe { a.str_bytes() < b.str_bytes() });
        }
        self.order_mm(scratch, a, b, self.names.lt)
    }

    /// `a <= b`.
    pub fn less_equal(&mut self, scratch: *mut Value, a: Value, b: Value) -> LResult<bool> {
        if a.tag == tag::NUM && b.tag == tag::NUM {
            return Ok(a.as_num() <= b.as_num());
        }
        if a.tag == tag::STR && b.tag == tag::STR {
            // SAFETY: STR tags.
            return Ok(unsafe { a.str_bytes() <= b.str_bytes() });
        }
        let mut mm = self.metamethod(a, self.names.le);
        if mm.is_nil() {
            mm = self.metamethod(b, self.names.le);
        }
        if !mm.is_nil() {
            return Ok(self.call1(scratch, mm, &[a, b])?.truthy());
        }
        // Lua 5.2 falls back to `not (b < a)`.
        let mut mm = self.metamethod(b, self.names.lt);
        if mm.is_nil() {
            mm = self.metamethod(a, self.names.lt);
        }
        if mm.is_nil() {
            return self.compare_error(a, b);
        }
        Ok(!self.call1(scratch, mm, &[b, a])?.truthy())
    }

    fn order_mm(&mut self, scratch: *mut Value, a: Value, b: Value, name: Value) -> LResult<bool> {
        let mut mm = self.metamethod(a, name);
        if mm.is_nil() {
            mm = self.metamethod(b, name);
        }
        if mm.is_nil() {
            return self.compare_error(a, b);
        }
        Ok(self.call1(scratch, mm, &[a, b])?.truthy())
    }

    fn compare_error<T>(&mut self, a: Value, b: Value) -> LResult<T> {
        let (t1, t2) = (a.type_name(), b.type_name());
        if t1 == t2 {
            self.throw(&format!("attempt to compare two {t1} values"))
        } else {
            self.throw(&format!("attempt to compare {t1} with {t2}"))
        }
    }

    /// `#v`.
    pub fn length(&mut self, scratch: *mut Value, v: Value) -> LResult<Value> {
        match v.tag {
            tag::TABLE => {
                let mm = self.metamethod(v, self.names.len);
                if mm.is_nil() {
                    // SAFETY: TABLE tag.
                    Ok(Value::num(unsafe { (*(v.bits as *const Table)).len() } as f64))
                } else {
                    self.call1(scratch, mm, &[v])
                }
            }
            // SAFETY: STR tag.
            tag::STR => Ok(Value::num(unsafe { (*(v.bits as *const LStr)).len } as f64)),
            _ => {
                let mm = self.metamethod(v, self.names.len);
                if mm.is_nil() {
                    return self.type_error(v, "get length of", 0);
                }
                self.call1(scratch, mm, &[v])
            }
        }
    }

    /// Concatenates `vals` right to left, as `OP_CONCAT` does.
    pub fn concat(&mut self, scratch: *mut Value, vals: &[Value]) -> LResult<Value> {
        let mut acc = vals[vals.len() - 1];
        for i in (0..vals.len() - 1).rev() {
            let l = vals[i];
            let plain = |v: Value| v.tag == tag::STR || v.tag == tag::NUM;
            if plain(l) && plain(acc) {
                let (a, b) = (self.to_str_raw(l), self.to_str_raw(acc));
                // SAFETY: both are strings now.
                let mut buf = unsafe { a.str_bytes().to_vec() };
                buf.extend_from_slice(unsafe { b.str_bytes() });
                acc = self.heap.intern(&buf);
            } else {
                let mut mm = self.metamethod(l, self.names.concat);
                if mm.is_nil() {
                    mm = self.metamethod(acc, self.names.concat);
                }
                if mm.is_nil() {
                    let (blame, which) = if plain(l) { (acc, i + 1) } else { (l, i) };
                    return self.type_error(blame, "concatenate", which as u32);
                }
                acc = self.call1(scratch, mm, &[l, acc])?;
            }
        }
        Ok(acc)
    }

    // ------------------------------------------------------------------ upvalues

    /// Finds or creates the open upvalue for `slot` on the running thread.
    pub fn find_upval(&mut self, slot: *mut Value) -> *mut UpVal {
        // SAFETY: `cur` is the running thread.
        let th = unsafe { &mut *self.cur };
        // SAFETY: open upvalues are live heap objects.
        let pos = th.open.partition_point(|u| unsafe { (**u).v } < slot);
        if let Some(u) = th.open.get(pos) {
            // SAFETY: as above.
            if unsafe { (**u).v } == slot {
                return *u;
            }
        }
        let u = self.heap.new_open_upval(slot);
        th.open.insert(pos, u);
        u
    }

    /// Closes every open upvalue at or above `level`.
    pub fn close_upvals(&mut self, level: *mut Value) {
        // SAFETY: `cur` is the running thread.
        let th = unsafe { &mut *self.cur };
        while let Some(&u) = th.open.last() {
            // SAFETY: live upvalue.
            unsafe {
                if (*u).v < level {
                    break;
                }
                (*u).closed = *(*u).v;
                (*u).v = &raw mut (*u).closed;
            }
            th.open.pop();
        }
    }

    // ------------------------------------------------------------------ coroutines

    /// Creates a suspended coroutine running `f`.
    pub fn new_coroutine(&mut self, f: Value) -> *mut Thread {
        let mut th = Thread::new(CO_SLOTS, CO_DEPTH);
        let thp: *mut Thread = &mut *th;
        let ctxp: *mut Ctx = self;
        let stack = DefaultStack::new(CO_NATIVE).expect("coroutine stack");
        let co = Coroutine::with_stack(stack, move |yielder: &Yielder<(), ()>, ()| {
            // SAFETY: the context and the thread outlive the coroutine (arena heap).
            let (ctx, th) = unsafe { (&mut *ctxp, &mut *thp) };
            th.yielder = yielder;
            let nargs = th.transfer.len();
            let base = th.stack.as_mut_ptr();
            // SAFETY: the coroutine stack has room for the entry frame.
            unsafe {
                *base = f;
                for (i, a) in th.transfer.iter().enumerate() {
                    *base.add(1 + i) = *a;
                }
            }
            // SAFETY: `base` is a valid callee slot.
            let n = unsafe { rt_call(ctx, base, nargs as u32) };
            if n < 0 {
                th.transfer.clear();
                return false;
            }
            // SAFETY: results at `base[0..n]`.
            th.transfer = (0..n as usize).map(|i| unsafe { *base.add(i) }).collect();
            true
        });
        th.co = Some(co);
        self.heap.threads.push(th);
        thp
    }

    /// Resumes `co` with `args`: `Ok(values)` on yield or return, `Err(value)` on error.
    pub fn resume(&mut self, co: *mut Thread, args: &[Value]) -> Result<Vec<Value>, Value> {
        // SAFETY: live thread of this heap.
        let th = unsafe { &mut *co };
        match th.status {
            CoStatus::Suspended => {}
            CoStatus::Dead => return Err(self.heap.intern(b"cannot resume dead coroutine")),
            _ => return Err(self.heap.intern(b"cannot resume non-suspended coroutine")),
        }
        let prev = self.cur;
        // SAFETY: `cur` is live.
        let prev_ref = unsafe { &mut *prev };
        prev_ref.status = CoStatus::Normal;
        th.status = CoStatus::Running;
        th.resumer = prev;
        th.transfer.clear();
        th.transfer.extend_from_slice(args);
        self.cur = co;
        self.stack_limit = th.limit();
        let result = match th.co.as_mut() {
            Some(c) => c.resume(()),
            None => CoroutineResult::Return(false),
        };
        self.cur = prev;
        prev_ref.status = CoStatus::Running;
        self.stack_limit = prev_ref.limit();
        match result {
            CoroutineResult::Yield(()) => {
                th.status = CoStatus::Suspended;
                Ok(std::mem::take(&mut th.transfer))
            }
            CoroutineResult::Return(true) => {
                th.status = CoStatus::Dead;
                self.close_thread(th);
                Ok(std::mem::take(&mut th.transfer))
            }
            CoroutineResult::Return(false) => {
                th.status = CoStatus::Dead;
                self.close_thread(th);
                Err(self.err)
            }
        }
    }

    fn close_thread(&mut self, th: &mut Thread) {
        let level = th.stack.as_mut_ptr();
        while let Some(&u) = th.open.last() {
            // SAFETY: live upvalue.
            unsafe {
                if (*u).v < level {
                    break;
                }
                (*u).closed = *(*u).v;
                (*u).v = &raw mut (*u).closed;
            }
            th.open.pop();
        }
    }

    /// Suspends the running coroutine with `vals`; returns the values of the next resume.
    pub fn yield_now(&mut self, vals: &[Value]) -> LResult<Vec<Value>> {
        // SAFETY: `cur` is live.
        let th = unsafe { &mut *self.cur };
        if th.yielder.is_null() || self.cur == self.main {
            return self.throw_lib("attempt to yield from outside a coroutine");
        }
        th.transfer.clear();
        th.transfer.extend_from_slice(vals);
        // SAFETY: the yielder lives on this coroutine's own stack for as long as it runs.
        unsafe { (*th.yielder).suspend(()) };
        // SAFETY: `cur` is this coroutine again after the resume.
        let th = unsafe { &mut *self.cur };
        Ok(std::mem::take(&mut th.transfer))
    }

    /// Runs `f(args)` on the main thread, on a dedicated native stack, and returns its results.
    pub fn run(&mut self, f: Value, args: &[Value]) -> Result<Vec<Value>, Value> {
        let ctxp: *mut Ctx = self;
        let args = args.to_vec();
        let Ok(stack) = DefaultStack::new(MAIN_NATIVE) else {
            return Err(self.heap.intern(b"cannot allocate the native stack"));
        };
        let mut co: Coroutine<(), (), Result<Vec<Value>, Value>, DefaultStack> =
            Coroutine::with_stack(stack, move |_: &Yielder<(), ()>, ()| {
                // SAFETY: the context outlives this coroutine, which finishes before `run` returns.
                let ctx = unsafe { &mut *ctxp };
                // SAFETY: the main thread is live.
                let base = unsafe { (*ctx.main).stack.as_mut_ptr() };
                ctx.cur = ctx.main;
                // SAFETY: as above.
                ctx.stack_limit = unsafe { (*ctx.main).limit() };
                match ctx.call_n(base, f, &args) {
                    Ok(v) => Ok(v),
                    Err(Thrown) => Err(ctx.err),
                }
            });
        match co.resume(()) {
            CoroutineResult::Return(r) => r,
            CoroutineResult::Yield(()) => Err(self.heap.intern(b"main chunk yielded")),
        }
    }

    /// Builds a closure for a loaded chunk with `_ENV` bound to the globals.
    pub fn main_closure(&mut self, proto: *const ProtoRt) -> Value {
        let env = self.heap.new_closed_upval(Value {
            tag: tag::TABLE,
            bits: self.globals as u64,
        });
        let c = self.heap.new_lua_closure(proto, vec![env]);
        Value {
            tag: tag::FUNC,
            bits: c as u64,
        }
    }
}

unsafe extern "C" fn rust_trampoline(ctx: *mut Ctx, func: *mut Value, nargs: u32) -> i32 {
    // SAFETY: `func[0]` is a native closure created by `new_rust_closure`.
    unsafe {
        let cl = (*func).bits as *const Closure;
        let Some(f) = (*cl).rust else {
            return -1;
        };
        (*ctx).lua_op = false;
        match f(
            &mut *ctx,
            Args {
                func,
                n: nargs as usize,
            },
        ) {
            Ok(n) => n as i32,
            Err(Thrown) => -1,
        }
    }
}

/// Calls the function in `func` with `nargs` arguments above it.
///
/// # Safety
/// `func` must point into the running thread's stack with `nargs` initialised slots above it.
pub unsafe extern "C" fn rt_call(ctx: *mut Ctx, func: *mut Value, nargs: u32) -> i32 {
    // SAFETY: caller contract.
    unsafe {
        (*ctx).site_cl = std::ptr::null();
        rt_call_inner(ctx, func, nargs)
    }
}

/// [`rt_call`] without touching the call-site registers.
///
/// # Safety
/// As [`rt_call`].
pub unsafe fn rt_call_inner(ctx: *mut Ctx, func: *mut Value, mut nargs: u32) -> i32 {
    // SAFETY: caller contract.
    unsafe {
        let c = &mut *ctx;
        let th = &mut *c.cur;
        th.depth += 1;
        if th.depth > th.depth_limit {
            th.depth -= 1;
            let _ = c.throw::<()>("stack overflow");
            return -1;
        }
        let r = loop {
            let f = *func;
            if f.tag == tag::FUNC {
                let cl = f.bits as *const Closure;
                break ((*cl).f)(ctx, func, nargs);
            }
            let mm = c.metamethod(f, c.names.call);
            if mm.is_nil() {
                let _ = c.type_error::<()>(f, "call", 0);
                break -1;
            }
            if c.ensure_stack(func, nargs as usize + 3).is_err() {
                break -1;
            }
            for i in (0..=nargs as usize).rev() {
                *func.add(i + 1) = *func.add(i);
            }
            *func = mm;
            nargs += 1;
        };
        (*c.cur).depth -= 1;
        r
    }
}
