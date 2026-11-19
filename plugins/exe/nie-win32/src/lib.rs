//! Runtime of the statically recompiled `nie.exe`.
//!
//! Recompiled functions take one argument, a pointer to [`Cpu`], and keep the
//! guest registers there between calls. Guest memory is the PE image mapped at
//! its preferred base, so a guest pointer is a host pointer. Anything the guest
//! calls outside the recompiled code (an import, a function that was not
//! lifted) goes through [`Rt::call`], which [`Machine`] implements.

pub mod cpu;
pub mod imports;
#[cfg(unix)]
pub mod kernel32;
#[cfg(unix)]
pub mod machine;
#[cfg(unix)]
pub mod mem;

pub use cpu::{Cpu, Rt, TrapKind, reg};
pub use imports::{ImportFn, ImportImpl, implemented, lookup};
#[cfg(unix)]
pub use machine::{Machine, STUB_BASE};
#[cfg(unix)]
pub use mem::Guest;
