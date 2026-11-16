// SPDX-License-Identifier: Apache-2.0
//! `yolo-re-core` (`bun_re`) — thin façade re-exporting [`aphrody_re_core`].
//!
//! All generic RE primitives (PE/ELF/Mach-O triage, strings, x86-64 disassembly,
//! headers, minidump, `.pdata`, pipeline, Ghidra decompile, Magika, LevelDB, YARA,
//! DBXV2 formats, CRI CPK/CRILAYLA/EAC, live process/AOB, catalog, unified overview)
//! live canonically in [`aphrody_re_core`] (`C:\aphrody\crates\re`).

#![forbid(unsafe_code)]
#![allow(non_ascii_idents)]

#[cfg(feature = "decompile")]
pub use aphrody_re_core::decompile;
#[cfg(feature = "leveldb")]
pub use aphrody_re_core::leveldb;
#[cfg(feature = "magika")]
pub use aphrody_re_core::magika;
#[cfg(feature = "pipeline")]
pub use aphrody_re_core::pipeline;
#[cfg(feature = "wasm")]
pub use aphrody_re_core::wasm;
#[cfg(feature = "yara")]
pub use aphrody_re_core::yara;

pub use aphrody_re_core::{
    AesCtrResult, AobScanResult, ArchitectureSlice, BinaryString, DISASM_LIMIT, DisasmInsn,
    EntropyMap, ExtractedStrings, Format, MultiHash, PeAnalysis, PeAnalysisImport,
    PeAnalysisSection, PeAnalysisSymbol, PeHeader, PeSection, ReError, STRINGS_MIN_LEN,
    STRINGS_SAMPLE_LIMIT, Section, SectionDumpResult, TriageReport, XorResult, adobe, aes_ctr_file,
    analyze_pe_file, android, asar, bun_engine_binary, cpu, disasm, electron, entropy_map_file,
    extract_strings, extract_strings_file, golang, google, hash_file, headers, list_cpk_file,
    minidump, parse_pe_header_file, pdata, pe, pe_inspect, propagate, section_dump_file,
    shannon_entropy, steam, triage, triage_bounded, virtualization, xor_file,
};

pub mod game;
pub mod nie;
