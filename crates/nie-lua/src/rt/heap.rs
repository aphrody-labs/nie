//! Heap objects and the per-run arena.
//!
//! Every object lives until the [`Heap`] is dropped: the garbage collector of a script run is the
//! arena itself. That is the contract of game scripts, which run for a bounded time per screen
//! or event.

use std::alloc::{Layout, alloc, dealloc};

use super::state::{Ctx, NativeFn};
use super::table::Table;
use super::value::{LStr, Value};

/// A captured variable. While open, `v` points into a thread stack; once closed it points at
/// `closed`.
#[repr(C)]
pub struct UpVal {
    /// Current location of the value.
    pub v: *mut Value,
    /// Storage once the variable left the stack.
    pub closed: Value,
}

/// A function object, Lua or native. `repr(C)`: generated code reads `upvals` and `k`.
#[repr(C)]
pub struct Closure {
    /// Entry point.
    pub f: NativeFn,
    /// Captured variables (`nup` entries), null for natives.
    pub upvals: *mut *mut UpVal,
    /// Constant pool of the prototype (Lua closures).
    pub k: *const Value,
    /// Prototype, null for natives.
    pub proto: *const ProtoRt,
    /// Number of upvalues.
    pub nup: u32,
    /// Slot for natives that bind a Rust function.
    pub rust: Option<super::state::RustFn>,
    /// Value bound to a native (host name, coroutine, ...).
    pub data: Value,
}

/// Runtime view of a compiled prototype.
pub struct ProtoRt {
    /// Compiled entry point.
    pub f: NativeFn,
    /// Constants as values.
    pub consts: Box<[Value]>,
    /// Nested prototypes.
    pub children: Vec<*const ProtoRt>,
    /// `(in_stack, index)` per upvalue.
    pub upvals: Vec<(bool, u8)>,
    /// Error descriptions per bytecode index, sorted by index.
    pub sites: Vec<(u32, Vec<Option<String>>)>,
}

/// Open-addressing set of interned strings.
struct StrTable {
    slots: Vec<*const LStr>,
    count: usize,
}

fn hash_bytes(b: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64 ^ b.len() as u64;
    let mut chunks = b.chunks_exact(8);
    for c in &mut chunks {
        let w = u64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]);
        h = (h.rotate_left(5) ^ w).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
    for &x in chunks.remainder() {
        h = (h.rotate_left(5) ^ u64::from(x)).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
    h ^ (h >> 32)
}

/// Owner of every object of a run.
pub struct Heap {
    strs: StrTable,
    str_blocks: Vec<(*mut u8, Layout)>,
    /// Tables.
    pub tables: Vec<Box<Table>>,
    /// Closures.
    pub closures: Vec<Box<Closure>>,
    /// Upvalues.
    pub upvals: Vec<Box<UpVal>>,
    /// Upvalue arrays of closures.
    pub upval_arrays: Vec<Box<[*mut UpVal]>>,
    /// Prototypes.
    pub protos: Vec<Box<ProtoRt>>,
    /// Coroutines.
    pub threads: Vec<Box<super::state::Thread>>,
}

impl Default for Heap {
    fn default() -> Self {
        Self::new()
    }
}

impl Heap {
    /// An empty heap.
    #[must_use]
    pub fn new() -> Self {
        Self {
            strs: StrTable {
                slots: vec![std::ptr::null(); 1024],
                count: 0,
            },
            str_blocks: Vec::new(),
            tables: Vec::new(),
            closures: Vec::new(),
            upvals: Vec::new(),
            upval_arrays: Vec::new(),
            protos: Vec::new(),
            threads: Vec::new(),
        }
    }

