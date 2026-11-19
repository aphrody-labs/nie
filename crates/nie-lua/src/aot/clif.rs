//! Cranelift backend: one native function per Lua prototype.
//!
//! Generic over [`Module`]: the same code is emitted into a JIT module (tests, in-process
//! execution) and into an object module (ahead-of-time artifacts). Generated code never names a
//! runtime symbol: every runtime entry is called through the helper table at the start of
//! [`Ctx`], so an object file links against nothing.

use std::mem::offset_of;

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{
    AbiParam, Block, BlockArg, InstBuilder, MemFlagsData, SigRef, Signature, Type, Value as V, types,
};
use cranelift_codegen::isa::CallConv;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{FuncId, Linkage, Module, ModuleError};

use super::ir::{Block as IrBlock, BlockId, Cond, Count, Func, Op, Opnd, Term};
use crate::bytecode::Constant;
use crate::rt::heap::{Closure, UpVal};
use crate::rt::ops::id as h;
use crate::rt::state::{Ctx, STACK_SLACK, arith};
use crate::rt::value::tag;

const I64: Type = types::I64;
const I32: Type = types::I32;

/// Parameter kinds of the helper signatures.
#[derive(Clone, Copy)]
enum K {
    P,
    W,
}

/// `(params, returns)` per helper id.
const SIGS: [(&[K], Option<K>); h::COUNT] = [
    (&[K::P, K::P, K::W, K::P, K::W], Some(K::W)),           // CALL
    (&[K::P, K::P, K::P, K::P, K::P], Some(K::W)),           // GETTABLE
    (&[K::P, K::P, K::P, K::P, K::P], Some(K::W)),           // SETTABLE
    (&[K::P, K::P, K::P, K::W, K::P, K::P], Some(K::W)),     // GETTABUP
    (&[K::P, K::P, K::P, K::W, K::P, K::P], Some(K::W)),     // SETTABUP
    (&[K::P, K::P, K::W, K::P, K::P, K::P], Some(K::W)),     // ARITH
    (&[K::P, K::P, K::P, K::P], Some(K::W)),                 // EQ
    (&[K::P, K::P, K::P, K::P], Some(K::W)),                 // LT
    (&[K::P, K::P, K::P, K::P], Some(K::W)),                 // LE
    (&[K::P, K::P, K::P, K::P], Some(K::W)),                 // LEN
    (&[K::P, K::P, K::P, K::W, K::P], Some(K::W)),           // CONCAT
    (&[K::P, K::P, K::W, K::P, K::P], Some(K::W)),           // CLOSURE
    (&[K::P, K::P], None),                                   // CLOSE
    (&[K::P, K::P, K::W, K::W], None),                       // NEWTABLE
    (&[K::P, K::P, K::P, K::W, K::W], None),                 // SETLIST
    (&[K::P, K::P, K::P, K::P, K::P], Some(K::W)),           // SELF
    (&[K::P, K::P], Some(K::W)),                             // FORPREP
    (&[K::P, K::P, K::W], Some(K::W)),                       // TFORCALL
    (&[K::P, K::P, K::P, K::W, K::W], Some(K::W)),           // VARARG
    (&[K::P, K::P, K::W, K::W, K::W], Some(K::P)),           // VPROLOGUE
    (&[K::P, K::W, K::W], None),                             // NILFILL
    (&[K::P], Some(K::W)),                                   // OUT_OF_STACK
    (&[K::P], Some(K::W)),                                   // OUT_OF_FUEL
    (&[K::P, K::W, K::W], None),                             // FIXRESULTS
    (&[K::P, K::P, K::W], None),                             // ANNOTATE
];

const OFF_HELPERS: i32 = offset_of!(Ctx, helpers) as i32;
const OFF_FUEL: i32 = offset_of!(Ctx, fuel) as i32;
const OFF_LIMIT: i32 = offset_of!(Ctx, stack_limit) as i32;
const OFF_UPVALS: i32 = offset_of!(Closure, upvals) as i32;
const OFF_K: i32 = offset_of!(Closure, k) as i32;
const OFF_UV_V: i32 = offset_of!(UpVal, v) as i32;

/// Signature of every compiled Lua function.
pub fn function_signature(call_conv: CallConv) -> Signature {
    let mut s = Signature::new(call_conv);
    s.params.push(AbiParam::new(I64));
    s.params.push(AbiParam::new(I64));
    s.params.push(AbiParam::new(I32));
    s.returns.push(AbiParam::new(I32));
    s
}

struct Gen<'a, 'f> {
    b: FunctionBuilder<'f>,
    f: &'a Func,
    sigs: Vec<Option<SigRef>>,
    flags: MemFlagsData,
    ctx: Variable,
    func: Variable,
    nargs: Variable,
    base: Variable,
    cl: Variable,
    kptr: Variable,
    top: Variable,
    ftop: Variable,
    vptr: Variable,
    nvar: Variable,
    err: Block,
    blocks: Vec<Block>,
    pending: Vec<(Block, BlockId)>,
    cur_pc: usize,
}

fn kty(k: K) -> Type {
    match k {
        K::P => I64,
        K::W => I32,
    }
}

impl Gen<'_, '_> {
    fn c64(&mut self, n: i64) -> V {
        self.b.ins().iconst(I64, n)
    }

    fn c32(&mut self, n: i64) -> V {
        self.b.ins().iconst(I32, n)
    }

