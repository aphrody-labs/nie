//! Measured facts about the binary: the machine-readable inventory behind `docs/recomp.md`.

use std::collections::{BTreeMap, HashMap};

use iced_x86::{Decoder, DecoderOptions, FlowControl, Instruction, Mnemonic, OpKind, Register};
use serde::Serialize;

use crate::image::{
    DIR_DEBUG, DIR_DELAY_IMPORT, DIR_EXPORT, DIR_IAT, DIR_IMPORT, DIR_LOAD_CONFIG, DIR_TLS, Image, SCN_EXEC, SCN_WRITE,
};
use crate::pdata::{self, Entry};

#[derive(Serialize, Default)]
pub struct SectionInfo {
    pub name: String,
    pub rva: u32,
    pub vsize: u32,
    pub raw_size: u32,
    pub flags: u32,
    pub entropy: f64,
    pub writable_and_executable: bool,
}

#[derive(Serialize, Default)]
pub struct ImportDll {
    pub dll: String,
    pub functions: Vec<String>,
}

#[derive(Serialize, Default)]
pub struct PdataStats {
    pub entries: usize,
    pub roots: usize,
    pub chained_fragments: usize,
    pub text_bytes: u64,
    pub root_bytes: u64,
    pub gap_bytes_total: u64,
    pub gap_bytes_not_padding: u64,
    pub gap_code_fragments: u64,
    pub size_min: u32,
    pub size_p50: u32,
    pub size_p90: u32,
    pub size_p99: u32,
    pub size_max: u32,
    pub unwind_with_handler: usize,
    pub handlers: Vec<HandlerStat>,
}

#[derive(Serialize)]
pub struct HandlerStat {
    pub rva: u32,
    pub functions: usize,
    pub head: Vec<String>,
}

#[derive(Serialize, Default)]
pub struct CodeStats {
    pub instructions: u64,
    pub decode_errors: u64,
    pub functions_scanned: usize,
    pub call_direct: u64,
    pub call_iat: u64,
    pub call_indirect_other: u64,
    pub jmp_direct: u64,
    pub jmp_iat_thunk: u64,
    pub jmp_indirect_table: u64,
    pub jmp_indirect_other: u64,
    pub ret: u64,
    pub functions_with_indirect_call: usize,
    pub functions_with_jump_table: usize,
    pub segment_gs: u64,
    pub segment_fs: u64,
    pub lock_prefix: u64,
    pub rep_string: u64,
    pub x87: u64,
    pub system: BTreeMap<String, u64>,
    pub isa_classes: BTreeMap<String, u64>,
    pub top_mnemonics: Vec<(String, u64)>,
}

#[derive(Serialize, Default)]
pub struct TlsInfo {
    pub present: bool,
    pub raw_data_bytes: u64,
    pub zero_fill: u32,
    pub index_va: u64,
    pub callbacks: Vec<u64>,
}

#[derive(Serialize, Default)]
pub struct LoadConfig {
    pub present: bool,
    pub security_cookie_va: u64,
    pub se_handler_table_va: u64,
    pub se_handler_count: u64,
    pub guard_flags: u32,
    pub cfg_function_count: u64,
}

#[derive(Serialize, Default)]
pub struct Findings {
    pub rtti_type_descriptors_class: usize,
    pub rtti_type_descriptors_struct: usize,
    pub lua_52_banner: bool,
    pub lua_api_error_strings: usize,
    pub steamstub_bind_section: bool,
    pub entry_in_exec_section: bool,
    pub entry_section: String,
    pub packed_suspects: Vec<String>,
    pub exports: Vec<String>,
    pub pdb: String,
    pub dynamic_crt: bool,
    pub static_crt_marker: bool,
    pub x87_and_vm_hints: Vec<String>,
}

#[derive(Serialize, Default)]
pub struct Win32Coverage {
    pub imported: usize,
    pub implemented: usize,
    pub percent: f64,
    pub per_dll: BTreeMap<String, (usize, usize)>,
}

