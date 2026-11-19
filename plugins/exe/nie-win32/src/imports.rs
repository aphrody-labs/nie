//! The Win32 import surface: which `(dll, function)` pairs have a Rust implementation.

use crate::cpu::Cpu;

/// A Win32 function implemented in Rust. Reads Win64 arguments from the
/// register file and leaves the return value in `rax`.
pub type ImportFn = fn(&mut Cpu);

/// One implemented import.
pub struct ImportImpl {
    pub dll: &'static str,
    pub name: &'static str,
    pub f: ImportFn,
}

/// Every implemented import, sorted by DLL then name.
pub fn implemented() -> &'static [ImportImpl] {
    #[cfg(unix)]
    return crate::kernel32::TABLE;
    #[cfg(not(unix))]
    &[]
}

/// Case-insensitive DLL match, exact function name.
pub fn lookup(dll: &str, name: &str) -> Option<&'static ImportImpl> {
    implemented()
        .iter()
        .find(|i| i.dll.eq_ignore_ascii_case(dll) && i.name == name)
}