    fn var(&mut self, v: Variable) -> V {
        self.b.use_var(v)
    }

    fn sig(&mut self, id: usize) -> SigRef {
        if let Some(s) = self.sigs[id] {
            return s;
        }
        let (ps, r) = SIGS[id];
        let mut sig = Signature::new(CallConv::SystemV);
        // The call convention is patched by the caller through `conv`.
        sig.call_conv = self.b.func.signature.call_conv;
        for p in ps {
            sig.params.push(AbiParam::new(kty(*p)));
        }
        if let Some(r) = r {
            sig.returns.push(AbiParam::new(kty(r)));
        }
        let s = self.b.import_signature(sig);
        self.sigs[id] = Some(s);
        s
    }

    /// Calls helper `id`; returns its result when it has one.
    fn call(&mut self, id: usize, args: &[V]) -> Option<V> {
        let sig = self.sig(id);
        let ctx = self.var(self.ctx);
        let fnptr = self.b.ins().load(I64, self.flags, ctx, OFF_HELPERS + 8 * id as i32);
        let inst = self.b.ins().call_indirect(sig, fnptr, args);
        self.b.inst_results(inst).first().copied()
    }

    fn reg_ptr(&mut self, r: u8) -> V {
        let base = self.var(self.base);
        self.b.ins().iadd_imm_s(base, 16 * i64::from(r))
    }

    fn k_ptr(&mut self, k: u32) -> V {
        let kp = self.var(self.kptr);
        self.b.ins().iadd_imm_s(kp, 16 * i64::from(k))
    }

    fn opnd_ptr(&mut self, o: Opnd) -> V {
        match o {
            Opnd::R(r) => self.reg_ptr(r),
            Opnd::K(k) => self.k_ptr(k),
        }
    }

    fn reg_tag(&mut self, r: u8) -> V {
        let base = self.var(self.base);
        self.b.ins().load(I64, self.flags, base, 16 * i32::from(r))
    }

    fn reg_bits(&mut self, r: u8) -> V {
        let base = self.var(self.base);
        self.b.ins().load(I64, self.flags, base, 16 * i32::from(r) + 8)
    }

    fn store_reg(&mut self, r: u8, t: V, bits: V) {
        let base = self.var(self.base);
        self.b.ins().store(self.flags, t, base, 16 * i32::from(r));
        self.b.ins().store(self.flags, bits, base, 16 * i32::from(r) + 8);
    }

    fn store_num(&mut self, r: u8, f: V) {
        let bits = self.b.ins().bitcast(I64, MemFlagsData::new(), f);
        let t = self.c64(tag::NUM as i64);
        self.store_reg(r, t, bits);
    }

    /// Statically known `(tag, bits)` of a constant, when it does not need the pool.
    fn const_imm(&self, k: u32) -> Option<(i64, i64)> {
        match &self.f.consts[k as usize] {
            Constant::Nil => Some((tag::NIL as i64, 0)),
            Constant::Boolean(b) => Some((tag::BOOL as i64, i64::from(*b))),
            Constant::Number(n) => Some((tag::NUM as i64, n.to_bits() as i64)),
            Constant::String(_) => None,
        }
    }

    fn opnd_tag(&mut self, o: Opnd) -> V {
        match o {
            Opnd::R(r) => self.reg_tag(r),
            Opnd::K(k) => match self.const_imm(k) {
                Some((t, _)) => self.c64(t),
                None => self.c64(tag::STR as i64),
            },
        }
    }

    fn opnd_bits(&mut self, o: Opnd) -> V {
        match o {
            Opnd::R(r) => self.reg_bits(r),
            Opnd::K(k) => match self.const_imm(k) {
                Some((_, bits)) => self.c64(bits),
                None => {
                    let kp = self.var(self.kptr);
                    self.b.ins().load(I64, self.flags, kp, 16 * k as i32 + 8)
                }
            },
        }
    }

    fn opnd_f64(&mut self, o: Opnd) -> V {
        if let Opnd::K(k) = o {
            if let Constant::Number(n) = self.f.consts[k as usize] {
                return self.b.ins().f64const(n);
            }
        }
        let bits = self.opnd_bits(o);
        self.b.ins().bitcast(types::F64, MemFlagsData::new(), bits)
    }

    fn is_num(&mut self, o: Opnd) -> Option<V> {
        // `None` when statically a number.
        if let Opnd::K(k) = o {
            if matches!(self.f.consts[k as usize], Constant::Number(_)) {
                return None;
            }
        }
        let t = self.opnd_tag(o);
        Some(self.b.ins().icmp_imm_s(IntCC::Equal, t, tag::NUM as i64))
    }

    fn both_num(&mut self, b: Opnd, c: Opnd) -> Option<V> {
        match (self.is_num(b), self.is_num(c)) {
            (None, None) => None,
            (Some(x), None) | (None, Some(x)) => Some(x),
            (Some(x), Some(y)) => Some(self.b.ins().band(x, y)),
        }
    }

    fn copy_value(&mut self, dst: u8, src_t: V, src_b: V) {
        self.store_reg(dst, src_t, src_b);
    }

    fn annotate(&mut self, pc: u32) {
        let (ctx, cl) = (self.var(self.ctx), self.var(self.cl));
        let p = self.c32(i64::from(pc));
        self.call(h::ANNOTATE, &[ctx, cl, p]);
    }