#[derive(Serialize)]
pub struct Inventory {
    pub file_len: usize,
    pub image_base: u64,
    pub size_of_image: u32,
    pub entry_rva: u32,
    pub linker: String,
    pub timestamp: u32,
    pub dll_characteristics: u16,
    pub sections: Vec<SectionInfo>,
    pub imports: Vec<ImportDll>,
    pub import_function_count: usize,
    pub delay_import_dlls: usize,
    pub pdata: PdataStats,
    pub code: CodeStats,
    pub tls: TlsInfo,
    pub load_config: LoadConfig,
    pub findings: Findings,
    pub win32: Win32Coverage,
}

pub fn entropy(b: &[u8]) -> f64 {
    if b.is_empty() {
        return 0.0;
    }
    let mut c = [0u64; 256];
    for &x in b {
        c[usize::from(x)] += 1;
    }
    let n = b.len() as f64;
    c.iter().filter(|&&k| k > 0).map(|&k| k as f64 / n).map(|p| -p * p.log2()).sum()
}

/// `(dll, function)` pairs of the import directory, in table order, with the IAT slot RVA.
pub fn import_table(img: &Image) -> Vec<(u32, String, String)> {
    let mut out = Vec::new();
    let d = img.dir(DIR_IMPORT);
    let mut p = d.rva;
    while let (Some(oft), Some(name), Some(ft)) = (img.u32(p), img.u32(p + 12), img.u32(p + 16)) {
        if name == 0 {
            break;
        }
        let dll = img.cstr(name).unwrap_or_default();
        let thunks = if oft != 0 { oft } else { ft };
        let mut i = 0u32;
        while let Some(t) = img.u64(thunks + 8 * i) {
            if t == 0 {
                break;
            }
            let func = if t >> 63 != 0 {
                format!("#{}", t & 0xffff)
            } else {
                img.cstr(t as u32 + 2).unwrap_or_default()
            };
            out.push((ft + 8 * i, dll.clone(), func));
            i += 1;
        }
        p += 20;
    }
    out
}

fn count_pattern(hay: &[u8], pat: &[u8]) -> usize {
    let mut n = 0;
    let mut i = 0;
    while let Some(p) = hay[i..].windows(pat.len()).position(|w| w == pat) {
        n += 1;
        i += p + pat.len();
    }
    n
}

fn disasm_head(img: &Image, rva: u32, n: usize) -> Vec<String> {
    let Some(bytes) = img.slice(rva, 64.min(img.flat.len() - rva as usize)) else {
        return Vec::new();
    };
    let mut d = Decoder::with_ip(64, bytes, img.base + u64::from(rva), DecoderOptions::NONE);
    let mut f = iced_x86::IntelFormatter::new();
    let mut out = Vec::new();
    for _ in 0..n {
        if !d.can_decode() {
            break;
        }
        let i = d.decode();
        let mut s = String::new();
        iced_x86::Formatter::format(&mut f, &i, &mut s);
        out.push(s);
    }
    out
}

/// True when a register-indirect `jmp` is the tail of an MSVC jump table sequence:
/// a 32-bit scaled-by-4 table load and an `add` in the preceding instructions.
pub fn is_table_jump(window: &[Instruction]) -> bool {
    let load = window.iter().rev().take(6).any(|i| {
        i.mnemonic() == Mnemonic::Mov
            && i.op_kind(1) == OpKind::Memory
            && i.memory_index_scale() == 4
            && i.memory_index() != Register::None
    });
    let add = window.iter().rev().take(4).any(|i| i.mnemonic() == Mnemonic::Add && i.op_kind(0) == OpKind::Register);
    load && add
}

