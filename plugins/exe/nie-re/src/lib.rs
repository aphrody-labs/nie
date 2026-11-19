// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `nie-re`: reverse-engineering core of the nie.exe stack. Binary format triage (`goblin`),
//! x86-64 disassembly (`iced-x86`), `.pdata` and minidump readers, the byte-exact PE64 forge
//! (`pe_forge`), Ghidra P-Code lifting and the staged recompilation pipeline.

#![allow(non_ascii_idents)]

pub mod android;
pub mod cpu;
#[cfg(feature = "decompile")]
pub mod decompile;
pub mod disasm;
pub mod dump;
pub mod ghidra;
pub mod minidump;
pub mod pdata;
pub mod pe_forge;
#[cfg(feature = "pipeline")]
pub mod pipeline;
pub mod propagate;
pub mod triage;
pub mod virtualization;
#[cfg(feature = "wasm")]
pub mod wasm_container;

#[cfg(feature = "wasm")]
pub use wasm_container as wasm;

pub use triage::{
    AesCtrResult, AobScanResult, ArchitectureSlice, BinaryString, DISASM_LIMIT, DisasmInsn,
    EntropyMap, ExtractedStrings, Format, MultiHash, PeAnalysis, PeAnalysisImport,
    PeAnalysisSection, PeAnalysisSymbol, PeHeader, PeSection, ReError, STRINGS_MIN_LEN,
    STRINGS_SAMPLE_LIMIT, Section, SectionDumpResult, TriageReport, XorResult, aes_ctr_file,
    analyze_pe_file, disasm, entropy_map_file, extract_strings, extract_strings_file, hash_file,
    list_cpk_file, parse_pe_header_file, section_dump_file, shannon_entropy, triage,
    triage_bounded, xor_file,
};

#[cfg(feature = "crlib")]
#[allow(unsafe_code)]
mod abi_generated;