    /// Like a status check, but the failure edge first attaches the variable description of the
    /// operation at `pc` to a pending type error.
    fn check_status_at(&mut self, st: V, pc: u32) {
        let ok = self.b.create_block();
        let fail = self.b.create_block();
        self.b.set_cold_block(fail);
        self.b.ins().brif(st, fail, &[], ok, &[]);
        self.b.switch_to_block(fail);
        self.annotate(pc);
        self.b.ins().jump(self.err, &[]);
        self.b.switch_to_block(ok);
    }

    fn check_status(&mut self, st: V) {
        let ok = self.b.create_block();
        self.b.ins().brif(st, self.err, &[], ok, &[]);
        self.b.switch_to_block(ok);
    }

    /// Block to jump to for an edge `cur -> to`: backward edges burn fuel first.
    fn edge(&mut self, to: BlockId) -> Block {
        if self.f.blocks[to].pc <= self.cur_pc {
            let t = self.b.create_block();
            self.pending.push((t, to));
            t
        } else {
            self.blocks[to]
        }
    }

    fn emit_fuel(&mut self, then: Block) {
        let ctx = self.var(self.ctx);
        let fuel = self.b.ins().load(I64, self.flags, ctx, OFF_FUEL);
        let nf = self.b.ins().iadd_imm_s(fuel, -1);
        self.b.ins().store(self.flags, nf, ctx, OFF_FUEL);
        let dead = self.b.create_block();
        self.b.set_cold_block(dead);
        let neg = self.b.ins().icmp_imm_s(IntCC::SignedLessThan, nf, 0);
        self.b.ins().brif(neg, dead, &[], then, &[]);
        self.b.switch_to_block(dead);
        let ctx = self.var(self.ctx);
        self.call(h::OUT_OF_FUEL, &[ctx]);
        self.b.ins().jump(self.err, &[]);
    }

    fn top_count(&mut self, from: V) -> V {
        // (top - from) / 16 as i32
        let top = self.var(self.top);
        let d = self.b.ins().isub(top, from);
        let n = self.b.ins().sshr_imm(d, 4);
        self.b.ins().ireduce(I32, n)
    }

    fn count_value(&mut self, c: Count, from: V) -> V {
        match c {
            Count::Fixed(n) => self.c32(i64::from(n)),
            Count::ToTop => self.top_count(from),
        }
    }

    fn truthy(&mut self, r: u8) -> V {
        let t = self.reg_tag(r);
        let bits = self.reg_bits(r);
        let nil = self.b.ins().icmp_imm_s(IntCC::Equal, t, tag::NIL as i64);
        let is_bool = self.b.ins().icmp_imm_s(IntCC::Equal, t, tag::BOOL as i64);
        let zero = self.b.ins().icmp_imm_s(IntCC::Equal, bits, 0);
        let fls = self.b.ins().band(is_bool, zero);
        let falsy = self.b.ins().bor(nil, fls);
        self.b.ins().bxor_imm(falsy, 1)
    }

