//! x86-64 to Cranelift lifting of one function at a time.
//!
//! Model: guest memory is host memory (the image is mapped at its base), the 16
//! general registers live in Cranelift variables and are spilled to [`Cpu`] around
//! calls and returns, and the arithmetic flags CF/ZF/SF/OF/PF are computed
//! eagerly into variables so that dead-code elimination removes the unused ones.
//! Guest `call` pushes the return address on the guest stack and calls the
//! callee as a host function taking the `Cpu`.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{
    self, AbiParam, AtomicRmwOp, Block, InstBuilder, MemFlagsData, SigRef, Signature, StackSlotData, StackSlotKind,
    Type, Value, types,
};
use cranelift_codegen::isa::{CallConv, TargetFrontendConfig};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Switch, Variable};
use cranelift_module::{FuncId, Linkage, Module};
use iced_x86::{ConditionCode, Decoder, DecoderOptions, FlowControl, Instruction, Mnemonic, OpKind, Register};
use nie_win32::{Cpu, Rt, TrapKind};

use crate::image::Image;
use crate::pdata;

/// Why a function was not lifted. The text is `class:detail`, grouped by class in reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Why(pub String);

macro_rules! why {
    ($($t:tt)*) => { Why(format!($($t)*)) };
}

type R<T> = Result<T, Why>;

/// A function to lift: its start and every `[begin, end)` range it owns (chained fragments included).
#[derive(Clone, Debug)]
pub struct Root {
    pub va: u64,
    pub ranges: Vec<(u64, u64)>,
}

/// All functions of an image, for call-target and fragment resolution.
pub struct Plan {
    pub roots: Vec<Root>,
    starts: HashSet<u64>,
}

impl Plan {
    /// Roots from `.pdata`: chained fragments are attached to their parent function.
    pub fn new(img: &Image) -> Self {
        let ents = pdata::entries(img);
        let by_begin: HashMap<u32, &pdata::Entry> = ents.iter().map(|e| (e.begin, e)).collect();
        let mut map: BTreeMap<u32, Vec<(u64, u64)>> = BTreeMap::new();
        for e in &ents {
            let mut root = e.begin;
            let mut cur = e;
            for _ in 0..8 {
                match cur.chain.and_then(|p| by_begin.get(&p)) {
                    Some(p) => {
                        root = p.begin;
                        cur = p;
                    }
                    None => break,
                }
            }
            if e.is_root() || cur.is_root() {
                map.entry(root).or_default().push((img.base + u64::from(e.begin), img.base + u64::from(e.end)));
            }
        }
        let roots: Vec<Root> = map
            .into_iter()
            .map(|(r, mut ranges)| {
                ranges.sort_unstable();
                Root { va: img.base + u64::from(r), ranges }
            })
            .collect();
        Self::from_roots(roots)
    }

    pub fn from_roots(roots: Vec<Root>) -> Self {
        let starts = roots.iter().map(|r| r.va).collect();
        Self { roots, starts }
    }

    pub fn is_start(&self, va: u64) -> bool {
        self.starts.contains(&va)
    }
}

/// What the runtime needs to know about a lifted function.
#[derive(Clone, Debug, Default)]
pub struct FuncMeta {
    pub va: u64,
    pub insns: usize,
    pub blocks: usize,
    /// Direct call and tail-call targets.
    pub callees: BTreeSet<u64>,
    /// Calls the runtime must resolve: indirect calls, import calls.
    pub dynamic: bool,
    /// Reads the guest TEB through `gs`/`fs`.
    pub teb: bool,
}

pub fn symbol(va: u64) -> String {
    format!("f_{va:x}")
}

pub fn lifted_sig() -> Signature {
    let mut s = Signature::new(CallConv::SystemV);
    s.params.push(AbiParam::new(types::I64));
    s
}

fn call_sig() -> Signature {
    let mut s = Signature::new(CallConv::SystemV);
    s.params.extend([AbiParam::new(types::I64); 3]);
    s
}

fn trap_sig() -> Signature {
    let mut s = Signature::new(CallConv::SystemV);
    s.params.extend([AbiParam::new(types::I64); 4]);
    s
}

fn div_sig() -> Signature {
    let mut s = Signature::new(CallConv::SystemV);
    s.params.extend([AbiParam::new(types::I64); 6]);
    s
}

struct Decoded {
    insns: BTreeMap<u64, Instruction>,
    leaders: BTreeSet<u64>,
    tables: HashMap<u64, Vec<u64>>,
}

fn in_ranges(root: &Root, va: u64) -> bool {
    root.ranges.iter().any(|&(b, e)| va >= b && va < e)
}

fn fetch(img: &Image, va: u64) -> Option<Instruction> {
    let rva = img.va_to_rva(va)?;
    if !img.is_exec(rva) {
        return None;
    }
    let want = 15.min(img.flat.len() - rva as usize);
    let bytes = img.slice(rva, want)?;
    let i = Decoder::with_ip(64, bytes, va, DecoderOptions::NONE).decode();
    (!i.is_invalid()).then_some(i)
}

/// Targets of an MSVC-style jump table ending in `jmp reg`, from the instructions before it.
fn table_targets(img: &Image, root: &Root, insns: &BTreeMap<u64, Instruction>, jmp: &Instruction) -> R<Vec<u64>> {
    let target = jmp.op0_register().full_register();
    let prev: Vec<&Instruction> = insns.range(..jmp.ip()).rev().take(40).map(|(_, i)| i).collect();
    let mut base_reg = Register::None;
    let mut load: Option<&Instruction> = None;
    for i in &prev {
        if base_reg == Register::None
            && i.mnemonic() == Mnemonic::Add
            && i.op_kind(0) == OpKind::Register
            && i.op0_register().full_register() == target
            && i.op_kind(1) == OpKind::Register
        {
            base_reg = i.op1_register().full_register();
        }
        if load.is_none()
            && matches!(i.mnemonic(), Mnemonic::Mov | Mnemonic::Movsxd)
            && i.op_kind(0) == OpKind::Register
            && i.op0_register().full_register() == target
            && i.op_kind(1) == OpKind::Memory
            && i.memory_index_scale() == 4
        {
            load = Some(i);
        }
    }
    let load = load.ok_or_else(|| why!("indirect-jmp:no-table-load"))?;
    if base_reg == Register::None {
        base_reg = load.memory_base().full_register();
    }
    let signed = load.mnemonic() == Mnemonic::Movsxd;
    // The table base register is set by `lea base, [rip+X]`; X is the image base for MSVC tables.
    let base_val = prev
        .iter()
        .find(|i| {
            i.mnemonic() == Mnemonic::Lea && i.op0_register().full_register() == base_reg && i.is_ip_rel_memory_operand()
        })
        .map(|i| i.ip_rel_memory_address())
        .ok_or_else(|| why!("indirect-jmp:no-table-base"))?;
    if load.memory_base().full_register() != base_reg {
        return Err(why!("indirect-jmp:table-base-mismatch"));
    }
    let table_va = base_val.wrapping_add(load.memory_displacement64());
    // Bound: nearest `cmp x, imm ; ja default` going back.
    let mut bound = None;
    let list: Vec<&Instruction> = prev.clone();
    for w in list.windows(2) {
        let (jcc, cmp) = (w[0], w[1]);
        if cmp.mnemonic() == Mnemonic::Cmp && cmp.op_kind(1) != OpKind::Register && cmp.op_kind(1) != OpKind::Memory {
            match jcc.mnemonic() {
                Mnemonic::Ja => bound = Some(cmp.immediate(1) + 1),
                Mnemonic::Jae => bound = Some(cmp.immediate(1)),
                _ => continue,
            }
            break;
        }
    }
    let mut n = bound.ok_or_else(|| why!("indirect-jmp:no-table-bound"))? as usize;
    if n == 0 || n > 4096 {
        return Err(why!("indirect-jmp:table-size"));
    }
    // Two-level tables: a byte map selects the entry; the table holds max(map)+1 entries.
    if let Some(map) = prev.iter().find(|i| {
        i.mnemonic() == Mnemonic::Movzx
            && i.op_kind(1) == OpKind::Memory
            && i.memory_index_scale() == 1
            && i.memory_size() == iced_x86::MemorySize::UInt8
    }) {
        let map_va = base_val.wrapping_add(map.memory_displacement64());
        let rva = img.va_to_rva(map_va).ok_or_else(|| why!("indirect-jmp:map-outside"))?;
        let bytes = img.slice(rva, n).ok_or_else(|| why!("indirect-jmp:map-outside"))?;
        n = usize::from(*bytes.iter().max().unwrap_or(&0)) + 1;
    }
    let trva = img.va_to_rva(table_va).ok_or_else(|| why!("indirect-jmp:table-outside"))?;
    let mut targets = Vec::with_capacity(n);
    for k in 0..n {
        let e = img.u32(trva + 4 * k as u32).ok_or_else(|| why!("indirect-jmp:table-outside"))?;
        let off = if signed { i64::from(e as i32) as u64 } else { u64::from(e) };
        let t = base_val.wrapping_add(off);
        if !in_ranges(root, t) || !insns.contains_key(&t) && fetch(img, t).is_none() {
            return Err(why!("indirect-jmp:table-target"));
        }
        targets.push(t);
    }
    targets.sort_unstable();
    targets.dedup();
    Ok(targets)
}