fn scan_code(img: &Image, roots: &[Entry], iat: (u32, u32)) -> CodeStats {
    let mut st = CodeStats::default();
    let mut mn: HashMap<Mnemonic, u64> = HashMap::new();
    let mut isa: HashMap<iced_x86::CpuidFeature, u64> = HashMap::new();
    let mut sys: HashMap<Mnemonic, u64> = HashMap::new();
    for e in roots {
        let Some(code) = img.slice(e.begin, (e.end - e.begin) as usize) else { continue };
        st.functions_scanned += 1;
        let mut d = Decoder::with_ip(64, code, img.base + u64::from(e.begin), DecoderOptions::NONE);
        let mut win: Vec<Instruction> = Vec::with_capacity(8);
        let (mut has_ind, mut has_tab) = (false, false);
        while d.can_decode() {
            let i = d.decode();
            if i.is_invalid() {
                st.decode_errors += 1;
                break;
            }
            st.instructions += 1;
            *mn.entry(i.mnemonic()).or_default() += 1;
            if let Some(&f) = i.cpuid_features().first() {
                *isa.entry(f).or_default() += 1;
            }
            match i.segment_prefix() {
                Register::GS => st.segment_gs += 1,
                Register::FS => st.segment_fs += 1,
                _ => {}
            }
            if i.has_lock_prefix() {
                st.lock_prefix += 1;
            }
            if i.has_rep_prefix() || i.has_repne_prefix() {
                st.rep_string += 1;
            }
            let iat_slot = |i: &Instruction| {
                i.is_ip_rel_memory_operand() && {
                    let t = i.ip_rel_memory_address().wrapping_sub(img.base);
                    t >= u64::from(iat.0) && t < u64::from(iat.0) + u64::from(iat.1)
                }
            };
            match i.mnemonic() {
                Mnemonic::Syscall | Mnemonic::Sysenter | Mnemonic::Int | Mnemonic::Cpuid | Mnemonic::Rdtsc
                | Mnemonic::Rdtscp | Mnemonic::Xgetbv | Mnemonic::Ud2 | Mnemonic::Hlt | Mnemonic::Rdrand
                | Mnemonic::Pause | Mnemonic::Mfence | Mnemonic::Lfence | Mnemonic::Sfence => {
                    *sys.entry(i.mnemonic()).or_default() += 1;
                }
                _ => {}
            }
            match i.flow_control() {
                FlowControl::Call => st.call_direct += 1,
                FlowControl::IndirectCall => {
                    if iat_slot(&i) {
                        st.call_iat += 1;
                    } else {
                        st.call_indirect_other += 1;
                        has_ind = true;
                    }
                }
                FlowControl::UnconditionalBranch => st.jmp_direct += 1,
                FlowControl::IndirectBranch => {
                    if iat_slot(&i) {
                        st.jmp_iat_thunk += 1;
                    } else if i.op0_kind() == OpKind::Register && is_table_jump(&win) {
                        st.jmp_indirect_table += 1;
                        has_tab = true;
                    } else {
                        st.jmp_indirect_other += 1;
                    }
                }
                FlowControl::Return => st.ret += 1,
                _ => {}
            }
            if win.len() == 8 {
                win.remove(0);
            }
            win.push(i);
        }
        st.functions_with_indirect_call += usize::from(has_ind);
        st.functions_with_jump_table += usize::from(has_tab);
    }
    let mut top: Vec<_> = mn.into_iter().collect();
    top.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
    st.top_mnemonics = top.into_iter().take(40).map(|(m, n)| (format!("{m:?}"), n)).collect();
    for (f, n) in isa {
        let class = format!("{f:?}");
        if class == "FPU" {
            st.x87 += n;
        }
        *st.isa_classes.entry(class).or_default() += n;
    }
    for (m, n) in sys {
        st.system.insert(format!("{m:?}"), n);
    }
    st
}