    fn emit_op(&mut self, op: Op, pc: u32) {
        match op {
            Op::Move { a, b } => {
                let (t, bits) = (self.reg_tag(b), self.reg_bits(b));
                self.copy_value(a, t, bits);
            }
            Op::LoadK { a, k } => {
                let (t, bits) = (self.opnd_tag(Opnd::K(k)), self.opnd_bits(Opnd::K(k)));
                self.copy_value(a, t, bits);
            }
            Op::LoadBool { a, v } => {
                let t = self.c64(tag::BOOL as i64);
                let bits = self.c64(i64::from(v));
                self.store_reg(a, t, bits);
            }
            Op::LoadNil { a, n } => {
                let z = self.c64(0);
                for i in 0..=n {
                    self.store_reg(a + i as u8, z, z);
                }
            }
            Op::GetUpval { a, u } => {
                let vptr = self.upval_slot(u);
                let t = self.b.ins().load(I64, self.flags, vptr, 0);
                let bits = self.b.ins().load(I64, self.flags, vptr, 8);
                self.store_reg(a, t, bits);
            }
            Op::SetUpval { a, u } => {
                let (t, bits) = (self.reg_tag(a), self.reg_bits(a));
                let vptr = self.upval_slot(u);
                self.b.ins().store(self.flags, t, vptr, 0);
                self.b.ins().store(self.flags, bits, vptr, 8);
            }
            Op::GetTabUp { a, u, key } => {
                let (ctx, top, cl) = (self.var(self.ctx), self.var(self.ftop), self.var(self.cl));
                let (uc, kp, dst) = (self.c32(i64::from(u)), self.opnd_ptr(key), self.reg_ptr(a));
                let st = self.call(h::GETTABUP, &[ctx, top, cl, uc, kp, dst]).unwrap_or(ctx);
                self.check_status_at(st, pc);
            }
            Op::SetTabUp { u, key, val } => {
                let (ctx, top, cl) = (self.var(self.ctx), self.var(self.ftop), self.var(self.cl));
                let (uc, kp, vp) = (self.c32(i64::from(u)), self.opnd_ptr(key), self.opnd_ptr(val));
                let st = self.call(h::SETTABUP, &[ctx, top, cl, uc, kp, vp]).unwrap_or(ctx);
                self.check_status_at(st, pc);
            }
            Op::GetTable { a, b, key } => {
                let (ctx, top) = (self.var(self.ctx), self.var(self.ftop));
                let (o, k, d) = (self.reg_ptr(b), self.opnd_ptr(key), self.reg_ptr(a));
                let st = self.call(h::GETTABLE, &[ctx, top, o, k, d]).unwrap_or(ctx);
                self.check_status_at(st, pc);
            }
            Op::SetTable { a, key, val } => {
                let (ctx, top) = (self.var(self.ctx), self.var(self.ftop));
                let (o, k, v) = (self.reg_ptr(a), self.opnd_ptr(key), self.opnd_ptr(val));
                let st = self.call(h::SETTABLE, &[ctx, top, o, k, v]).unwrap_or(ctx);
                self.check_status_at(st, pc);
            }
            Op::NewTable { a, narr, nhash } => {
                let ctx = self.var(self.ctx);
                let (d, n1, n2) = (self.reg_ptr(a), self.c32(i64::from(narr)), self.c32(i64::from(nhash)));
                self.call(h::NEWTABLE, &[ctx, d, n1, n2]);
            }
            Op::SelfOp { a, b, key } => {
                let (ctx, top) = (self.var(self.ctx), self.var(self.ftop));
                let (ra, o, k) = (self.reg_ptr(a), self.reg_ptr(b), self.opnd_ptr(key));
                let st = self.call(h::SELF, &[ctx, top, ra, o, k]).unwrap_or(ctx);
                self.check_status_at(st, pc);
            }
            Op::Arith { op, a, b, c, nums } => self.emit_arith(op, a, b, c, nums, pc),
            Op::Unm { a, b, num } => {
                if num {
                    let x = self.opnd_f64(Opnd::R(b));
                    let r = self.b.ins().fneg(x);
                    self.store_num(a, r);
                } else {
                    let ctx = self.var(self.ctx);
                    let top = self.var(self.ftop);
                    let opc = self.c32(i64::from(arith::UNM));
                    let (d, s) = (self.reg_ptr(a), self.reg_ptr(b));
                    let st = self.call(h::ARITH, &[ctx, top, opc, d, s, s]).unwrap_or(ctx);
                    self.check_status_at(st, pc);
                }
            }
            Op::Not { a, b } => {
                let t = self.truthy(b);
                let nt = self.b.ins().bxor_imm(t, 1);
                let bits = self.b.ins().uextend(I64, nt);
                let tg = self.c64(tag::BOOL as i64);
                self.store_reg(a, tg, bits);
            }
            Op::Len { a, b } => {
                let (ctx, top) = (self.var(self.ctx), self.var(self.ftop));
                let (d, s) = (self.reg_ptr(a), self.reg_ptr(b));
                let st = self.call(h::LEN, &[ctx, top, d, s]).unwrap_or(ctx);
                self.check_status_at(st, pc);
            }
            Op::Concat { a, b, c } => {
                let (ctx, top) = (self.var(self.ctx), self.var(self.ftop));
                let (first, n, d) = (self.reg_ptr(b), self.c32(i64::from(c - b) + 1), self.reg_ptr(a));
                let st = self.call(h::CONCAT, &[ctx, top, first, n, d]).unwrap_or(ctx);
                self.check_status_at(st, pc);
            }
            Op::Close { from } => {
                let ctx = self.var(self.ctx);
                let p = self.reg_ptr(from);
                self.call(h::CLOSE, &[ctx, p]);
            }
            Op::Closure { a, idx } => {
                let (ctx, cl, base) = (self.var(self.ctx), self.var(self.cl), self.var(self.base));
                let (i, d) = (self.c32(i64::from(idx)), self.reg_ptr(a));
                let st = self.call(h::CLOSURE, &[ctx, cl, i, base, d]).unwrap_or(ctx);
                self.check_status(st);
            }
            Op::Call { a, args, rets } => self.emit_call(a, args, rets, pc),
            Op::SetList { a, n, start } => {
                let ctx = self.var(self.ctx);
                let (tbl, first) = (self.reg_ptr(a), self.reg_ptr(a + 1));
                let count = self.count_value(n, first);
                let st = self.c32(i64::from(start));
                self.call(h::SETLIST, &[ctx, tbl, first, count, st]);
            }
            Op::VarArg { a, n } => {
                let ctx = self.var(self.ctx);
                let dst = self.reg_ptr(a);
                let (src, nvar) = (self.var(self.vptr), self.var(self.nvar));
                let want = match n {
                    Count::Fixed(k) => self.c32(i64::from(k)),
                    Count::ToTop => self.c32(-1),
                };
                let cnt = self.call(h::VARARG, &[ctx, dst, src, nvar, want]).unwrap_or(ctx);
                let neg = self.b.ins().icmp_imm_s(IntCC::SignedLessThan, cnt, 0);
                let ok = self.b.create_block();
                self.b.ins().brif(neg, self.err, &[], ok, &[]);
                self.b.switch_to_block(ok);
                if n == Count::ToTop {
                    let c64 = self.b.ins().sextend(I64, cnt);
                    let off = self.b.ins().ishl_imm(c64, 4);
                    let top = self.b.ins().iadd(dst, off);
                    self.b.def_var(self.top, top);
                }
            }
            Op::ForPrep { a } => {
                let ctx = self.var(self.ctx);
                let ra = self.reg_ptr(a);
                let st = self.call(h::FORPREP, &[ctx, ra]).unwrap_or(ctx);
                self.check_status(st);
            }
            Op::TForCall { a, n } => {
                let ctx = self.var(self.ctx);
                let (ra, nr) = (self.reg_ptr(a), self.c32(i64::from(n)));
                let st = self.call(h::TFORCALL, &[ctx, ra, nr]).unwrap_or(ctx);
                self.check_status_at(st, pc);
            }
        }
    }