fn decode_function(img: &Image, root: &Root) -> R<Decoded> {
    let mut insns: BTreeMap<u64, Instruction> = BTreeMap::new();
    let mut leaders: BTreeSet<u64> = BTreeSet::new();
    let mut tables = HashMap::new();
    let mut work = vec![root.va];
    leaders.insert(root.va);
    while let Some(mut ip) = work.pop() {
        loop {
            if insns.contains_key(&ip) {
                break;
            }
            if !in_ranges(root, ip) {
                return Err(why!("flow:leaves-function"));
            }
            let i = fetch(img, ip).ok_or_else(|| why!("decode:invalid"))?;
            insns.insert(ip, i);
            let next = i.next_ip();
            match i.flow_control() {
                FlowControl::Next | FlowControl::Call | FlowControl::IndirectCall | FlowControl::XbeginXabortXend => {
                    ip = next;
                }
                FlowControl::ConditionalBranch => {
                    let t = i.near_branch_target();
                    leaders.insert(t);
                    leaders.insert(next);
                    work.push(t);
                    ip = next;
                }
                FlowControl::UnconditionalBranch => {
                    let t = i.near_branch_target();
                    if in_ranges(root, t) {
                        leaders.insert(t);
                        work.push(t);
                    }
                    break;
                }
                FlowControl::IndirectBranch => {
                    if i.op0_kind() == OpKind::Register {
                        let ts = table_targets(img, root, &insns, &i)?;
                        for &t in &ts {
                            leaders.insert(t);
                            work.push(t);
                        }
                        tables.insert(ip, ts);
                    } else if i.is_ip_rel_memory_operand() {
                        // Tail call through an import slot.
                    } else {
                        return Err(why!("indirect-jmp:memory"));
                    }
                    break;
                }
                FlowControl::Return | FlowControl::Exception | FlowControl::Interrupt => break,
            }
        }
    }
    Ok(Decoded { insns, leaders, tables })
}

fn ty_of(bits: u32) -> Type {
    match bits {
        8 => types::I8,
        16 => types::I16,
        32 => types::I32,
        _ => types::I64,
    }
}

fn mask_of(bits: u32) -> u64 {
    if bits >= 64 { u64::MAX } else { (1u64 << bits) - 1 }
}

fn is_high8(r: Register) -> bool {
    matches!(r, Register::AH | Register::CH | Register::DH | Register::BH)
}

struct Sigs {
    call: SigRef,
    trap: SigRef,
    div: SigRef,
}

struct Tx<'a, 'f, M: Module> {
    b: FunctionBuilder<'f>,
    m: &'a mut M,
    cfg: TargetFrontendConfig,
    img: &'a Image,
    plan: &'a Plan,
    cpu: Value,
    gpr: [Variable; 16],
    cf: Variable,
    zf: Variable,
    sf: Variable,
    of: Variable,
    pf: Variable,
    blocks: BTreeMap<u64, Block>,
    exit: Option<Block>,
    sigs: Sigs,
    meta: FuncMeta,
}

#[derive(PartialEq, Eq)]
enum Flow {
    Next,
    Done,
}

impl<M: Module> Tx<'_, '_, M> {
    fn gpr_val(&mut self, n: usize) -> Value {
        self.b.use_var(self.gpr[n])
    }

    fn get_reg(&mut self, r: Register) -> R<Value> {
        if !r.is_gpr() {
            return Err(why!("operand:non-gpr {r:?}"));
        }
        let n = r.full_register().number();
        let full = self.gpr_val(n);
        Ok(match r.size() {
            8 => full,
            4 => self.b.ins().ireduce(types::I32, full),
            2 => self.b.ins().ireduce(types::I16, full),
            _ if is_high8(r) => {
                let s = self.b.ins().ushr_imm_u(full, 8);
                self.b.ins().ireduce(types::I8, s)
            }
            _ => self.b.ins().ireduce(types::I8, full),
        })
    }

    fn set_reg(&mut self, r: Register, v: Value) -> R<()> {
        if !r.is_gpr() {
            return Err(why!("operand:non-gpr {r:?}"));
        }
        let n = r.full_register().number();
        let new = match r.size() {
            8 => v,
            4 => self.b.ins().uextend(types::I64, v),
            size => {
                let old = self.gpr_val(n);
                let ext = self.b.ins().uextend(types::I64, v);
                let (keep, ins) = if is_high8(r) {
                    (!0xff00u64, self.b.ins().ishl_imm_u(ext, 8))
                } else {
                    (!mask_of(size as u32 * 8), ext)
                };
                let kept = self.b.ins().band_imm_u(old, keep as i64);
                self.b.ins().bor(kept, ins)
            }
        };
        self.b.def_var(self.gpr[n], new);
        Ok(())
    }

    fn iconst(&mut self, bits: u32, v: u64) -> Value {
        self.b.ins().iconst(ty_of(bits), (v & mask_of(bits)) as i64)
    }

    fn mem_addr(&mut self, i: &Instruction) -> R<Value> {
        if matches!(i.segment_prefix(), Register::FS | Register::GS) {
            self.meta.teb = true;
            return Err(why!("segment:{:?}", i.segment_prefix()));
        }
        let base = i.memory_base();
        let index = i.memory_index();
        if base == Register::EIP || (base != Register::None && base != Register::RIP && !base.is_gpr64())
            || (index != Register::None && !index.is_gpr64())
        {
            return Err(why!("operand:addr32"));
        }
        let mut acc: Option<Value> = None;
        let add = |tx: &mut Self, v: Value, acc: &mut Option<Value>| {
            *acc = Some(match *acc {
                Some(a) => tx.b.ins().iadd(a, v),
                None => v,
            });
        };
        let mut disp = i.memory_displacement64();
        if base == Register::RIP {
            disp = i.ip_rel_memory_address();
        } else if base != Register::None {
            let b = self.get_reg(base)?;
            add(self, b, &mut acc);
        }
        if index != Register::None {
            let mut x = self.get_reg(index)?;
            let sc = i.memory_index_scale();
            if sc > 1 {
                x = self.b.ins().ishl_imm_u(x, i64::from(sc.trailing_zeros()));
            }
            add(self, x, &mut acc);
        }
        Ok(match acc {
            Some(a) if disp == 0 => a,
            Some(a) => self.b.ins().iadd_imm_s(a, disp as i64),
            None => self.b.ins().iconst(types::I64, disp as i64),
        })
    }

