//! Guest register file and the runtime vtable recompiled code calls through.

use core::ffi::c_void;

/// Guest general purpose register numbers, the hardware encoding.
pub mod reg {
    pub const RAX: usize = 0;
    pub const RCX: usize = 1;
    pub const RDX: usize = 2;
    pub const RBX: usize = 3;
    pub const RSP: usize = 4;
    pub const RBP: usize = 5;
    pub const RSI: usize = 6;
    pub const RDI: usize = 7;
    pub const R8: usize = 8;
    pub const R9: usize = 9;
}

/// Why a guest run stopped before returning normally.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum TrapKind {
    /// Execution reached an address that is neither a lifted function nor an import stub.
    NotLifted = 1,
    /// Execution reached an import that has no implementation.
    UnimplementedImport = 2,
    /// The guest executed an instruction the lifter refuses (`ud2`, `int3`, unsupported form).
    Instruction = 3,
    /// An import implementation reported a fatal guest error.
    Fatal = 4,
}

/// Registers of one guest thread. `#[repr(C)]`: the lifter emits loads and
/// stores at the `OFF_*` offsets.
#[repr(C)]
pub struct Cpu {
    pub gpr: [u64; 16],
    pub rt: *const Rt,
    pub trap: u64,
    pub xmm: [u64; 32],
}

impl Cpu {
    pub const OFF_GPR: i32 = 0;
    pub const OFF_RT: i32 = 128;
    pub const OFF_TRAP: i32 = 136;
    pub const OFF_XMM: i32 = 144;

    pub const fn new() -> Self {
        Self {
            gpr: [0; 16],
            rt: core::ptr::null(),
            trap: 0,
            xmm: [0; 32],
        }
    }

    /// Win64 integer argument `n` (0..4) read from the register file.
    pub fn arg(&self, n: usize) -> u64 {
        match n {
            0 => self.gpr[reg::RCX],
            1 => self.gpr[reg::RDX],
            2 => self.gpr[reg::R8],
            3 => self.gpr[reg::R9],
            _ => 0,
        }
    }

    /// Set the integer return value.
    pub fn ret(&mut self, v: u64) {
        self.gpr[reg::RAX] = v;
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

/// Entry of a recompiled function.
pub type LiftedFn = unsafe extern "C" fn(*mut Cpu);

/// Runtime services for recompiled code. Offsets are fixed: the lifter reads
/// `ctx`, `call` and `trap` by their `RT_*` position.
#[repr(C)]
pub struct Rt {
    pub ctx: *mut c_void,
    pub call: unsafe extern "C" fn(*mut c_void, *mut Cpu, u64),
    pub trap: unsafe extern "C" fn(*mut c_void, *mut Cpu, u64, u64),
    /// 128-by-64 bit division: `(ctx, hi, lo, divisor, signed, out[quotient, remainder])`.
    pub div: unsafe extern "C" fn(*mut c_void, u64, u64, u64, u64, *mut u64),
}

impl Rt {
    pub const OFF_CTX: i32 = 0;
    pub const OFF_CALL: i32 = 8;
    pub const OFF_TRAP: i32 = 16;
    pub const OFF_DIV: i32 = 24;
}