    /// Address of the value cell of upvalue `u`.
    fn upval_slot(&mut self, u: u32) -> V {
        let cl = self.var(self.cl);
        let ups = self.b.ins().load(I64, self.flags, cl, OFF_UPVALS);
        let uv = self.b.ins().load(I64, self.flags, ups, 8 * u as i32);
        self.b.ins().load(I64, self.flags, uv, OFF_UV_V)
    }

    fn emit_arith(&mut self, op: u32, a: u8, b: Opnd, c: Opnd, nums: bool, pc: u32) {
        let fast_ok = op != arith::POW;
        let guard = if nums || !fast_ok { None } else { self.both_num(b, c) };
        let static_nums = nums && fast_ok;
        if static_nums {
            let (x, y) = (self.opnd_f64(b), self.opnd_f64(c));
            let r = self.fop(op, x, y);
            self.store_num(a, r);
            return;
        }
        let join = self.b.create_block();
        let slow = self.b.create_block();
        match guard {
            Some(g) if fast_ok => {
                let fast = self.b.create_block();
                self.b.ins().brif(g, fast, &[], slow, &[]);
                self.b.switch_to_block(fast);
                let (x, y) = (self.opnd_f64(b), self.opnd_f64(c));
                let r = self.fop(op, x, y);
                self.store_num(a, r);
                self.b.ins().jump(join, &[]);
            }
            _ => {
                self.b.ins().jump(slow, &[]);
            }
        }
        self.b.switch_to_block(slow);
        let (ctx, top) = (self.var(self.ctx), self.var(self.ftop));
        let opc = self.c32(i64::from(op));
        let (d, bp, cp) = (self.reg_ptr(a), self.opnd_ptr(b), self.opnd_ptr(c));
        let st = self.call(h::ARITH, &[ctx, top, opc, d, bp, cp]).unwrap_or(ctx);
        self.check_status_at(st, pc);
        self.b.ins().jump(join, &[]);
        self.b.switch_to_block(join);
    }

    fn fop(&mut self, op: u32, x: V, y: V) -> V {
        match op {
            arith::ADD => self.b.ins().fadd(x, y),
            arith::SUB => self.b.ins().fsub(x, y),
            arith::MUL => self.b.ins().fmul(x, y),
            arith::DIV => self.b.ins().fdiv(x, y),
            _ => {
                // a - floor(a / b) * b
                let q = self.b.ins().fdiv(x, y);
                let f = self.b.ins().floor(q);
                let m = self.b.ins().fmul(f, y);
                self.b.ins().fsub(x, m)
            }
        }
    }

    fn emit_call(&mut self, a: u8, args: Count, rets: Count, pc: u32) {
        let ctx = self.var(self.ctx);
        let func = self.reg_ptr(a);
        let nargs = match args {
            Count::Fixed(n) => self.c32(i64::from(n)),
            Count::ToTop => {
                let n = self.top_count(func);
                self.b.ins().iadd_imm_s(n, -1)
            }
        };
        let (cl, pcv) = (self.var(self.cl), self.c32(i64::from(pc)));
        let n = self.call(h::CALL, &[ctx, func, nargs, cl, pcv]).unwrap_or(ctx);
        let neg = self.b.ins().icmp_imm_s(IntCC::SignedLessThan, n, 0);
        self.check_status_at(neg, pc);
        match rets {
            Count::ToTop => {
                let n64 = self.b.ins().sextend(I64, n);
                let off = self.b.ins().ishl_imm(n64, 4);
                let top = self.b.ins().iadd(func, off);
                self.b.def_var(self.top, top);
            }
            Count::Fixed(0) => {}
            Count::Fixed(k) => {
                let want = self.c32(i64::from(k));
                let short = self.b.ins().icmp(IntCC::SignedLessThan, n, want);
                let fix = self.b.create_block();
                let cont = self.b.create_block();
                self.b.ins().brif(short, fix, &[], cont, &[]);
                self.b.switch_to_block(fix);
                self.call(h::FIXRESULTS, &[func, n, want]);
                self.b.ins().jump(cont, &[]);
                self.b.switch_to_block(cont);
            }
        }
    }