    fn op_bits(&self, i: &Instruction, n: u32) -> R<u32> {
        match i.op_kind(n) {
            OpKind::Register => Ok(i.op_register(n).size() as u32 * 8),
            OpKind::Memory => Ok(i.memory_size().size() as u32 * 8),
            k => Err(why!("operand:width {k:?}")),
        }
    }

    fn read_op(&mut self, i: &Instruction, n: u32, bits: u32) -> R<Value> {
        match i.op_kind(n) {
            OpKind::Register => self.get_reg(i.op_register(n)),
            OpKind::Memory => {
                let a = self.mem_addr(i)?;
                Ok(self.b.ins().load(ty_of(bits), MemFlagsData::new(), a, 0))
            }
            OpKind::Immediate8
            | OpKind::Immediate16
            | OpKind::Immediate32
            | OpKind::Immediate64
            | OpKind::Immediate8to16
            | OpKind::Immediate8to32
            | OpKind::Immediate8to64
            | OpKind::Immediate32to64 => Ok(self.iconst(bits, i.immediate(n))),
            k => Err(why!("operand:read {k:?}")),
        }
    }

    fn write_op(&mut self, i: &Instruction, n: u32, v: Value) -> R<()> {
        match i.op_kind(n) {
            OpKind::Register => self.set_reg(i.op_register(n), v),
            OpKind::Memory => {
                let a = self.mem_addr(i)?;
                self.b.ins().store(MemFlagsData::new(), v, a, 0);
                Ok(())
            }
            k => Err(why!("operand:write {k:?}")),
        }
    }

    fn flag(&mut self, v: Variable, val: Value) {
        self.b.def_var(v, val);
    }

    fn set_zsp(&mut self, res: Value, bits: u32) {
        let zf = self.b.ins().icmp_imm_s(IntCC::Equal, res, 0);
        let sf = self.b.ins().icmp_imm_s(IntCC::SignedLessThan, res, 0);
        let low = if bits > 8 { self.b.ins().ireduce(types::I8, res) } else { res };
        let pc = self.b.ins().popcnt(low);
        let odd = self.b.ins().band_imm_u(pc, 1);
        let pf = self.b.ins().icmp_imm_s(IntCC::Equal, odd, 0);
        self.flag(self.zf, zf);
        self.flag(self.sf, sf);
        self.flag(self.pf, pf);
    }

    fn set_logic_flags(&mut self, res: Value, bits: u32) {
        let z = self.b.ins().iconst(types::I8, 0);
        self.flag(self.cf, z);
        self.flag(self.of, z);
        self.set_zsp(res, bits);
    }

    /// `a + b (+ cin)` flags. `res` is the already computed sum.
    fn set_add_flags(&mut self, a: Value, b: Value, res: Value, cin: Option<Value>, bits: u32) {
        let mut cf = self.b.ins().icmp(IntCC::UnsignedLessThan, res, a);
        if let Some(c) = cin {
            let same = self.b.ins().icmp(IntCC::Equal, res, a);
            let c8 = self.b.ins().icmp_imm_s(IntCC::NotEqual, c, 0);
            let extra = self.b.ins().band(same, c8);
            cf = self.b.ins().bor(cf, extra);
        }
        let x = self.b.ins().bxor(a, res);
        let y = self.b.ins().bxor(b, res);
        let t = self.b.ins().band(x, y);
        let of = self.b.ins().icmp_imm_s(IntCC::SignedLessThan, t, 0);
        self.flag(self.cf, cf);
        self.flag(self.of, of);
        self.set_zsp(res, bits);
    }

    fn set_sub_flags(&mut self, a: Value, b: Value, res: Value, cin: Option<Value>, bits: u32) {
        let mut cf = self.b.ins().icmp(IntCC::UnsignedLessThan, a, b);
        if let Some(c) = cin {
            let same = self.b.ins().icmp(IntCC::Equal, a, b);
            let c8 = self.b.ins().icmp_imm_s(IntCC::NotEqual, c, 0);
            let extra = self.b.ins().band(same, c8);
            cf = self.b.ins().bor(cf, extra);
        }
        let x = self.b.ins().bxor(a, b);
        let y = self.b.ins().bxor(a, res);
        let t = self.b.ins().band(x, y);
        let of = self.b.ins().icmp_imm_s(IntCC::SignedLessThan, t, 0);
        self.flag(self.cf, cf);
        self.flag(self.of, of);
        self.set_zsp(res, bits);
    }

    fn not8(&mut self, v: Value) -> Value {
        self.b.ins().bxor_imm_u(v, 1)
    }

    fn cond(&mut self, cc: ConditionCode) -> R<Value> {
        let cf = self.b.use_var(self.cf);
        let zf = self.b.use_var(self.zf);
        let sf = self.b.use_var(self.sf);
        let of = self.b.use_var(self.of);
        let pf = self.b.use_var(self.pf);
        use ConditionCode as C;
        Ok(match cc {
            C::o => of,
            C::no => self.not8(of),
            C::b => cf,
            C::ae => self.not8(cf),
            C::e => zf,
            C::ne => self.not8(zf),
            C::be => self.b.ins().bor(cf, zf),
            C::a => {
                let t = self.b.ins().bor(cf, zf);
                self.not8(t)
            }
            C::s => sf,
            C::ns => self.not8(sf),
            C::p => pf,
            C::np => self.not8(pf),
            C::l => self.b.ins().bxor(sf, of),
            C::ge => {
                let t = self.b.ins().bxor(sf, of);
                self.not8(t)
            }
            C::le => {
                let t = self.b.ins().bxor(sf, of);
                self.b.ins().bor(zf, t)
            }
            C::g => {
                let t = self.b.ins().bxor(sf, of);
                let u = self.b.ins().bor(zf, t);
                self.not8(u)
            }
            C::None => return Err(why!("flags:no-condition")),
        })
    }

    fn writeback(&mut self) {
        for n in 0..16 {
            let v = self.gpr_val(n);
            self.b.ins().store(MemFlagsData::new(), v, self.cpu, Cpu::OFF_GPR + 8 * n as i32);
        }
    }

    fn reload(&mut self) {
        for n in 0..16 {
            let v = self.b.ins().load(types::I64, MemFlagsData::new(), self.cpu, Cpu::OFF_GPR + 8 * n as i32);
            self.b.def_var(self.gpr[n], v);
        }
    }

    fn exit_block(&mut self) -> Block {
        if let Some(b) = self.exit {
            return b;
        }
        let blk = self.b.create_block();
        self.exit = Some(blk);
        blk
    }

    /// After a call returned: stop if the callee trapped, else reload the registers.
    fn after_call(&mut self) {
        let t = self.b.ins().load(types::I64, MemFlagsData::new(), self.cpu, Cpu::OFF_TRAP);
        let cont = self.b.create_block();
        let ex = self.exit_block();
        self.b.ins().brif(t, ex, &[], cont, &[]);
        self.b.switch_to_block(cont);
        self.reload();
    }

    fn rt_field(&mut self, off: i32) -> Value {
        let rt = self.b.ins().load(types::I64, MemFlagsData::new(), self.cpu, Cpu::OFF_RT);
        self.b.ins().load(types::I64, MemFlagsData::new(), rt, off)
    }

    fn rt_call(&mut self, target: Value) {
        let ctx = self.rt_field(Rt::OFF_CTX);
        let f = self.rt_field(Rt::OFF_CALL);
        self.b.ins().call_indirect(self.sigs.call, f, &[ctx, self.cpu, target]);
    }

    fn rt_trap(&mut self, kind: TrapKind, va: u64) {
        let ctx = self.rt_field(Rt::OFF_CTX);
        let f = self.rt_field(Rt::OFF_TRAP);
        let k = self.b.ins().iconst(types::I64, kind as i64);
        let a = self.b.ins().iconst(types::I64, va as i64);
        self.b.ins().call_indirect(self.sigs.trap, f, &[ctx, self.cpu, k, a]);
    }