fn pdata_stats(img: &Image, ents: &[Entry]) -> (PdataStats, Vec<Entry>) {
    let roots: Vec<Entry> = ents.iter().copied().filter(Entry::is_root).collect();
    let mut sizes: Vec<u32> = roots.iter().map(|e| e.end - e.begin).collect();
    sizes.sort_unstable();
    let pct = |p: usize| sizes.get((sizes.len() * p / 100).min(sizes.len().saturating_sub(1))).copied().unwrap_or(0);
    let text = img.section(".text");
    let (tb, te) = text.map_or((0, 0), |t| (t.rva, t.rva + t.vsize));
    let mut covered: Vec<(u32, u32)> = ents.iter().map(|e| (e.begin, e.end)).collect();
    covered.sort_unstable();
    let (mut gap_total, mut gap_code, mut frag) = (0u64, 0u64, 0u64);
    let mut cur = tb;
    let mut gap = |from: u32, to: u32| {
        if to > from {
            gap_total += u64::from(to - from);
            let b = img.slice(from, (to - from) as usize).unwrap_or(&[]);
            let n = b.iter().filter(|&&x| x != 0xcc && x != 0x90 && x != 0).count() as u64;
            gap_code += n;
            frag += u64::from(n > 0);
        }
    };
    for &(b, e) in &covered {
        if b >= te {
            break;
        }
        gap(cur, b.max(cur).min(te));
        cur = cur.max(e);
    }
    gap(cur, te);
    let mut hs: HashMap<u32, usize> = HashMap::new();
    for e in &roots {
        if let Some(h) = e.handler {
            *hs.entry(h).or_default() += 1;
        }
    }
    let with_handler = hs.values().sum();
    let mut hv: Vec<_> = hs.into_iter().collect();
    hv.sort_by_key(|&(r, n)| (std::cmp::Reverse(n), r));
    let handlers = hv
        .into_iter()
        .take(8)
        .map(|(rva, functions)| HandlerStat { rva, functions, head: disasm_head(img, rva, 5) })
        .collect();
    let st = PdataStats {
        entries: ents.len(),
        roots: roots.len(),
        chained_fragments: ents.len() - roots.len(),
        text_bytes: u64::from(te - tb),
        root_bytes: sizes.iter().map(|&s| u64::from(s)).sum(),
        gap_bytes_total: gap_total,
        gap_bytes_not_padding: gap_code,
        gap_code_fragments: frag,
        size_min: sizes.first().copied().unwrap_or(0),
        size_p50: pct(50),
        size_p90: pct(90),
        size_p99: pct(99),
        size_max: sizes.last().copied().unwrap_or(0),
        unwind_with_handler: with_handler,
        handlers,
    };
    (st, roots)
}

fn tls_info(img: &Image) -> TlsInfo {
    let d = img.dir(DIR_TLS);
    let (Some(start), Some(end), Some(idx), Some(cbs), Some(zf)) =
        (img.u64(d.rva), img.u64(d.rva + 8), img.u64(d.rva + 16), img.u64(d.rva + 24), img.u32(d.rva + 32))
    else {
        return TlsInfo::default();
    };
    let mut callbacks = Vec::new();
    if let Some(mut p) = img.va_to_rva(cbs) {
        while let Some(cb) = img.u64(p).filter(|&c| c != 0) {
            callbacks.push(cb);
            p += 8;
        }
    }
    TlsInfo { present: true, raw_data_bytes: end - start, zero_fill: zf, index_va: idx, callbacks }
}

fn load_config(img: &Image) -> LoadConfig {
    let d = img.dir(DIR_LOAD_CONFIG);
    let Some(size) = img.u32(d.rva) else { return LoadConfig::default() };
    let r = |o: u32| if size > o + 8 { img.u64(d.rva + o).unwrap_or(0) } else { 0 };
    let flags = if size > 148 { img.u32(d.rva + 144).unwrap_or(0) } else { 0 };
    LoadConfig {
        present: true,
        security_cookie_va: r(88),
        se_handler_table_va: r(96),
        se_handler_count: r(104),
        guard_flags: flags,
        cfg_function_count: r(136),
    }
}