    fn emit_return(&mut self, a: u8, n: Count) {
        if self.f.captures_locals {
            let ctx = self.var(self.ctx);
            let base = self.var(self.base);
            self.call(h::CLOSE, &[ctx, base]);
        }
        let src = self.reg_ptr(a);
        let func = self.var(self.func);
        match n {
            Count::Fixed(k) => {
                for i in 0..k as i32 {
                    let t = self.b.ins().load(I64, self.flags, src, 16 * i);
                    let bits = self.b.ins().load(I64, self.flags, src, 16 * i + 8);
                    self.b.ins().store(self.flags, t, func, 16 * i);
                    self.b.ins().store(self.flags, bits, func, 16 * i + 8);
                }
                let r = self.c32(i64::from(k));
                self.b.ins().return_(&[r]);
            }
            Count::ToTop => {
                let cnt = self.top_count(src);
                let cnt64 = self.b.ins().sextend(I64, cnt);
                let head = self.b.create_block();
                self.b.append_block_param(head, I64);
                let body = self.b.create_block();
                let done = self.b.create_block();
                let zero = self.c64(0);
                self.b.ins().jump(head, &[BlockArg::Value(zero)]);
                self.b.switch_to_block(head);
                let i = self.b.block_params(head)[0];
                let more = self.b.ins().icmp(IntCC::SignedLessThan, i, cnt64);
                self.b.ins().brif(more, body, &[], done, &[]);
                self.b.switch_to_block(body);
                let off = self.b.ins().ishl_imm(i, 4);
                let (sp, dp) = (self.b.ins().iadd(src, off), self.b.ins().iadd(func, off));
                let t = self.b.ins().load(I64, self.flags, sp, 0);
                let bits = self.b.ins().load(I64, self.flags, sp, 8);
                self.b.ins().store(self.flags, t, dp, 0);
                self.b.ins().store(self.flags, bits, dp, 8);
                let next = self.b.ins().iadd_imm_s(i, 1);
                self.b.ins().jump(head, &[BlockArg::Value(next)]);
                self.b.switch_to_block(done);
                self.b.ins().return_(&[cnt]);
            }
        }
    }

    /// Emits `cond` and branches to `then` / `els`.
    fn emit_branch(&mut self, cond: Cond, then: Block, els: Block) {
        match cond {
            Cond::Test { a, expect } => {
                let t = self.truthy(a);
                if expect {
                    self.b.ins().brif(t, then, &[], els, &[]);
                } else {
                    self.b.ins().brif(t, els, &[], then, &[]);
                }
            }
            Cond::Eq { b, c, expect, nums } => {
                let r = self.emit_eq(b, c, nums);
                if expect {
                    self.b.ins().brif(r, then, &[], els, &[]);
                } else {
                    self.b.ins().brif(r, els, &[], then, &[]);
                }
            }
            Cond::Lt { b, c, expect, nums } => {
                let r = self.emit_order(h::LT, FloatCC::LessThan, b, c, nums);
                if expect {
                    self.b.ins().brif(r, then, &[], els, &[]);
                } else {
                    self.b.ins().brif(r, els, &[], then, &[]);
                }
            }
            Cond::Le { b, c, expect, nums } => {
                let r = self.emit_order(h::LE, FloatCC::LessThanOrEqual, b, c, nums);
                if expect {
                    self.b.ins().brif(r, then, &[], els, &[]);
                } else {
                    self.b.ins().brif(r, els, &[], then, &[]);
                }
            }
        }
    }

    /// Returns an `i8` boolean for `b == c`.
    fn emit_eq(&mut self, b: Opnd, c: Opnd, nums: bool) -> V {
        if nums {
            let (x, y) = (self.opnd_f64(b), self.opnd_f64(c));
            return self.b.ins().fcmp(FloatCC::Equal, x, y);
        }
        // A constant of a type without `__eq` (nil, boolean, interned string) compares by
        // identity of tag and payload.
        for (k, other) in [(c, b), (b, c)] {
            if let Opnd::K(ki) = k {
                if !matches!(self.f.consts[ki as usize], Constant::Number(_)) {
                    let (kt, kb) = (self.opnd_tag(k), self.opnd_bits(k));
                    let (ot, ob) = (self.opnd_tag(other), self.opnd_bits(other));
                    let t = self.b.ins().icmp(IntCC::Equal, kt, ot);
                    let v = self.b.ins().icmp(IntCC::Equal, kb, ob);
                    return self.b.ins().band(t, v);
                }
            }
        }
        let join = self.b.create_block();
        self.b.append_block_param(join, types::I8);
        let slow = self.b.create_block();
        if let Some(g) = self.both_num(b, c) {
            let fast = self.b.create_block();
            self.b.ins().brif(g, fast, &[], slow, &[]);
            self.b.switch_to_block(fast);
        } else {
            self.b.ins().jump(slow, &[]);
            self.b.switch_to_block(slow);
            return self.emit_eq_helper_in_place(b, c, join);
        }
        let (x, y) = (self.opnd_f64(b), self.opnd_f64(c));
        let r = self.b.ins().fcmp(FloatCC::Equal, x, y);
        self.b.ins().jump(join, &[BlockArg::Value(r)]);
        self.b.switch_to_block(slow);
        self.emit_eq_helper_in_place(b, c, join)
    }

    fn emit_eq_helper_in_place(&mut self, b: Opnd, c: Opnd, join: Block) -> V {
        let (ctx, top) = (self.var(self.ctx), self.var(self.ftop));
        let (bp, cp) = (self.opnd_ptr(b), self.opnd_ptr(c));
        let r = self.call(h::EQ, &[ctx, top, bp, cp]).unwrap_or(ctx);
        let ok = self.b.create_block();
        let neg = self.b.ins().icmp_imm_s(IntCC::SignedLessThan, r, 0);
        self.b.ins().brif(neg, self.err, &[], ok, &[]);
        self.b.switch_to_block(ok);
        let v = self.b.ins().icmp_imm_s(IntCC::NotEqual, r, 0);
        self.b.ins().jump(join, &[BlockArg::Value(v)]);
        self.b.switch_to_block(join);
        self.b.block_params(join)[0]
    }