    fn func_ref(&mut self, va: u64) -> R<ir::FuncRef> {
        let id: FuncId = self
            .m
            .declare_function(&symbol(va), Linkage::Export, &lifted_sig())
            .map_err(|e| why!("module:{e}"))?;
        Ok(self.m.declare_func_in_func(id, self.b.func))
    }

    fn push(&mut self, v: Value) {
        let rsp = self.gpr_val(4);
        let nrsp = self.b.ins().iadd_imm_s(rsp, -8);
        self.b.ins().store(MemFlagsData::new(), v, nrsp, 0);
        self.b.def_var(self.gpr[4], nrsp);
    }

    fn pop(&mut self) -> Value {
        let rsp = self.gpr_val(4);
        let v = self.b.ins().load(types::I64, MemFlagsData::new(), rsp, 0);
        let nrsp = self.b.ins().iadd_imm_s(rsp, 8);
        self.b.def_var(self.gpr[4], nrsp);
        v
    }

    fn block_of(&self, va: u64) -> R<Block> {
        self.blocks.get(&va).copied().ok_or_else(|| why!("flow:no-block {va:#x}"))
    }

    fn emit_call(&mut self, i: &Instruction) -> R<()> {
        let ret = self.b.ins().iconst(types::I64, i.next_ip() as i64);
        match i.op0_kind() {
            OpKind::NearBranch64 => {
                let t = i.near_branch_target();
                self.push(ret);
                self.writeback();
                if self.plan.is_start(t) {
                    self.meta.callees.insert(t);
                    let fr = self.func_ref(t)?;
                    self.b.ins().call(fr, &[self.cpu]);
                } else {
                    self.meta.dynamic = true;
                    let tv = self.b.ins().iconst(types::I64, t as i64);
                    self.rt_call(tv);
                }
            }
            _ => {
                let tgt = self.read_op(i, 0, 64)?;
                self.push(ret);
                self.writeback();
                self.meta.dynamic = true;
                self.rt_call(tgt);
            }
        }
        self.after_call();
        Ok(())
    }

    fn emit_ret(&mut self, i: &Instruction) {
        let extra = if i.op_count() == 1 { i.immediate(0) as i64 } else { 0 };
        let rsp = self.gpr_val(4);
        let n = self.b.ins().iadd_imm_s(rsp, 8 + extra);
        self.b.def_var(self.gpr[4], n);
        self.writeback();
        self.b.ins().return_(&[]);
    }

    fn emit_jmp(&mut self, i: &Instruction, dec: &Decoded) -> R<()> {
        if i.flow_control() == FlowControl::IndirectBranch {
            if i.op0_kind() == OpKind::Register {
                let ts = &dec.tables[&i.ip()];
                let v = self.get_reg(i.op0_register())?;
                let dflt = self.b.create_block();
                let mut sw = Switch::new();
                for &t in ts {
                    let blk = self.block_of(t)?;
                    sw.set_entry(u128::from(t), blk);
                }
                sw.emit(&mut self.b, v, dflt);
                self.b.switch_to_block(dflt);
                self.rt_trap(TrapKind::Instruction, i.ip());
                self.b.ins().return_(&[]);
            } else {
                // Tail call through an import slot: the slot holds the callee address.
                let tgt = self.read_op(i, 0, 64)?;
                self.meta.dynamic = true;
                self.writeback();
                self.rt_call(tgt);
                self.b.ins().return_(&[]);
            }
            return Ok(());
        }
        let t = i.near_branch_target();
        if in_ranges_plan(self.plan, self.img, t, &self.blocks) {
            let blk = self.block_of(t)?;
            self.b.ins().jump(blk, &[]);
        } else if self.plan.is_start(t) {
            self.meta.callees.insert(t);
            self.writeback();
            let fr = self.func_ref(t)?;
            self.b.ins().call(fr, &[self.cpu]);
            self.b.ins().return_(&[]);
        } else {
            return Err(why!("flow:jmp-out"));
        }
        Ok(())
    }

    fn shift(&mut self, i: &Instruction) -> R<()> {
        let bits = self.op_bits(i, 0)?;
        let ty = ty_of(bits);
        let x = self.read_op(i, 0, bits)?;
        let imm = i.op_kind(1) != OpKind::Register;
        let cmask = if bits == 64 { 63 } else { 31 };
        let (cnt64, statically_zero) = if imm {
            let c = i.immediate(1) & cmask;
            (self.b.ins().iconst(types::I64, c as i64), c == 0)
        } else {
            let c = self.get_reg(Register::CL)?;
            let c = self.b.ins().uextend(types::I64, c);
            (self.b.ins().band_imm_u(c, cmask as i64), false)
        };
        if statically_zero {
            return Ok(());
        }
        let (old_cf, old_zf, old_sf, old_of, old_pf) = (
            self.b.use_var(self.cf),
            self.b.use_var(self.zf),
            self.b.use_var(self.sf),
            self.b.use_var(self.of),
            self.b.use_var(self.pf),
        );
        let m = i.mnemonic();
        let (res, cf, of_override): (Value, Value, Option<Value>) = match m {
            Mnemonic::Shl | Mnemonic::Sal | Mnemonic::Shr | Mnemonic::Sar => {
                let x64 = if m == Mnemonic::Sar { self.b.ins().sextend(types::I64, x) } else if bits < 64 { self.b.ins().uextend(types::I64, x) } else { x };
                let r64 = match m {
                    Mnemonic::Shl | Mnemonic::Sal => self.b.ins().ishl(x64, cnt64),
                    Mnemonic::Shr => self.b.ins().ushr(x64, cnt64),
                    _ => self.b.ins().sshr(x64, cnt64),
                };
                let res = if bits < 64 { self.b.ins().ireduce(ty, r64) } else { r64 };
                let cf = if matches!(m, Mnemonic::Shl | Mnemonic::Sal) {
                    let kb = self.b.ins().iconst(types::I64, i64::from(bits));
                    let sh = self.b.ins().isub(kb, cnt64);
                    let t = self.b.ins().ushr(x64, sh);
                    let t = self.b.ins().band_imm_u(t, 1);
                    self.b.ins().icmp_imm_s(IntCC::NotEqual, t, 0)
                } else {
                    let c1 = self.b.ins().iadd_imm_s(cnt64, -1);
                    let t = if m == Mnemonic::Sar { self.b.ins().sshr(x64, c1) } else { self.b.ins().ushr(x64, c1) };
                    let t = self.b.ins().band_imm_u(t, 1);
                    self.b.ins().icmp_imm_s(IntCC::NotEqual, t, 0)
                };
                let of = match m {
                    Mnemonic::Shl | Mnemonic::Sal => {
                        let msb = self.b.ins().icmp_imm_s(IntCC::SignedLessThan, res, 0);
                        Some(self.b.ins().bxor(msb, cf))
                    }
                    Mnemonic::Shr => Some(self.b.ins().icmp_imm_s(IntCC::SignedLessThan, x, 0)),
                    _ => Some(self.b.ins().iconst(types::I8, 0)),
                };
                (res, cf, of)
            }
            Mnemonic::Rol | Mnemonic::Ror => {
                let c = if bits == 64 { cnt64 } else { self.b.ins().ireduce(types::I32, cnt64) };
                let c = if bits == 64 { c } else { self.b.ins().uextend(types::I64, c) };
                let cty = if bits == 64 { c } else { self.b.ins().ireduce(ty, c) };
                let res = if m == Mnemonic::Rol { self.b.ins().rotl(x, cty) } else { self.b.ins().rotr(x, cty) };
                let (cf, of) = if m == Mnemonic::Rol {
                    let lsb = self.b.ins().band_imm_u(res, 1);
                    let cf = self.b.ins().icmp_imm_s(IntCC::NotEqual, lsb, 0);
                    let msb = self.b.ins().icmp_imm_s(IntCC::SignedLessThan, res, 0);
                    (cf, self.b.ins().bxor(msb, cf))
                } else {
                    let cf = self.b.ins().icmp_imm_s(IntCC::SignedLessThan, res, 0);
                    let sh = self.b.ins().ushr_imm_u(res, i64::from(bits - 2));
                    let b2 = self.b.ins().band_imm_u(sh, 1);
                    let b2 = self.b.ins().icmp_imm_s(IntCC::NotEqual, b2, 0);
                    (cf, self.b.ins().bxor(cf, b2))
                };
                self.write_op(i, 0, res)?;
                let nz = self.b.ins().icmp_imm_s(IntCC::NotEqual, cnt64, 0);
                let cf = self.b.ins().select(nz, cf, old_cf);
                let of = self.b.ins().select(nz, of, old_of);
                self.flag(self.cf, cf);
                self.flag(self.of, of);
                return Ok(());
            }
            _ => return Err(why!("insn:{m:?}")),
        };
        self.write_op(i, 0, res)?;
        // A zero count leaves the flags alone; for immediates the count is nonzero here.
        let zero_count = if imm { None } else { Some(self.b.ins().icmp_imm_s(IntCC::Equal, cnt64, 0)) };
        let (save_zf, save_sf, save_pf) = (old_zf, old_sf, old_pf);
        self.set_zsp(res, bits);
        let (nzf, nsf, npf) = (self.b.use_var(self.zf), self.b.use_var(self.sf), self.b.use_var(self.pf));
        let of = of_override.unwrap_or(old_of);
        let (fcf, fof, fzf, fsf, fpf) = match zero_count {
            Some(z) => (
                self.b.ins().select(z, old_cf, cf),
                self.b.ins().select(z, old_of, of),
                self.b.ins().select(z, save_zf, nzf),
                self.b.ins().select(z, save_sf, nsf),
                self.b.ins().select(z, save_pf, npf),
            ),
            None => (cf, of, nzf, nsf, npf),
        };
        self.flag(self.cf, fcf);
        self.flag(self.of, fof);
        self.flag(self.zf, fzf);
        self.flag(self.sf, fsf);
        self.flag(self.pf, fpf);
        Ok(())
    }

