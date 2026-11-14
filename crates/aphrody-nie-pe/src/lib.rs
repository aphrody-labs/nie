//! `aphrody-nie-pe` — thin façade re-exporting [`aphrody_re_core::pe_forge`].
//!
//! All PE64 partitioning, byte-exact rebuild, MSVC-dialect x86-64 assembly,
//! COFF inspection, and `.pdata` scanning live canonically in `aphrody-re` (`crates/re`).

pub use aphrody_re_core::pe_forge::{
    CoffHeader, Cover, DataDirectory, OptionalHeader64, PeError, PeImage, Result, SectionHeader,
    Unit, UnitKind, asm, checksum, coff, diff, image, pdata, sha256_hex, units,
};