    fn emit_order(&mut self, helper: usize, cc: FloatCC, b: Opnd, c: Opnd, nums: bool) -> V {
        if nums {
            let (x, y) = (self.opnd_f64(b), self.opnd_f64(c));
            return self.b.ins().fcmp(cc, x, y);
        }
        let join = self.b.create_block();
        self.b.append_block_param(join, types::I8);
        let slow = self.b.create_block();
        if let Some(g) = self.both_num(b, c) {
            let fast = self.b.create_block();
            self.b.ins().brif(g, fast, &[], slow, &[]);
            self.b.switch_to_block(fast);
        } else {
            self.b.ins().jump(slow, &[]);
            self.b.switch_to_block(slow);
            return self.order_helper(helper, b, c, join);
        }
        let (x, y) = (self.opnd_f64(b), self.opnd_f64(c));
        let r = self.b.ins().fcmp(cc, x, y);
        self.b.ins().jump(join, &[BlockArg::Value(r)]);
        self.b.switch_to_block(slow);
        self.order_helper(helper, b, c, join)
    }

    fn order_helper(&mut self, helper: usize, b: Opnd, c: Opnd, join: Block) -> V {
        let (ctx, top) = (self.var(self.ctx), self.var(self.ftop));
        let (bp, cp) = (self.opnd_ptr(b), self.opnd_ptr(c));
        let r = self.call(helper, &[ctx, top, bp, cp]).unwrap_or(ctx);
        let ok = self.b.create_block();
        let neg = self.b.ins().icmp_imm_s(IntCC::SignedLessThan, r, 0);
        self.b.ins().brif(neg, self.err, &[], ok, &[]);
        self.b.switch_to_block(ok);
        let v = self.b.ins().icmp_imm_s(IntCC::NotEqual, r, 0);
        self.b.ins().jump(join, &[BlockArg::Value(v)]);
        self.b.switch_to_block(join);
        self.b.block_params(join)[0]
    }

    fn emit_term(&mut self, term: Term) {
        match term {
            Term::Jmp(t) => {
                let to = self.edge(t);
                self.b.ins().jump(to, &[]);
            }
            Term::Br { cond, then, els } => {
                let (t, e) = (self.edge(then), self.edge(els));
                self.emit_branch(cond, t, e);
            }
            Term::ForLoop { a, body, exit } => {
                let (idx, limit, step) = (
                    self.opnd_f64(Opnd::R(a)),
                    self.opnd_f64(Opnd::R(a + 1)),
                    self.opnd_f64(Opnd::R(a + 2)),
                );
                let next = self.b.ins().fadd(idx, step);
                let base = self.var(self.base);
                let bits = self.b.ins().bitcast(I64, MemFlagsData::new(), next);
                self.b.ins().store(self.flags, bits, base, 16 * i32::from(a) + 8);
                let zero = self.b.ins().f64const(0.0);
                let up = self.b.ins().fcmp(FloatCC::GreaterThan, step, zero);
                let le = self.b.ins().fcmp(FloatCC::LessThanOrEqual, next, limit);
                let ge = self.b.ins().fcmp(FloatCC::LessThanOrEqual, limit, next);
                let cont = self.b.ins().select(up, le, ge);
                let enter = self.b.create_block();
                let ex = self.edge(exit);
                self.b.ins().brif(cont, enter, &[], ex, &[]);
                self.b.switch_to_block(enter);
                self.store_num(a + 3, next);
                let to = self.edge(body);
                self.b.ins().jump(to, &[]);
            }
            Term::TForLoop { a, body, exit } => {
                let t = self.reg_tag(a + 1);
                let bits = self.reg_bits(a + 1);
                let cont = self.b.ins().icmp_imm_s(IntCC::NotEqual, t, tag::NIL as i64);
                let enter = self.b.create_block();
                let ex = self.edge(exit);
                self.b.ins().brif(cont, enter, &[], ex, &[]);
                self.b.switch_to_block(enter);
                self.store_reg(a, t, bits);
                let to = self.edge(body);
                self.b.ins().jump(to, &[]);
            }
            Term::Return { a, n } => self.emit_return(a, n),
        }
    }