    fn imul(&mut self, i: &Instruction) -> R<()> {
        let bits = self.op_bits(i, 0)?;
        let ty = ty_of(bits);
        let (a, b) = match i.op_count() {
            2 => (self.read_op(i, 0, bits)?, self.read_op(i, 1, bits)?),
            3 => (self.read_op(i, 1, bits)?, self.read_op(i, 2, bits)?),
            _ => return Err(why!("insn:Imul1")),
        };
        let res = self.b.ins().imul(a, b);
        let overflow = if bits == 64 {
            let hi = self.b.ins().smulhi(a, b);
            let sign = self.b.ins().sshr_imm_s(res, 63);
            self.b.ins().icmp(IntCC::NotEqual, hi, sign)
        } else {
            let a64 = self.b.ins().sextend(types::I64, a);
            let b64 = self.b.ins().sextend(types::I64, b);
            let full = self.b.ins().imul(a64, b64);
            let lo = self.b.ins().ireduce(ty, full);
            let back = self.b.ins().sextend(types::I64, lo);
            self.b.ins().icmp(IntCC::NotEqual, full, back)
        };
        self.write_op(i, 0, res)?;
        self.flag(self.cf, overflow);
        self.flag(self.of, overflow);
        self.set_zsp(res, bits);
        Ok(())
    }

