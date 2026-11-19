//! `nie-pe` — thin façade re-exporting [`nie_re::pe_forge`].
//!
//! All PE64 partitioning, byte-exact rebuild, MSVC-dialect x86-64 assembly,
//! COFF inspection, and `.pdata` scanning live canonically in `nie-re-core` (`crates/re`).

pub use nie_re::pe_forge::{
    CoffHeader, Cover, DataDirectory, OptionalHeader64, PeError, PeImage, Result, SectionHeader,
    Unit, UnitKind, asm, checksum, coff, diff, image, pdata, sha256_hex, units,
};

#[cfg(feature = "crlib")]
#[allow(unsafe_code)]
mod abi_generated;
