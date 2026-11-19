// SPDX-License-Identifier: Apache-2.0
//! `nie-re-core` (`nie_re_core`) — thin façade re-exporting [`nie_re`].
//!
//! All generic RE primitives (PE/ELF/Mach-O triage, strings, x86-64 disassembly,
//! headers, minidump, `.pdata`, pipeline, Ghidra decompile, Magika, LevelDB, YARA,
//! DBXV2 formats, CRI CPK/CRILAYLA/EAC, live process/AOB, catalog, unified overview)
//! live canonically in [`nie_re`] (`plugins/exe/nie-re`).

#![forbid(unsafe_code)]
#![allow(non_ascii_idents)]

#[cfg(feature = "decompile")]
pub use nie_re::decompile;
#[cfg(feature = "leveldb")]
pub use nie_re::leveldb;
#[cfg(feature = "magika")]
pub use nie_re::magika;
#[cfg(feature = "pipeline")]
pub use nie_re::pipeline;
#[cfg(feature = "wasm")]
pub use nie_re::wasm;
#[cfg(feature = "yara")]
pub use nie_re::yara;

pub use nie_re::{
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

/// The nie.exe reverse-engineering plugin: indexing, RTTI, disassembly, `.pdata`, strings, the
/// knowledge database and live process memory.
pub struct ExePlugin;

impl nie_plugin::Plugin for ExePlugin {
    fn info(&self) -> nie_plugin::PluginInfo {
        nie_plugin::PluginInfo {
            id: "exe",
            summary: "Reverse engineering of nie.exe: index, RTTI, disassembly, .pdata, strings, live memory",
            package: "nie-re-core",
            commands: &[
                "index",
                "rtti",
                "disasm",
                "pdata",
                "rebuild",
                "recover",
                "propagate",
                "strings",
                "seed",
                "seed-ui",
                "queue",
                "coverage",
                "atlas",
                "mem",
                "computer-use",
            ],
        }
    }
}