    /// `mul`/`imul`/`div`/`idiv` with one explicit operand.
    fn unary_wide(&mut self, i: &Instruction) -> R<()> {
        let bits = self.op_bits(i, 0)?;
        if bits < 32 {
            return Err(why!("insn:{:?}{bits}", i.mnemonic()));
        }
        let ty = ty_of(bits);
        let src = self.read_op(i, 0, bits)?;
        let rax = self.get_reg(if bits == 64 { Register::RAX } else { Register::EAX })?;
        let rdx = self.get_reg(if bits == 64 { Register::RDX } else { Register::EDX })?;
        let (lo_reg, hi_reg) = if bits == 64 { (Register::RAX, Register::RDX) } else { (Register::EAX, Register::EDX) };
        match i.mnemonic() {
            Mnemonic::Mul | Mnemonic::Imul => {
                let signed = i.mnemonic() == Mnemonic::Imul;
                let (lo, hi) = if bits == 64 {
                    let lo = self.b.ins().imul(rax, src);
                    let hi = if signed { self.b.ins().smulhi(rax, src) } else { self.b.ins().umulhi(rax, src) };
                    (lo, hi)
                } else {
                    let (a, s) = if signed {
                        (self.b.ins().sextend(types::I64, rax), self.b.ins().sextend(types::I64, src))
                    } else {
                        (self.b.ins().uextend(types::I64, rax), self.b.ins().uextend(types::I64, src))
                    };
                    let full = self.b.ins().imul(a, s);
                    let lo = self.b.ins().ireduce(ty, full);
                    let h = self.b.ins().ushr_imm_u(full, 32);
                    (lo, self.b.ins().ireduce(ty, h))
                };
                self.set_reg(lo_reg, lo)?;
                self.set_reg(hi_reg, hi)?;
                let ov = if signed {
                    let sign = self.b.ins().sshr_imm_s(lo, i64::from(bits - 1));
                    self.b.ins().icmp(IntCC::NotEqual, hi, sign)
                } else {
                    self.b.ins().icmp_imm_s(IntCC::NotEqual, hi, 0)
                };
                self.flag(self.cf, ov);
                self.flag(self.of, ov);
            }
            Mnemonic::Div | Mnemonic::Idiv => {
                let signed = i.mnemonic() == Mnemonic::Idiv;
                let (q, r) = if bits == 64 {
                    let ss = self.b.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 16, 3));
                    let out = self.b.ins().stack_addr(types::I64, ss, 0);
                    let ctx = self.rt_field(Rt::OFF_CTX);
                    let f = self.rt_field(Rt::OFF_DIV);
                    let sg = self.b.ins().iconst(types::I64, i64::from(signed));
                    self.b.ins().call_indirect(self.sigs.div, f, &[ctx, rdx, rax, src, sg, out]);
                    (
                        self.b.ins().load(types::I64, MemFlagsData::new(), out, 0),
                        self.b.ins().load(types::I64, MemFlagsData::new(), out, 8),
                    )
                } else {
                    let lo = self.b.ins().uextend(types::I64, rax);
                    let hi = self.b.ins().uextend(types::I64, rdx);
                    let hs = self.b.ins().ishl_imm_u(hi, 32);
                    let n = self.b.ins().bor(hs, lo);
                    if signed {
                        let d = self.b.ins().sextend(types::I64, src);
                        let q = self.b.ins().sdiv(n, d);
                        let r = self.b.ins().srem(n, d);
                        (self.b.ins().ireduce(types::I32, q), self.b.ins().ireduce(types::I32, r))
                    } else {
                        let d = self.b.ins().uextend(types::I64, src);
                        let q = self.b.ins().udiv(n, d);
                        let r = self.b.ins().urem(n, d);
                        (self.b.ins().ireduce(types::I32, q), self.b.ins().ireduce(types::I32, r))
                    }
                };
                self.set_reg(lo_reg, q)?;
                self.set_reg(hi_reg, r)?;
            }
            m => return Err(why!("insn:{m:?}")),
        }
        Ok(())
    }

    fn arith(&mut self, i: &Instruction) -> R<()> {
        let m = i.mnemonic();
        let bits = self.op_bits(i, 0)?;
        let ty = ty_of(bits);
        let a = self.read_op(i, 0, bits)?;
        let b = self.read_op(i, 1, bits)?;
        match m {
            Mnemonic::Add => {
                let r = self.b.ins().iadd(a, b);
                self.set_add_flags(a, b, r, None, bits);
                self.write_op(i, 0, r)?;
            }
            Mnemonic::Sub | Mnemonic::Cmp => {
                let r = self.b.ins().isub(a, b);
                self.set_sub_flags(a, b, r, None, bits);
                if m == Mnemonic::Sub {
                    self.write_op(i, 0, r)?;
                }
            }
            Mnemonic::Adc | Mnemonic::Sbb => {
                let c8 = self.b.use_var(self.cf);
                let c = self.b.ins().uextend(ty, c8);
                if m == Mnemonic::Adc {
                    let s = self.b.ins().iadd(a, b);
                    let r = self.b.ins().iadd(s, c);
                    self.set_add_flags(a, b, r, Some(c), bits);
                    self.write_op(i, 0, r)?;
                } else {
                    let s = self.b.ins().isub(a, b);
                    let r = self.b.ins().isub(s, c);
                    self.set_sub_flags(a, b, r, Some(c), bits);
                    self.write_op(i, 0, r)?;
                }
            }
            Mnemonic::And | Mnemonic::Test => {
                let r = self.b.ins().band(a, b);
                self.set_logic_flags(r, bits);
                if m == Mnemonic::And {
                    self.write_op(i, 0, r)?;
                }
            }
            Mnemonic::Or => {
                let r = self.b.ins().bor(a, b);
                self.set_logic_flags(r, bits);
                self.write_op(i, 0, r)?;
            }
            Mnemonic::Xor => {
                let r = self.b.ins().bxor(a, b);
                self.set_logic_flags(r, bits);
                self.write_op(i, 0, r)?;
            }
            _ => return Err(why!("insn:{m:?}")),
        }
        Ok(())
    }

    fn atomic(&mut self, i: &Instruction) -> R<()> {
        let m = i.mnemonic();
        let bits = self.op_bits(i, 0)?;
        if i.op_kind(0) != OpKind::Memory || bits < 32 {
            return Err(why!("lock:{m:?}"));
        }
        let ty = ty_of(bits);
        let addr = self.mem_addr(i)?;
        let (op, val) = match m {
            Mnemonic::Add | Mnemonic::Sub | Mnemonic::And | Mnemonic::Or | Mnemonic::Xor => {
                let v = self.read_op(i, 1, bits)?;
                (
                    match m {
                        Mnemonic::Add => AtomicRmwOp::Add,
                        Mnemonic::Sub => AtomicRmwOp::Sub,
                        Mnemonic::And => AtomicRmwOp::And,
                        Mnemonic::Or => AtomicRmwOp::Or,
                        _ => AtomicRmwOp::Xor,
                    },
                    v,
                )
            }
            Mnemonic::Inc => (AtomicRmwOp::Add, self.b.ins().iconst(ty, 1)),
            Mnemonic::Dec => (AtomicRmwOp::Sub, self.b.ins().iconst(ty, 1)),
            Mnemonic::Xadd => (AtomicRmwOp::Add, self.get_reg(i.op1_register())?),
            Mnemonic::Xchg => (AtomicRmwOp::Xchg, self.get_reg(i.op1_register())?),
            _ => return Err(why!("lock:{m:?}")),
        };
        let old = self.b.ins().atomic_rmw(ty, MemFlagsData::new(), op, addr, val);
        let res = match op {
            AtomicRmwOp::Add => self.b.ins().iadd(old, val),
            AtomicRmwOp::Sub => self.b.ins().isub(old, val),
            AtomicRmwOp::And => self.b.ins().band(old, val),
            AtomicRmwOp::Or => self.b.ins().bor(old, val),
            AtomicRmwOp::Xor => self.b.ins().bxor(old, val),
            _ => val,
        };
        match op {
            AtomicRmwOp::Add => self.set_add_flags(old, val, res, None, bits),
            AtomicRmwOp::Sub => self.set_sub_flags(old, val, res, None, bits),
            AtomicRmwOp::And | AtomicRmwOp::Or | AtomicRmwOp::Xor => self.set_logic_flags(res, bits),
            _ => {}
        }
        if matches!(m, Mnemonic::Xadd | Mnemonic::Xchg) {
            self.set_reg(i.op1_register(), old)?;
        }
        Ok(())
    }

    fn string_op(&mut self, i: &Instruction) -> R<()> {
        let m = i.mnemonic();
        let (size, is_stos) = match m {
            Mnemonic::Movsb => (1u32, false),
            Mnemonic::Movsw => (2, false),
            Mnemonic::Movsd if i.op_kind(0) == OpKind::MemoryESDI => (4, false),
            Mnemonic::Movsq => (8, false),
            Mnemonic::Stosb => (1, true),
            Mnemonic::Stosw => (2, true),
            Mnemonic::Stosd => (4, true),
            Mnemonic::Stosq => (8, true),
            _ => return Err(why!("insn:{m:?}")),
        };
        let bits = size * 8;
        let ty = ty_of(bits);
        let rep = i.has_rep_prefix();
        let header = self.b.create_block();
        let body = self.b.create_block();
        let done = self.b.create_block();
        if rep {
            self.b.ins().jump(header, &[]);
            self.b.switch_to_block(header);
            let rcx = self.gpr_val(1);
            self.b.ins().brif(rcx, body, &[], done, &[]);
            self.b.switch_to_block(body);
        }
        let rdi = self.gpr_val(7);
        if is_stos {
            let v = self.get_reg(match size {
                1 => Register::AL,
                2 => Register::AX,
                4 => Register::EAX,
                _ => Register::RAX,
            })?;
            self.b.ins().store(MemFlagsData::new(), v, rdi, 0);
        } else {
            let rsi = self.gpr_val(6);
            let v = self.b.ins().load(ty, MemFlagsData::new(), rsi, 0);
            self.b.ins().store(MemFlagsData::new(), v, rdi, 0);
            let n = self.b.ins().iadd_imm_s(rsi, i64::from(size));
            self.b.def_var(self.gpr[6], n);
        }
        let n = self.b.ins().iadd_imm_s(rdi, i64::from(size));
        self.b.def_var(self.gpr[7], n);
        if rep {
            let rcx = self.gpr_val(1);
            let n = self.b.ins().iadd_imm_s(rcx, -1);
            self.b.def_var(self.gpr[1], n);
            self.b.ins().jump(header, &[]);
            self.b.switch_to_block(done);
        }
        Ok(())
    }

    fn insn(&mut self, i: &Instruction, dec: &Decoded) -> R<Flow> {
        use Mnemonic as M;
        let m = i.mnemonic();
        if i.has_lock_prefix() {
            self.atomic(i)?;
            return Ok(Flow::Next);
        }
        if (i.has_rep_prefix() || i.has_repne_prefix()) && !matches!(m, M::Movsb | M::Movsw | M::Movsd | M::Movsq | M::Stosb | M::Stosw | M::Stosd | M::Stosq) {
            return Err(why!("insn:rep {m:?}"));
        }
        match m {
            M::Nop | M::Pause | M::Endbr64 | M::Endbr32 | M::Prefetcht0 | M::Prefetcht1 | M::Prefetcht2 | M::Prefetchnta | M::Prefetchw | M::Lfence | M::Mfence | M::Sfence | M::Cld => {}
            M::Mov => {
                let bits = self.op_bits(i, 0)?;
                let v = self.read_op(i, 1, bits)?;
                self.write_op(i, 0, v)?;
            }
            M::Movzx | M::Movsx | M::Movsxd => {
                let dbits = self.op_bits(i, 0)?;
                let sbits = self.op_bits(i, 1)?;
                let v = self.read_op(i, 1, sbits)?;
                let r = if sbits >= dbits {
                    v
                } else if m == M::Movzx {
                    self.b.ins().uextend(ty_of(dbits), v)
                } else {
                    self.b.ins().sextend(ty_of(dbits), v)
                };
                self.write_op(i, 0, r)?;
            }
            M::Lea => {
                let a = self.mem_addr(i)?;
                let bits = self.op_bits(i, 0)?;
                let v = if bits == 64 { a } else { self.b.ins().ireduce(ty_of(bits), a) };
                self.write_op(i, 0, v)?;
            }
            M::Xchg => {
                let bits = self.op_bits(i, 0)?;
                if i.op_kind(0) == OpKind::Memory || i.op_kind(1) == OpKind::Memory {
                    let (mi, ri) = if i.op_kind(0) == OpKind::Memory { (0, 1) } else { (1, 0) };
                    if bits < 32 {
                        return Err(why!("insn:Xchg{bits}"));
                    }
                    let addr = self.mem_addr(i)?;
                    let r = self.get_reg(i.op_register(ri))?;
                    let old = self.b.ins().atomic_rmw(ty_of(bits), MemFlagsData::new(), AtomicRmwOp::Xchg, addr, r);
                    self.set_reg(i.op_register(ri), old)?;
                    let _ = mi;
                } else {
                    let a = self.get_reg(i.op0_register())?;
                    let b = self.get_reg(i.op1_register())?;
                    self.set_reg(i.op0_register(), b)?;
                    self.set_reg(i.op1_register(), a)?;
                }
            }
            M::Push => {
                if i.op_kind(0) == OpKind::Register && i.op_register(0).size() != 8 {
                    return Err(why!("insn:Push16"));
                }
                let v = self.read_op(i, 0, 64)?;
                self.push(v);
            }
            M::Pop => {
                if i.op_kind(0) != OpKind::Register || i.op_register(0).size() != 8 {
                    return Err(why!("insn:PopMem"));
                }
                let v = self.pop();
                self.set_reg(i.op_register(0), v)?;
            }
            M::Leave => {
                let rbp = self.gpr_val(5);
                self.b.def_var(self.gpr[4], rbp);
                let v = self.pop();
                self.b.def_var(self.gpr[5], v);
            }
            M::Add | M::Sub | M::Cmp | M::Adc | M::Sbb | M::And | M::Or | M::Xor | M::Test => self.arith(i)?,
            M::Inc | M::Dec | M::Neg | M::Not => {
                let bits = self.op_bits(i, 0)?;
                let ty = ty_of(bits);
                let a = self.read_op(i, 0, bits)?;
                match m {
                    M::Inc => {
                        let one = self.b.ins().iconst(ty, 1);
                        let r = self.b.ins().iadd(a, one);
                        let cf = self.b.use_var(self.cf);
                        self.set_add_flags(a, one, r, None, bits);
                        self.flag(self.cf, cf);
                        self.write_op(i, 0, r)?;
                    }
                    M::Dec => {
                        let one = self.b.ins().iconst(ty, 1);
                        let r = self.b.ins().isub(a, one);
                        let cf = self.b.use_var(self.cf);
                        self.set_sub_flags(a, one, r, None, bits);
                        self.flag(self.cf, cf);
                        self.write_op(i, 0, r)?;
                    }
                    M::Neg => {
                        let zero = self.b.ins().iconst(ty, 0);
                        let r = self.b.ins().isub(zero, a);
                        self.set_sub_flags(zero, a, r, None, bits);
                        self.write_op(i, 0, r)?;
                    }
                    _ => {
                        let r = self.b.ins().bnot(a);
                        self.write_op(i, 0, r)?;
                    }
                }
            }
            M::Shl | M::Sal | M::Shr | M::Sar | M::Rol | M::Ror => self.shift(i)?,
            M::Imul if i.op_count() >= 2 => self.imul(i)?,
            M::Imul | M::Mul | M::Div | M::Idiv => self.unary_wide(i)?,
            M::Cdq => {
                let eax = self.get_reg(Register::EAX)?;
                let s = self.b.ins().sshr_imm_s(eax, 31);
                self.set_reg(Register::EDX, s)?;
            }
            M::Cqo => {
                let rax = self.gpr_val(0);
                let s = self.b.ins().sshr_imm_s(rax, 63);
                self.b.def_var(self.gpr[2], s);
            }
            M::Cdqe => {
                let eax = self.get_reg(Register::EAX)?;
                let v = self.b.ins().sextend(types::I64, eax);
                self.b.def_var(self.gpr[0], v);
            }
            M::Cwde => {
                let ax = self.get_reg(Register::AX)?;
                let v = self.b.ins().sextend(types::I32, ax);
                self.set_reg(Register::EAX, v)?;
            }
            M::Bswap => {
                let bits = self.op_bits(i, 0)?;
                let a = self.read_op(i, 0, bits)?;
                let r = self.b.ins().bswap(a);
                self.write_op(i, 0, r)?;
            }
            M::Popcnt | M::Lzcnt | M::Tzcnt => {
                let bits = self.op_bits(i, 0)?;
                if bits < 32 {
                    return Err(why!("insn:{m:?}{bits}"));
                }
                let a = self.read_op(i, 1, bits)?;
                let r = match m {
                    M::Popcnt => self.b.ins().popcnt(a),
                    M::Lzcnt => self.b.ins().clz(a),
                    _ => self.b.ins().ctz(a),
                };
                let zero_src = self.b.ins().icmp_imm_s(IntCC::Equal, a, 0);
                if m == M::Popcnt {
                    self.flag(self.zf, zero_src);
                    let z = self.b.ins().iconst(types::I8, 0);
                    self.flag(self.cf, z);
                    self.flag(self.of, z);
                    self.flag(self.sf, z);
                    self.flag(self.pf, z);
                } else {
                    self.flag(self.cf, zero_src);
                    let zr = self.b.ins().icmp_imm_s(IntCC::Equal, r, 0);
                    self.flag(self.zf, zr);
                }
                self.write_op(i, 0, r)?;
            }
            M::Bsf | M::Bsr => {
                let bits = self.op_bits(i, 0)?;
                if bits < 32 {
                    return Err(why!("insn:{m:?}{bits}"));
                }
                let a = self.read_op(i, 1, bits)?;
                let old = self.read_op(i, 0, bits)?;
                let r = if m == M::Bsf {
                    self.b.ins().ctz(a)
                } else {
                    let c = self.b.ins().clz(a);
                    let top = self.b.ins().iconst(ty_of(bits), i64::from(bits - 1));
                    self.b.ins().isub(top, c)
                };
                let z = self.b.ins().icmp_imm_s(IntCC::Equal, a, 0);
                self.flag(self.zf, z);
                let r = self.b.ins().select(z, old, r);
                self.write_op(i, 0, r)?;
            }
            M::Bt | M::Bts | M::Btr | M::Btc => {
                if i.op_kind(0) != OpKind::Register {
                    return Err(why!("insn:{m:?}mem"));
                }
                let bits = self.op_bits(i, 0)?;
                let a = self.read_op(i, 0, bits)?;
                let idx = self.read_op(i, 1, bits)?;
                let idx = if bits < 64 { self.b.ins().uextend(types::I64, idx) } else { idx };
                let idx = self.b.ins().band_imm_u(idx, i64::from(bits - 1));
                let a64 = if bits < 64 { self.b.ins().uextend(types::I64, a) } else { a };
                let bit = self.b.ins().ushr(a64, idx);
                let bit = self.b.ins().band_imm_u(bit, 1);
                let cf = self.b.ins().icmp_imm_s(IntCC::NotEqual, bit, 0);
                self.flag(self.cf, cf);
                if m != M::Bt {
                    let one = self.b.ins().iconst(types::I64, 1);
                    let mk = self.b.ins().ishl(one, idx);
                    let r64 = match m {
                        M::Bts => self.b.ins().bor(a64, mk),
                        M::Btr => {
                            let n = self.b.ins().bnot(mk);
                            self.b.ins().band(a64, n)
                        }
                        _ => self.b.ins().bxor(a64, mk),
                    };
                    let r = if bits < 64 { self.b.ins().ireduce(ty_of(bits), r64) } else { r64 };
                    self.write_op(i, 0, r)?;
                }
            }
            M::Clc | M::Stc | M::Cmc => {
                let cf = self.b.use_var(self.cf);
                let v = match m {
                    M::Clc => self.b.ins().iconst(types::I8, 0),
                    M::Stc => self.b.ins().iconst(types::I8, 1),
                    _ => self.not8(cf),
                };
                self.flag(self.cf, v);
            }
            M::Movsb | M::Movsw | M::Movsd | M::Movsq | M::Stosb | M::Stosw | M::Stosd | M::Stosq => self.string_op(i)?,
            m if i.condition_code() != ConditionCode::None && is_setcc(m) => {
                let c = self.cond(i.condition_code())?;
                self.write_op(i, 0, c)?;
            }
            m if i.condition_code() != ConditionCode::None && is_cmovcc(m) => {
                let bits = self.op_bits(i, 0)?;
                let c = self.cond(i.condition_code())?;
                let src = self.read_op(i, 1, bits)?;
                let old = self.read_op(i, 0, bits)?;
                let v = self.b.ins().select(c, src, old);
                self.write_op(i, 0, v)?;
            }
            _ => match i.flow_control() {
                FlowControl::ConditionalBranch => {
                    let c = self.cond(i.condition_code())?;
                    let t = self.block_of(i.near_branch_target())?;
                    let n = self.block_of(i.next_ip())?;
                    self.b.ins().brif(c, t, &[], n, &[]);
                    return Ok(Flow::Done);
                }
                FlowControl::UnconditionalBranch | FlowControl::IndirectBranch => {
                    self.emit_jmp(i, dec)?;
                    return Ok(Flow::Done);
                }
                FlowControl::Call | FlowControl::IndirectCall => self.emit_call(i)?,
                FlowControl::Return => {
                    self.emit_ret(i);
                    return Ok(Flow::Done);
                }
                FlowControl::Exception | FlowControl::Interrupt => {
                    self.writeback();
                    self.rt_trap(TrapKind::Instruction, i.ip());
                    self.b.ins().return_(&[]);
                    return Ok(Flow::Done);
                }
                _ => return Err(why!("insn:{m:?}")),
            },
        }
        Ok(Flow::Next)
    }
}