    /// Interns `bytes` and returns the string value.
    pub fn intern(&mut self, bytes: &[u8]) -> Value {
        let h = hash_bytes(bytes);
        let mask = self.strs.slots.len() - 1;
        let mut i = (h as usize) & mask;
        loop {
            let p = self.strs.slots[i];
            if p.is_null() {
                break;
            }
            // SAFETY: slots hold live strings of this heap.
            let hit = unsafe { (*p).hash == h && LStr::bytes(p) == bytes };
            if hit {
                return Value {
                    tag: super::value::tag::STR,
                    bits: p as u64,
                };
            }
            i = (i + 1) & mask;
        }
        let size = std::mem::size_of::<LStr>() + bytes.len() + 1;
        // SAFETY: the layout is non-zero sized and 8-aligned; the block is fully initialised
        // below and freed with the same layout in `Drop`.
        let p = unsafe {
            let layout = Layout::from_size_align_unchecked(size, 8);
            let raw = alloc(layout);
            if raw.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            let s = raw.cast::<LStr>();
            (*s).len = bytes.len();
            (*s).hash = h;
            let data = raw.add(std::mem::size_of::<LStr>());
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len());
            *data.add(bytes.len()) = 0;
            self.str_blocks.push((raw, layout));
            s.cast_const()
        };
        self.strs.slots[i] = p;
        self.strs.count += 1;
        if self.strs.count * 2 > self.strs.slots.len() {
            self.grow_strs();
        }
        Value {
            tag: super::value::tag::STR,
            bits: p as u64,
        }
    }

    fn grow_strs(&mut self) {
        let n = self.strs.slots.len() * 2;
        let old = std::mem::replace(&mut self.strs.slots, vec![std::ptr::null(); n]);
        for p in old.into_iter().filter(|p| !p.is_null()) {
            // SAFETY: live string.
            let h = unsafe { (*p).hash };
            let mut i = (h as usize) & (n - 1);
            while !self.strs.slots[i].is_null() {
                i = (i + 1) & (n - 1);
            }
            self.strs.slots[i] = p;
        }
    }

    /// Allocates a table.
    pub fn new_table(&mut self, narr: usize, nhash: usize) -> *mut Table {
        let mut t = Box::new(Table::new(narr, nhash));
        let p: *mut Table = &mut *t;
        self.tables.push(t);
        p
    }

    /// Allocates a closed upvalue holding `v`.
    pub fn new_closed_upval(&mut self, v: Value) -> *mut UpVal {
        let mut u = Box::new(UpVal {
            v: std::ptr::null_mut(),
            closed: v,
        });
        u.v = &raw mut u.closed;
        let p: *mut UpVal = &mut *u;
        self.upvals.push(u);
        p
    }

    /// Allocates an open upvalue pointing at `slot`.
    pub fn new_open_upval(&mut self, slot: *mut Value) -> *mut UpVal {
        let mut u = Box::new(UpVal {
            v: slot,
            closed: Value::NIL,
        });
        let p: *mut UpVal = &mut *u;
        self.upvals.push(u);
        p
    }

    /// Allocates a native closure.
    pub fn new_native(
        &mut self,
        f: NativeFn,
        rust: Option<super::state::RustFn>,
        data: Value,
    ) -> *mut Closure {
        let mut c = Box::new(Closure {
            f,
            upvals: std::ptr::null_mut(),
            k: std::ptr::null(),
            proto: std::ptr::null(),
            nup: 0,
            rust,
            data,
        });
        let p: *mut Closure = &mut *c;
        self.closures.push(c);
        p
    }

    /// Allocates a Lua closure for `proto` with its upvalue array.
    pub fn new_lua_closure(&mut self, proto: *const ProtoRt, ups: Vec<*mut UpVal>) -> *mut Closure {
        let mut arr = ups.into_boxed_slice();
        let upvals = arr.as_mut_ptr();
        let nup = arr.len() as u32;
        self.upval_arrays.push(arr);
        // SAFETY: protos are owned by this heap and outlive the closure.
        let (f, k) = unsafe { ((*proto).f, (*proto).consts.as_ptr()) };
        let mut c = Box::new(Closure {
            f,
            upvals,
            k,
            proto,
            nup,
            rust: None,
            data: Value::NIL,
        });
        let p: *mut Closure = &mut *c;
        self.closures.push(c);
        p
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        for t in &mut self.threads {
            t.release();
        }
        for (p, layout) in self.str_blocks.drain(..) {
            // SAFETY: allocated in `intern` with this layout.
            unsafe { dealloc(p, layout) };
        }
    }
}

/// Helper used by libraries: builds a table from `(name, native)` pairs.
pub fn register_fns(ctx: &mut Ctx, table: *mut Table, fns: &[(&str, super::state::RustFn)]) {
    for (name, f) in fns {
        let key = ctx.heap.intern(name.as_bytes());
        let func = ctx.new_rust_fn(*f);
        // SAFETY: `table` was allocated by this heap.
        unsafe { (*table).set(key, func) };
    }
}