fn findings(img: &Image, imports: &[(u32, String, String)]) -> Findings {
    let mut f = Findings {
        rtti_type_descriptors_class: count_pattern(&img.flat, b".?AV"),
        rtti_type_descriptors_struct: count_pattern(&img.flat, b".?AU"),
        lua_52_banner: count_pattern(&img.flat, b"Lua 5.2") > 0,
        lua_api_error_strings: count_pattern(&img.flat, b"attempt to call a %s value")
            + count_pattern(&img.flat, b"bad argument #%d to '%s'"),
        steamstub_bind_section: img.section(".bind").is_some(),
        ..Findings::default()
    };
    if let Some(s) = img.section_of(img.entry_rva) {
        f.entry_section = s.name.clone();
        f.entry_in_exec_section = s.chars & SCN_EXEC != 0;
    }
    for s in &img.sections {
        let raw = img.slice(s.rva, s.raw_size.min(s.vsize) as usize).unwrap_or(&[]);
        if s.chars & SCN_EXEC != 0 && s.chars & SCN_WRITE != 0 {
            f.packed_suspects.push(format!("{}: writable and executable", s.name));
        }
        if s.chars & SCN_EXEC != 0 && entropy(raw) > 7.2 {
            f.packed_suspects.push(format!("{}: executable with entropy {:.2}", s.name, entropy(raw)));
        }
    }
    let exp = img.dir(DIR_EXPORT);
    if let (Some(n), Some(names)) = (img.u32(exp.rva + 24), img.u32(exp.rva + 32)) {
        for i in 0..n.min(64) {
            if let Some(s) = img.u32(names + 4 * i).and_then(|r| img.cstr(r)) {
                f.exports.push(s);
            }
        }
    }
    let dbg = img.dir(DIR_DEBUG);
    for k in 0..dbg.size / 28 {
        let e = dbg.rva + 28 * k;
        if img.u32(e + 12) == Some(2)
            && let Some(raw) = img.u32(e + 20).and_then(|r| img.cstr(r + 24))
        {
            f.pdb = raw;
        }
    }
    let dlls: Vec<String> = imports.iter().map(|(_, d, _)| d.to_ascii_lowercase()).collect();
    f.dynamic_crt = dlls.iter().any(|d| d.starts_with("vcruntime") || d.starts_with("api-ms-win-crt") || d.starts_with("ucrtbase"));
    f.static_crt_marker = count_pattern(&img.flat, b"__scrt_") > 0 || count_pattern(&img.flat, b"R6030") > 0;
    for n in ["VirtualProtect", "VirtualAlloc", "WriteProcessMemory", "CreateRemoteThread", "IsDebuggerPresent", "NtQueryInformationProcess"] {
        if imports.iter().any(|(_, _, f)| f == n) {
            f.x87_and_vm_hints.push(format!("imports {n}"));
        }
    }
    f
}

fn win32_coverage(imports: &[(u32, String, String)]) -> Win32Coverage {
    let mut w = Win32Coverage { imported: imports.len(), ..Win32Coverage::default() };
    for (_, dll, f) in imports {
        let hit = nie_win32::lookup(dll, f).is_some();
        let e = w.per_dll.entry(dll.to_ascii_lowercase()).or_default();
        e.0 += 1;
        e.1 += usize::from(hit);
        w.implemented += usize::from(hit);
    }
    w.percent = if w.imported == 0 { 0.0 } else { (w.implemented as f64 * 10000.0 / w.imported as f64).round() / 100.0 };
    w
}

pub fn build(img: &Image) -> Inventory {
    let imports = import_table(img);
    let mut by_dll: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (_, d, f) in &imports {
        by_dll.entry(d.clone()).or_default().push(f.clone());
    }
    let ents = pdata::entries(img);
    let (pstats, roots) = pdata_stats(img, &ents);
    let iat = img.dir(DIR_IAT);
    let delay = img.dir(DIR_DELAY_IMPORT);
    Inventory {
        file_len: img.file_len,
        image_base: img.base,
        size_of_image: img.size_of_image,
        entry_rva: img.entry_rva,
        linker: format!("{}.{}", img.linker.0, img.linker.1),
        timestamp: img.timestamp,
        dll_characteristics: img.dll_chars,
        sections: img
            .sections
            .iter()
            .map(|s| SectionInfo {
                name: s.name.clone(),
                rva: s.rva,
                vsize: s.vsize,
                raw_size: s.raw_size,
                flags: s.chars,
                entropy: (entropy(img.slice(s.rva, s.raw_size.min(s.vsize) as usize).unwrap_or(&[])) * 100.0).round() / 100.0,
                writable_and_executable: s.chars & SCN_EXEC != 0 && s.chars & SCN_WRITE != 0,
            })
            .collect(),
        import_function_count: imports.len(),
        imports: by_dll.into_iter().map(|(dll, functions)| ImportDll { dll, functions }).collect(),
        delay_import_dlls: usize::from(delay.rva != 0),
        code: scan_code(img, &roots, (iat.rva, iat.size)),
        pdata: pstats,
        tls: tls_info(img),
        load_config: load_config(img),
        findings: findings(img, &imports),
        win32: win32_coverage(&imports),
    }
}