fn in_ranges_plan(_plan: &Plan, _img: &Image, t: u64, blocks: &BTreeMap<u64, Block>) -> bool {
    blocks.contains_key(&t)
}

fn is_setcc(m: Mnemonic) -> bool {
    use Mnemonic::*;
    matches!(m, Seto | Setno | Setb | Setae | Sete | Setne | Setbe | Seta | Sets | Setns | Setp | Setnp | Setl | Setge | Setle | Setg)
}

fn is_cmovcc(m: Mnemonic) -> bool {
    use Mnemonic::*;
    matches!(m, Cmovo | Cmovno | Cmovb | Cmovae | Cmove | Cmovne | Cmovbe | Cmova | Cmovs | Cmovns | Cmovp | Cmovnp | Cmovl | Cmovge | Cmovle | Cmovg)
}

/// Lifts functions into a Cranelift module.
pub struct Lifter {
    cfg: TargetFrontendConfig,
}

impl Lifter {
    pub fn new(cfg: TargetFrontendConfig) -> Self {
        Self { cfg }
    }

    /// Lift `root` to IR; on success the function is declared in `m` and the IR returned for `define_function`.
    pub fn lift<M: Module>(&mut self, m: &mut M, img: &Image, plan: &Plan, root: &Root) -> R<(FuncId, ir::Function, FuncMeta)> {
        let dec = decode_function(img, root)?;
        let mut func = ir::Function::with_name_signature(ir::UserFuncName::user(1, (root.va >> 4) as u32), lifted_sig());
        let mut fbc = FunctionBuilderContext::new();
        let mut b = FunctionBuilder::new(&mut func, &mut fbc);
        let sigs = Sigs { call: b.import_signature(call_sig()), trap: b.import_signature(trap_sig()), div: b.import_signature(div_sig()) };
        let entry = b.create_block();
        b.append_block_params_for_function_params(entry);
        b.switch_to_block(entry);
        let cpu = b.block_params(entry)[0];
        let gpr = std::array::from_fn(|_| b.declare_var(types::I64));
        let [cf, zf, sf, of, pf] = std::array::from_fn(|_| b.declare_var(types::I8));
        let mut tx = Tx {
            b,
            m,
            cfg: self.cfg,
            img,
            plan,
            cpu,
            gpr,
            cf,
            zf,
            sf,
            of,
            pf,
            blocks: BTreeMap::new(),
            exit: None,
            sigs,
            meta: FuncMeta { va: root.va, insns: dec.insns.len(), ..FuncMeta::default() },
        };
        for &l in &dec.leaders {
            if dec.insns.contains_key(&l) {
                let blk = tx.b.create_block();
                tx.blocks.insert(l, blk);
            }
        }
        tx.meta.blocks = tx.blocks.len();
        tx.reload();
        for v in [cf, zf, sf, of, pf] {
            let z = tx.b.ins().iconst(types::I8, 0);
            tx.b.def_var(v, z);
        }
        let first = tx.block_of(root.va)?;
        tx.b.ins().jump(first, &[]);
        let leaders: Vec<u64> = tx.blocks.keys().copied().collect();
        for (k, &start) in leaders.iter().enumerate() {
            let blk = tx.blocks[&start];
            tx.b.switch_to_block(blk);
            let mut ip = start;
            loop {
                let i = dec.insns[&ip];
                if tx.insn(&i, &dec)? == Flow::Done {
                    break;
                }
                ip = i.next_ip();
                if tx.blocks.contains_key(&ip) {
                    tx.b.ins().jump(tx.blocks[&ip], &[]);
                    break;
                }
                if !dec.insns.contains_key(&ip) {
                    return Err(why!("flow:falls-off {ip:#x}"));
                }
            }
            let _ = k;
        }
        if let Some(ex) = tx.exit {
            tx.b.switch_to_block(ex);
            tx.b.ins().return_(&[]);
        }
        tx.b.seal_all_blocks();
        let meta = tx.meta.clone();
        let cfg = tx.cfg;
        tx.b.finalize(cfg);
        let id = m.declare_function(&symbol(root.va), Linkage::Export, &lifted_sig()).map_err(|e| why!("module:{e}"))?;
        Ok((id, func, meta))
    }

