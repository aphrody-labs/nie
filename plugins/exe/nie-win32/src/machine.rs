//! Call dispatch between recompiled functions, import stubs and the host.

use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::c_void;

use crate::cpu::{Cpu, LiftedFn, Rt, TrapKind, reg};
use crate::imports::{self, ImportFn};
use crate::mem::{Guest, Stack};

/// Guest addresses handed out for import stubs. Never mapped, never dereferenced.
pub const STUB_BASE: u64 = 0x7ffd_0000_0000;
const STUB_STRIDE: u64 = 16;
/// Return address of the outermost guest call.
pub const SENTINEL: u64 = 0x7ffc_ffff_fff0;
const STACK_BYTES: usize = 1 << 20;

struct Stub {
    dll: String,
    name: String,
    f: Option<ImportFn>,
}

/// Why a run ended abnormally.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trap {
    pub kind: TrapKind,
    pub va: u64,
    pub detail: String,
}

/// One guest process: image, call tables and a main stack.
pub struct Machine {
    pub guest: Guest,
    lifted: HashMap<u64, LiftedFn>,
    stubs: Vec<Stub>,
    rt: Rt,
    trap: Cell<Option<(TrapKind, u64)>>,
    stack: Stack,
}

impl Machine {
    pub fn new(guest: Guest) -> std::io::Result<Box<Self>> {
        let mut m = Box::new(Self {
            guest,
            lifted: HashMap::new(),
            stubs: Vec::new(),
            rt: Rt {
                ctx: core::ptr::null_mut(),
                call: rt_call,
                trap: rt_trap,
                div: rt_div,
            },
            trap: Cell::new(None),
            stack: Stack::new(STACK_BYTES)?,
        });
        m.rt.ctx = (&*m as *const Machine).cast_mut().cast::<c_void>();
        Ok(m)
    }

    /// Register a recompiled function at guest address `va`.
    pub fn register(&mut self, va: u64, f: LiftedFn) {
        self.lifted.insert(va, f);
    }

    pub fn lifted_count(&self) -> usize {
        self.lifted.len()
    }

    /// Allocate a stub for `dll!name`, write its address into the guest IAT slot, return it.
    pub fn bind_import(&mut self, iat_va: u64, dll: &str, name: &str) -> u64 {
        let va = STUB_BASE + self.stubs.len() as u64 * STUB_STRIDE;
        let f = imports::lookup(dll, name).map(|i| i.f);
        self.stubs.push(Stub { dll: dll.to_owned(), name: name.to_owned(), f });
        self.guest.write_u64(iat_va, va);
        va
    }

    /// Imports bound without a Rust implementation.
    pub fn unimplemented_imports(&self) -> Vec<(&str, &str)> {
        self.stubs
            .iter()
            .filter(|s| s.f.is_none())
            .map(|s| (s.dll.as_str(), s.name.as_str()))
            .collect()
    }

    /// A fresh register file with an empty guest stack, shadow space included.
    pub fn new_cpu(&self) -> Cpu {
        let mut cpu = Cpu::new();
        cpu.rt = &self.rt;
        cpu.gpr[reg::RSP] = self.stack.top() - 0x100;
        cpu
    }

    /// Call the guest function at `va` as if from outside the image.
    pub fn run(&self, cpu: &mut Cpu, va: u64) -> Result<(), Trap> {
        cpu.trap = 0;
        self.trap.set(None);
        cpu.gpr[reg::RSP] -= 8;
        // SAFETY: rsp lies in the machine's own stack mapping.
        unsafe { (cpu.gpr[reg::RSP] as *mut u64).write_unaligned(SENTINEL) };
        // SAFETY: cpu.rt points at self.rt, ctx at self; no other &mut Machine exists during the run.
        unsafe { dispatch(self, cpu, va) };
        match self.trap.take() {
            None => Ok(()),
            Some((kind, at)) => Err(Trap { kind, va: at, detail: self.describe(kind, at) }),
        }
    }

    fn describe(&self, kind: TrapKind, va: u64) -> String {
        match kind {
            TrapKind::UnimplementedImport => self
                .stub_at(va)
                .map(|s| format!("{}!{}", s.dll, s.name))
                .unwrap_or_default(),
            TrapKind::NotLifted => format!("{va:#x}"),
            _ => String::new(),
        }
    }

    fn stub_at(&self, va: u64) -> Option<&Stub> {
        let idx = va.checked_sub(STUB_BASE)? / STUB_STRIDE;
        self.stubs.get(usize::try_from(idx).ok()?)
    }
}

/// # Safety
/// `cpu` must be a live register file whose guest stack holds the return address.
unsafe fn dispatch(m: &Machine, cpu: *mut Cpu, va: u64) {
    if let Some(f) = m.lifted.get(&va) {
        // SAFETY: registered functions follow the `fn(*mut Cpu)` contract.
        unsafe { f(cpu) };
        return;
    }
    // SAFETY: cpu is live per this function's contract.
    let cpu_ref = unsafe { &mut *cpu };
    match m.stub_at(va) {
        Some(Stub { f: Some(f), .. }) => {
            f(cpu_ref);
            cpu_ref.gpr[reg::RSP] += 8;
        }
        Some(_) => trap(m, cpu_ref, TrapKind::UnimplementedImport, va),
        None => trap(m, cpu_ref, TrapKind::NotLifted, va),
    }
}

fn trap(m: &Machine, cpu: &mut Cpu, kind: TrapKind, va: u64) {
    if m.trap.get().is_none() {
        m.trap.set(Some((kind, va)));
    }
    cpu.trap = kind as u64;
}

unsafe extern "C" fn rt_call(ctx: *mut c_void, cpu: *mut Cpu, va: u64) {
    // SAFETY: ctx is the Machine that owns this Rt; recompiled code passes its live Cpu.
    unsafe { dispatch(&*ctx.cast::<Machine>(), cpu, va) };
}

unsafe extern "C" fn rt_trap(ctx: *mut c_void, cpu: *mut Cpu, kind: u64, va: u64) {
    // SAFETY: as in rt_call.
    let (m, cpu) = unsafe { (&*ctx.cast::<Machine>(), &mut *cpu) };
    let kind = if kind == TrapKind::Fatal as u64 { TrapKind::Fatal } else { TrapKind::Instruction };
    trap(m, cpu, kind, va);
}

unsafe extern "C" fn rt_div(_ctx: *mut c_void, hi: u64, lo: u64, d: u64, signed: u64, out: *mut u64) {
    let (q, r) = if signed != 0 {
        let n = (i128::from(hi as i64) << 64) | i128::from(lo);
        let d = i128::from(d as i64);
        if d == 0 { (0, 0) } else { (n.wrapping_div(d) as u64, n.wrapping_rem(d) as u64) }
    } else {
        let n = (u128::from(hi) << 64) | u128::from(lo);
        let d = u128::from(d);
        match (n.checked_div(d), n.checked_rem(d)) {
            (Some(q), Some(r)) => (q as u64, r as u64),
            _ => (0, 0),
        }
    };
    // SAFETY: out points at two u64 slots owned by the caller.
    unsafe {
        out.write(q);
        out.add(1).write(r);
    }
}