    fn prologue(&mut self, entry: Block) {
        self.b.append_block_params_for_function_params(entry);
        self.b.switch_to_block(entry);
        let params = self.b.block_params(entry).to_vec();
        self.b.def_var(self.ctx, params[0]);
        self.b.def_var(self.func, params[1]);
        self.b.def_var(self.nargs, params[2]);
        let (ctx, func, nargs) = (params[0], params[1], params[2]);
        let f = self.f;
        let frame = 16 * i64::from(f.max_stack);
        let zero64 = self.c64(0);
        self.b.def_var(self.top, zero64);
        self.b.def_var(self.vptr, zero64);
        let zero32 = self.c32(0);
        self.b.def_var(self.nvar, zero32);
        if f.is_vararg {
            let np = self.c32(i64::from(f.num_params));
            let ms = self.c32(i64::from(f.max_stack));
            let base = self.call(h::VPROLOGUE, &[ctx, func, nargs, np, ms]).unwrap_or(ctx);
            let null = self.b.ins().icmp_imm_s(IntCC::Equal, base, 0);
            let ok = self.b.create_block();
            let fail = self.b.create_block();
            self.b.ins().brif(null, fail, &[], ok, &[]);
            self.b.switch_to_block(fail);
            self.call(h::OUT_OF_STACK, &[ctx]);
            self.b.ins().jump(self.err, &[]);
            self.b.switch_to_block(ok);
            self.b.def_var(self.base, base);
            let over = self.b.ins().icmp_imm_s(IntCC::SignedGreaterThan, nargs, i64::from(f.num_params));
            let diff = self.b.ins().iadd_imm_s(nargs, -i64::from(f.num_params));
            let nv = self.b.ins().select(over, diff, zero32);
            self.b.def_var(self.nvar, nv);
            let vp = self.b.ins().iadd_imm_s(func, 16 * (1 + i64::from(f.num_params)));
            self.b.def_var(self.vptr, vp);
        } else {
            let base = self.b.ins().iadd_imm_s(func, 16);
            self.b.def_var(self.base, base);
            let end = self.b.ins().iadd_imm_s(base, frame);
            let limit = self.b.ins().load(I64, self.flags, ctx, OFF_LIMIT);
            let over = self.b.ins().icmp(IntCC::UnsignedGreaterThan, end, limit);
            let ok = self.b.create_block();
            let fail = self.b.create_block();
            self.b.set_cold_block(fail);
            self.b.ins().brif(over, fail, &[], ok, &[]);
            self.b.switch_to_block(fail);
            self.call(h::OUT_OF_STACK, &[ctx]);
            self.b.ins().jump(self.err, &[]);
            self.b.switch_to_block(ok);
            if f.num_params > 0 {
                let short = self.b.ins().icmp_imm_s(IntCC::SignedLessThan, nargs, i64::from(f.num_params));
                let fill = self.b.create_block();
                let cont = self.b.create_block();
                self.b.ins().brif(short, fill, &[], cont, &[]);
                self.b.switch_to_block(fill);
                let np = self.c32(i64::from(f.num_params));
                self.call(h::NILFILL, &[base, nargs, np]);
                self.b.ins().jump(cont, &[]);
                self.b.switch_to_block(cont);
            }
        }
        let base = self.var(self.base);
        let ftop = self.b.ins().iadd_imm_s(base, frame);
        self.b.def_var(self.ftop, ftop);
        let cl = self.b.ins().load(I64, self.flags, func, 8);
        self.b.def_var(self.cl, cl);
        let k = self.b.ins().load(I64, self.flags, cl, OFF_K);
        self.b.def_var(self.kptr, k);
        // Entry fuel: bounds recursion and call-heavy loops.
        let first = self.blocks[0];
        self.emit_fuel(first);
    }
}

/// Compiles `f` into `module` under `name`; returns the function id.
///
/// # Errors
/// Any [`ModuleError`] of the underlying module (declaration, definition).
pub fn compile_func<M: Module>(
    module: &mut M,
    f: &Func,
    name: &str,
    linkage: Linkage,
) -> Result<FuncId, ModuleError> {
    let sig = function_signature(module.isa().default_call_conv());
    let id = module.declare_function(name, linkage, &sig)?;
    let module_isa_cfg = module.isa().frontend_config();
    let mut ctx = module.make_context();
    ctx.func.signature = sig;
    let mut fctx = FunctionBuilderContext::new();
    {
        let mut b = FunctionBuilder::new(&mut ctx.func, &mut fctx);
        let mk = |b: &mut FunctionBuilder<'_>| b.declare_var(I64);
        let (v_ctx, v_func, v_base, v_cl, v_k, v_top, v_ftop, v_vptr) =
            (mk(&mut b), mk(&mut b), mk(&mut b), mk(&mut b), mk(&mut b), mk(&mut b), mk(&mut b), mk(&mut b));
        let v_nargs = b.declare_var(I32);
        let v_nvar = b.declare_var(I32);
        let entry = b.create_block();
        let err = b.create_block();
        let blocks: Vec<Block> = f.blocks.iter().map(|_| b.create_block()).collect();
        let mut g = Gen {
            b,
            f,
            sigs: vec![None; h::COUNT],
            flags: MemFlagsData::trusted(),
            ctx: v_ctx,
            func: v_func,
            nargs: v_nargs,
            base: v_base,
            cl: v_cl,
            kptr: v_k,
            top: v_top,
            ftop: v_ftop,
            vptr: v_vptr,
            nvar: v_nvar,
            err,
            blocks,
            pending: Vec::new(),
            cur_pc: 0,
        };
        g.prologue(entry);
        let mut order: Vec<usize> = (0..f.blocks.len()).collect();
        order.sort_by_key(|i| f.blocks[*i].pc);
        for bi in order {
            let blk: &IrBlock = &f.blocks[bi];
            g.cur_pc = blk.pc;
            g.b.switch_to_block(g.blocks[bi]);
            for (op, pc) in blk.ops.iter().zip(&blk.pcs) {
                g.emit_op(*op, *pc);
            }
            g.emit_term(blk.term);
            while let Some((tramp, to)) = g.pending.pop() {
                g.b.switch_to_block(tramp);
                let target = g.blocks[to];
                g.emit_fuel(target);
            }
        }
        g.b.switch_to_block(err);
        g.b.set_cold_block(err);
        let m1 = g.b.ins().iconst(I32, -1);
        g.b.ins().return_(&[m1]);
        g.b.seal_all_blocks();
        let fc = module_isa_cfg;
        g.b.finalize(fc);
    }
    module.define_function(id, &mut ctx)?;
    module.clear_context(&mut ctx);
    Ok(id)
}

/// Slack the generated prologue relies on, re-exported for tests.
pub const FRAME_SLACK: usize = STACK_SLACK;