    /// A function that traps with `NotLifted`: stands in for a callee that was not lifted.
    pub fn trap_stub<M: Module>(&mut self, m: &mut M, va: u64) -> R<(FuncId, ir::Function)> {
        let id = m.declare_function(&symbol(va), Linkage::Export, &lifted_sig()).map_err(|e| why!("module:{e}"))?;
        let mut func = ir::Function::with_name_signature(ir::UserFuncName::user(0, id.as_u32()), lifted_sig());
        let mut fbc = FunctionBuilderContext::new();
        let mut b = FunctionBuilder::new(&mut func, &mut fbc);
        let sig = b.import_signature(trap_sig());
        let e = b.create_block();
        b.append_block_params_for_function_params(e);
        b.switch_to_block(e);
        let cpu = b.block_params(e)[0];
        let rt = b.ins().load(types::I64, MemFlagsData::new(), cpu, Cpu::OFF_RT);
        let ctx = b.ins().load(types::I64, MemFlagsData::new(), rt, Rt::OFF_CTX);
        let f = b.ins().load(types::I64, MemFlagsData::new(), rt, Rt::OFF_TRAP);
        let k = b.ins().iconst(types::I64, TrapKind::NotLifted as i64);
        let a = b.ins().iconst(types::I64, va as i64);
        b.ins().call_indirect(sig, f, &[ctx, cpu, k, a]);
        b.ins().return_(&[]);
        b.seal_all_blocks();
        b.finalize(self.cfg);
        Ok((id, func))
    }
}
