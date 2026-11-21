//! Extract string literals from the binary and anchor **function -> string**.
//!
//! ## Why
//!
//! The knowledge-base `str` table was empty: no binary string was addressed. A string is the
//! least ambiguous reverse anchor (a function referencing `"CHARA_PARAM_INFO"` loads characters),
//! and without `str` those anchors exist only in the disassembler.
//!
//! `func_str_ref` already held references **inherited from the Ghidra index**, folded by inclusion
//! onto the `.pdata` roots (`nie_re_core::nie::pdata::rebuild_from_pdata`). Those rows have no
//! provenance and are heavily redundant. This module writes others, **derived from the real
//! disassembly**, under the [`SOURCE`] label: they coexist without mixing, and the `source` column
//! allows querying ground truth only.
//!
//! ## Method
//!
//! 1. `goblin` parses the PE: section geometry and `ImageBase`.
//! 2. Byte-by-byte scan of data sections (`.rdata`, `.data`, `.rodata`):
//!    - **ASCII**: printable bytes ended by a `NUL`, length >= `min_len`;
//!    - **UTF-16LE**: text `u16` units (latin, kana, CJK, fullwidth) ended by `0x0000`, aligned,
//!      and **not overlapping any retained ASCII string** (otherwise any ASCII text re-reads as
//!      CJK).
//!
//!      The mandatory `NUL` terminator is the deciding filter: the compiler merges terminated
//!      literals, not fragments.
//! 3. Function bounds: `.pdata` (`nie_re_core::nie::pdata::parse_roots`), the ground truth, not the
//!    Ghidra index whose addresses are misaligned.
//! 4. `iced-x86` decoding of each function body; a reference is kept only if its target
//!    **exactly matches** the start of a retained string:
//!    - `lea r64, [rip+disp32]`: the form MSVC emits for a literal address;
//!    - `mov r64, imm64`: the absolute form (rare with `/O2`, but it exists).
//!
//!      Other RIP-relative accesses (`mov`/`cmp [rip+x]`) are **counted, not ingested**: they hit
//!      the IAT and variables, and a `mov rax, [rip+s]` landing on a string reads its first eight
//!      bytes without "referencing" it as an anchor.
//! 5. **Suffixes.** MSVC merges literals by their tail: `".../chara.cfg.bin"` and `"chara.cfg.bin"`
//!    share bytes, and the second has no address of its own; the `lea` targets the **inside** of the
//!    long string. Those targets are resolved into full strings (the targeted byte does start a
//!    `NUL`-terminated run) and counted separately (`suffix=`): nothing is guessed, the suffix
//!    string is read from the image.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result, bail};
use goblin::pe::PE;
use iced_x86::{Decoder, DecoderOptions, Instruction, Mnemonic, OpKind};
use nie_index::rusqlite::{Connection, params};
use sha2::{Digest, Sha256};

/// Provenance label written to `func_str_ref.source`.
///
/// Rows without `source` (`NULL`) are the Ghidra index legacy folded onto `.pdata`; these come from
/// the binary disassembly.
const SOURCE: &str = "rdata-xref";

/// `kind` of the function -> string edges written to `xref` (`from_addr` = function start,
/// `to_addr` = string address). It is the only table that keeps the string **address**;
/// `func_str_ref` keeps only the text.
const XREF_KIND: &str = "str";

/// Data sections scanned by default.
const DEFAULT_SECTIONS: &[&str] = &[".rdata", ".data", ".rodata"];

/// Guard: maximum decoded length of a function body.
const MAX_FUNC_LEN: u64 = 256 * 1024;

/// Maximum length of a retained string, in bytes. Beyond that it is no longer a literal but a
/// data block that happens to read as text.
const MAX_STR_BYTES: usize = 8192;

/// Encoding of a found string (`str.kind` column).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Enc {
    /// Printable ASCII bytes ended by `NUL`.
    Ascii,
    /// Little-endian `u16` units ended by `0x0000`.
    Utf16,
}

impl Enc {
    fn tag(self) -> &'static str {
        match self {
            Self::Ascii => "ascii",
            Self::Utf16 => "utf16",
        }
    }
}

/// A retained string, with its virtual address.
struct FoundStr {
    vaddr: u64,
    /// Length in bytes, terminator excluded.
    len: usize,
    /// Section index in `Stats::par_section`.
    sec: usize,
    enc: Enc,
    value: String,
}

/// Pass options.
#[derive(Debug, Clone)]
pub struct Options {
    /// Minimum retained length (characters, not bytes).
    pub min_len: usize,
    /// Sections scanned; empty = [`DEFAULT_SECTIONS`].
    pub sections: Vec<String>,
    /// Force the binary id instead of resolving it by sha256.
    pub binary_id: Option<i64>,
    /// Write neither `func_str_ref` nor `xref` (string extraction only).
    pub no_xrefs: bool,
    /// Compute everything, write nothing.
    pub dry_run: bool,
    /// Print `n` sample strings (eyeball check).
    pub sample: usize,
}

/// Pass counters.
#[derive(Debug, Default, Clone)]
pub struct Stats {
    /// Binary resolved in the `binary` table.
    pub binary_id: i64,
    /// Path recorded for this binary.
    pub binary_path: String,
    /// Par section : `(nom, ascii, utf16)`.
    pub par_section: Vec<(String, usize, usize)>,
    /// Retained ASCII strings.
    pub ascii: usize,
    /// Retained UTF-16LE strings.
    pub utf16: usize,
    /// Candidates dropped for being longer than [`MAX_STR_BYTES`].
    pub oversized: usize,
    /// Rows actually inserted into `str` (excluding duplicates).
    pub str_inserted: usize,
    /// `.pdata` roots of the binary.
    pub roots: usize,
    /// Roots matching a `function` row of the target binary.
    pub roots_mapped: usize,
    /// Decoded function bodies.
    pub funcs_scanned: usize,
    /// Decoded instructions.
    pub insns: u64,
    /// `lea r64, [rip+disp32]` met.
    pub lea_rip: u64,
    /// `lea` whose target is exactly the start of a retained string.
    pub lea_hits: u64,
    /// `lea` whose target falls inside a string (suffix merged by MSVC).
    pub lea_suffix: u64,
    /// Suffix strings materialized and added to `str`.
    pub suffixes: usize,
    /// Function bodies stopped on undecodable bytes (inline data).
    pub bodies_truncated: usize,
    /// Distinct functions carrying at least one string.
    pub funcs_with_str: usize,
    /// `mov r64, imm64` whose immediate is exactly the start of a string.
    pub imm_hits: u64,
    /// **Non-`lea`** RIP-relative accesses landing on a string: counted, **not ingested**.
    pub rip_other_hits: u64,
    /// Distinct (function, string) pairs.
    pub pairs: usize,
    /// `func_str_ref` rows of source [`SOURCE`] deleted before rewriting.
    pub str_refs_deleted: usize,
    /// Inserted `func_str_ref` rows.
    pub str_refs_inserted: usize,
    /// Inserted `xref` edges (`kind='str'`).
    pub xrefs_inserted: usize,
    /// Nothing was written.
    pub dry_run: bool,
}

/// A byte that can belong to a C literal.
fn ascii_is_text(b: u8) -> bool {
    matches!(b, 0x20..=0x7E | b'\t' | b'\r' | b'\n')
}

/// A UTF-16 unit that can belong to a wide literal.
///
/// Deliberately limited to the blocks the game really uses (latin, general punctuation, kana,
/// CJK, fullwidth): accepting the whole multilingual plane would make any byte pair pass as text.
fn wide_is_text(u: u16) -> bool {
    matches!(u,
        0x09 | 0x0A | 0x0D
        | 0x0020..=0x007E
        | 0x00A1..=0x00FF
        | 0x2010..=0x203A
        | 0x3000..=0x30FF
        | 0x4E00..=0x9FFF
        | 0xFF01..=0xFF5E
    )
}

/// Scan the ASCII literals of a section and mark their span in `covered`.
fn scan_ascii(
    data: &[u8],
    va: u64,
    sec: usize,
    min_len: usize,
    out: &mut Vec<FoundStr>,
    covered: &mut [bool],
    oversized: &mut usize,
) {
    let mut i = 0usize;
    while i < data.len() {
        if !ascii_is_text(data[i]) {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < data.len() && ascii_is_text(data[j]) {
            j += 1;
        }
        // Mandatory NUL terminator: it tells a literal from a fragment.
        if j < data.len() && data[j] == 0 && j - i >= min_len {
            if j - i > MAX_STR_BYTES {
                *oversized += 1;
            } else {
                // `ascii_is_text` garantit un contenu ASCII pur, donc UTF-8 valide.
                let value = String::from_utf8_lossy(&data[i..j]).into_owned();
                out.push(FoundStr {
                    vaddr: va + i as u64,
                    len: j - i,
                    sec,
                    enc: Enc::Ascii,
                    value,
                });
                covered[i..=j].fill(true);
            }
        }
        i = j + 1;
    }
}

/// Scan the UTF-16LE literals of a section, dropping anything that overlaps an already retained
/// ASCII string.
fn scan_utf16(
    data: &[u8],
    va: u64,
    sec: usize,
    min_len: usize,
    out: &mut Vec<FoundStr>,
    covered: &[bool],
    oversized: &mut usize,
) {
    let unit = |k: usize| -> u16 { u16::from_le_bytes([data[k], data[k + 1]]) };
    let mut i = 0usize;
    while i + 1 < data.len() {
        if !wide_is_text(unit(i)) {
            i += 2;
            continue;
        }
        let mut j = i;
        while j + 1 < data.len() && wide_is_text(unit(j)) {
            j += 2;
        }
        let units = (j - i) / 2;
        if j + 1 < data.len() && unit(j) == 0 && units >= min_len && !covered[i..j].contains(&true)
        {
            if j - i > MAX_STR_BYTES {
                *oversized += 1;
            } else {
                let codes: Vec<u16> = (i..j).step_by(2).map(unit).collect();
                if let Ok(value) = String::from_utf16(&codes) {
                    out.push(FoundStr {
                        vaddr: va + i as u64,
                        len: j - i,
                        sec,
                        enc: Enc::Utf16,
                        value,
                    });
                }
            }
        }
        i = j + 2;
    }
}

/// Index of retained strings: exact membership, and **suffix** resolution.
///
/// MSVC merges literals by their tail: a suffix string has no bytes of its own, only an address
/// inside the long string. A target strictly inside a retained string therefore starts, by
/// construction, a `NUL`-terminated run.
struct StrIndex {
    /// Start addresses, sorted.
    starts: Vec<u64>,
    /// `(exclusive end, index in `found`)`, element-aligned with `starts`.
    spans: Vec<(u64, usize)>,
    exact: HashSet<u64>,
}

impl StrIndex {
    /// `found` must be sorted by ascending address and deduplicated.
    fn new(found: &[FoundStr]) -> Self {
        Self {
            starts: found.iter().map(|f| f.vaddr).collect(),
            spans: found
                .iter()
                .enumerate()
                .map(|(i, f)| (f.vaddr + f.len as u64, i))
                .collect(),
            exact: found.iter().map(|f| f.vaddr).collect(),
        }
    }

    /// `(index of the containing string, byte offset)` if `t` falls strictly inside a retained
    /// string. The offset in a wide string stays even.
    fn suffix_of(&self, found: &[FoundStr], t: u64) -> Option<(usize, usize)> {
        let idx = self.starts.partition_point(|&s| s <= t);
        if idx == 0 {
            return None;
        }
        let (end, fi) = self.spans[idx - 1];
        let start = self.starts[idx - 1];
        if t <= start || t >= end {
            return None;
        }
        let off = (t - start) as usize;
        if found[fi].enc == Enc::Utf16 && !off.is_multiple_of(2) {
            return None;
        }
        Some((fi, off))
    }

    /// Does the target designate a string (exact start or suffix)?
    fn hits(&self, found: &[FoundStr], t: u64) -> bool {
        self.exact.contains(&t) || self.suffix_of(found, t).is_some()
    }
}

/// Build the suffix string located `off` bytes into `src`.
fn suffix_str(src: &FoundStr, vaddr: u64, off: usize) -> FoundStr {
    let value = match src.enc {
        // ASCII content: a byte offset is a character offset.
        Enc::Ascii => src.value[off..].to_string(),
        Enc::Utf16 => src.value.chars().skip(off / 2).collect(),
    };
    FoundStr {
        vaddr,
        len: src.len - off,
        sec: src.sec,
        enc: src.enc,
        value,
    }
}

/// Geometry of a loaded section.
struct SecView<'a> {
    name: String,
    va: u64,
    data: &'a [u8],
}

/// PE sections actually present in the file, by name.
fn sections<'a>(pe: &PE, bytes: &'a [u8], wanted: &[String]) -> Vec<SecView<'a>> {
    let mut out = Vec::new();
    for s in &pe.sections {
        let Ok(name) = s.name() else { continue };
        if !wanted.iter().any(|w| w == name) {
            continue;
        }
        let off = s.pointer_to_raw_data as usize;
        // `virtual_size` exceeds `size_of_raw_data` for BSS sections (`.data`): only the part present in
        // the file holds readable bytes.
        let len = s.virtual_size.min(s.size_of_raw_data) as usize;
        let Some(data) = bytes.get(off..off + len) else {
            continue;
        };
        out.push(SecView {
            name: name.to_string(),
            va: pe.image_base + u64::from(s.virtual_address),
            data,
        });
    }
    out
}

/// Resolve the target binary: the `#pdata` view first (it carries the functions with their real
/// bounds), otherwise the file view.
fn resolve_binary(
    conn: &Connection,
    exe: &Path,
    sha: &str,
    forced: Option<i64>,
) -> Result<(i64, String)> {
    if let Some(id) = forced {
        let path: String = conn
            .query_row("SELECT path FROM binary WHERE id = ?1", [id], |r| r.get(0))
            .with_context(|| format!("no binary with id {id} in the database"))?;
        return Ok((id, path));
    }
    let pdata_key = format!("{sha}-pdata");
    for key in [pdata_key.as_str(), sha] {
        let row = conn.query_row(
            "SELECT id, path FROM binary WHERE sha256 = ?1",
            [key],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
        );
        if let Ok(found) = row {
            return Ok(found);
        }
    }
    let mut stmt = conn.prepare("SELECT id, sha256, path FROM binary ORDER BY id")?;
    let known: Vec<String> = stmt
        .query_map([], |r| {
            Ok(format!(
                "  id={} sha256={} {}",
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?
            ))
        })?
        .collect::<std::result::Result<_, _>>()?;
    bail!(
        "no indexed binary matches {} (sha256 {sha}).\nKnown binaries:\n{}\n\
         Pass --binary-id <id> to force, or index this binary first.",
        exe.display(),
        known.join("\n")
    );
}

/// Decode function bodies and collect (function start, string address) pairs.
fn scan_text(
    text: &[u8],
    text_va: u64,
    roots: &[nie_re_core::nie::pdata::RootFn],
    found: &[FoundStr],
    known: &StrIndex,
    mapped: &HashSet<u64>,
    stats: &mut Stats,
) -> HashSet<(u64, u64)> {
    let text_end = text_va + text.len() as u64;
    let mut hits: HashSet<(u64, u64)> = HashSet::new();
    let mut insn = Instruction::default();

    for r in roots {
        if r.start < text_va || r.start >= text_end {
            continue;
        }
        let end = r.end.min(text_end).min(r.start + MAX_FUNC_LEN);
        if end <= r.start {
            continue;
        }
        let off = (r.start - text_va) as usize;
        let Some(slice) = text.get(off..off + (end - r.start) as usize) else {
            continue;
        };
        stats.funcs_scanned += 1;

        let mut dec = Decoder::with_ip(64, slice, r.start, DecoderOptions::NONE);
        while dec.can_decode() {
            dec.decode_out(&mut insn);
            if insn.is_invalid() {
                // Inline data: stop this body rather than emit references from a desync.
                stats.bodies_truncated += 1;
                break;
            }
            stats.insns += 1;

            if insn.is_ip_rel_memory_operand() {
                let t = insn.ip_rel_memory_address();
                if insn.mnemonic() == Mnemonic::Lea {
                    stats.lea_rip += 1;
                    if known.exact.contains(&t) {
                        stats.lea_hits += 1;
                        hits.insert((r.start, t));
                    } else if known.suffix_of(found, t).is_some() {
                        stats.lea_suffix += 1;
                        hits.insert((r.start, t));
                    }
                } else if known.hits(found, t) {
                    // Diagnostic only: not an address-taking.
                    stats.rip_other_hits += 1;
                }
            }

            for i in 0..insn.op_count() {
                if insn.op_kind(i) == OpKind::Immediate64 {
                    let v = insn.immediate64();
                    if known.hits(found, v) {
                        stats.imm_hits += 1;
                        hits.insert((r.start, v));
                    }
                }
            }
        }
    }

    // Keep only functions present in the database: a root without a `function` row has no anchor
    // to attach its string to.
    hits.retain(|(f, _)| mapped.contains(f));
    hits
}

/// Extract strings from `exe`, ingest them into `str`, and (unless `no_xrefs`) anchor the
/// functions that reference them in `func_str_ref` and `xref`.
pub fn run(db: &nie_index::Db, exe: &Path, opts: &Options) -> Result<Stats> {
    let bytes = std::fs::read(exe).with_context(|| format!("read {}", exe.display()))?;
    let sha = hex::encode(Sha256::digest(&bytes));
    let pe = PE::parse(&bytes).with_context(|| format!("parse PE {}", exe.display()))?;

    let conn = db.conn();
    let (binary_id, binary_path) = resolve_binary(conn, exe, &sha, opts.binary_id)?;

    let wanted: Vec<String> = if opts.sections.is_empty() {
        DEFAULT_SECTIONS.iter().map(|s| (*s).to_string()).collect()
    } else {
        opts.sections.clone()
    };

    let mut stats = Stats {
        binary_id,
        binary_path,
        dry_run: opts.dry_run,
        ..Stats::default()
    };

    // --- 1. Strings ---------------------------------------------------------
    let mut found: Vec<FoundStr> = Vec::new();
    for (idx, sec) in sections(&pe, &bytes, &wanted).into_iter().enumerate() {
        let before = (stats.ascii, stats.utf16);
        let mut covered = vec![false; sec.data.len() + 1];
        let mut n = found.len();
        scan_ascii(
            sec.data,
            sec.va,
            idx,
            opts.min_len,
            &mut found,
            &mut covered,
            &mut stats.oversized,
        );
        stats.ascii += found.len() - n;
        n = found.len();
        scan_utf16(
            sec.data,
            sec.va,
            idx,
            opts.min_len,
            &mut found,
            &covered,
            &mut stats.oversized,
        );
        stats.utf16 += found.len() - n;
        stats
            .par_section
            .push((sec.name, stats.ascii - before.0, stats.utf16 - before.1));
    }
    found.sort_unstable_by_key(|f| f.vaddr);
    found.dedup_by_key(|f| f.vaddr);

    if opts.sample > 0 {
        for f in found.iter().take(opts.sample) {
            let sec = stats.par_section.get(f.sec).map_or("?", |s| s.0.as_str());
            println!("  0x{:x} {sec} {} {}", f.vaddr, f.enc.tag(), f.value);
        }
    }

    // --- 2. References from .text ----------------------------------------
    let mut hits: HashSet<(u64, u64)> = HashSet::new();
    let mut fid: HashMap<u64, i64> = HashMap::new();
    if !opts.no_xrefs {
        let roots = nie_re_core::nie::pdata::parse_roots(&bytes)?;
        stats.roots = roots.len();
        {
            let mut q = conn.prepare("SELECT vaddr, id FROM function WHERE binary_id = ?1")?;
            let rows = q.query_map([binary_id], |r| {
                Ok((r.get::<_, i64>(0)? as u64, r.get::<_, i64>(1)?))
            })?;
            for row in rows {
                let (v, id) = row?;
                fid.insert(v, id);
            }
        }
        let mapped: HashSet<u64> = roots
            .iter()
            .map(|r| r.start)
            .filter(|s| fid.contains_key(s))
            .collect();
        stats.roots_mapped = mapped.len();

        let text = pe
            .sections
            .iter()
            .find(|s| s.name().is_ok_and(|n| n.starts_with(".text")))
            .context("section .text not found")?;
        let toff = text.pointer_to_raw_data as usize;
        let tlen = text.virtual_size.min(text.size_of_raw_data) as usize;
        let tbytes = bytes
            .get(toff..toff + tlen)
            .context(".text outside the file bounds")?;
        let tva = pe.image_base + u64::from(text.virtual_address);

        let index = StrIndex::new(&found);
        hits = scan_text(tbytes, tva, &roots, &found, &index, &mapped, &mut stats);
        stats.pairs = hits.len();
        stats.funcs_with_str = hits.iter().map(|(f, _)| *f).collect::<HashSet<_>>().len();

        // Suffix targets have no `str` row: materialize them from the containing string, otherwise the
        // anchor would point to an address without text.
        let mut extra: Vec<FoundStr> = Vec::new();
        for (_, t) in &hits {
            if index.exact.contains(t) {
                continue;
            }
            if let Some((fi, off)) = index.suffix_of(&found, *t) {
                extra.push(suffix_str(&found[fi], *t, off));
            }
        }
        extra.sort_unstable_by_key(|f| f.vaddr);
        extra.dedup_by_key(|f| f.vaddr);
        stats.suffixes = extra.len();
        found.extend(extra);
        found.sort_unstable_by_key(|f| f.vaddr);
        found.dedup_by_key(|f| f.vaddr);
    }

    if opts.dry_run {
        return Ok(stats);
    }

    // --- 3. Écriture --------------------------------------------------------
    let by_addr: HashMap<u64, &FoundStr> = found.iter().map(|f| (f.vaddr, f)).collect();
    conn.execute_batch("BEGIN")?;
    {
        let mut ins = conn.prepare_cached(
            "INSERT OR IGNORE INTO str(binary_id, vaddr, len, section, value, kind)
             VALUES(?1,?2,?3,?4,?5,?6)",
        )?;
        for f in &found {
            let sec = stats.par_section.get(f.sec).map_or("?", |s| s.0.as_str());
            stats.str_inserted += ins.execute(params![
                binary_id,
                f.vaddr as i64,
                f.len as i64,
                sec,
                f.value,
                f.enc.tag()
            ])?;
        }
    }
    if !opts.no_xrefs {
        stats.str_refs_deleted = conn.execute(
            "DELETE FROM func_str_ref WHERE binary_id = ?1 AND source = ?2",
            params![binary_id, SOURCE],
        )?;
        let mut ins = conn.prepare_cached(
            "INSERT INTO func_str_ref(binary_id, function_id, value, source) VALUES(?1,?2,?3,?4)",
        )?;
        let mut xins = conn.prepare_cached(
            "INSERT OR IGNORE INTO xref(binary_id, from_addr, to_addr, kind) VALUES(?1,?2,?3,?4)",
        )?;
        for (f, s) in &hits {
            let (Some(&id), Some(st)) = (fid.get(f), by_addr.get(s)) else {
                continue;
            };
            stats.str_refs_inserted += ins.execute(params![binary_id, id, st.value, SOURCE])?;
            stats.xrefs_inserted +=
                xins.execute(params![binary_id, *f as i64, *s as i64, XREF_KIND])?;
        }
    }
    conn.execute_batch("COMMIT")?;
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_exige_la_terminaison_nul() {
        // "abcd\0" is kept; "efgh" without a final NUL is not.
        let data = b"abcd\0efgh".to_vec();
        let mut out = Vec::new();
        let mut cov = vec![false; data.len() + 1];
        let mut over = 0;
        scan_ascii(&data, 0x1000, 0, 4, &mut out, &mut cov, &mut over);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].value, "abcd");
        assert_eq!(out[0].vaddr, 0x1000);
        assert_eq!(out[0].len, 4);
    }

    #[test]
    fn ascii_ecarte_les_trop_courtes() {
        let data = b"ab\0abcdef\0".to_vec();
        let mut out = Vec::new();
        let mut cov = vec![false; data.len() + 1];
        let mut over = 0;
        scan_ascii(&data, 0, 0, 4, &mut out, &mut cov, &mut over);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].value, "abcdef");
    }

    #[test]
    fn utf16_retenu_et_non_recouvrant() {
        let mut data = Vec::new();
        for c in "Menu".encode_utf16() {
            data.extend_from_slice(&c.to_le_bytes());
        }
        data.extend_from_slice(&0u16.to_le_bytes());
        let mut out = Vec::new();
        let cov = vec![false; data.len() + 1];
        let mut over = 0;
        scan_utf16(&data, 0x2000, 0, 4, &mut out, &cov, &mut over);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].value, "Menu");
        assert_eq!(out[0].enc, Enc::Utf16);
    }

    #[test]
    fn utf16_refuse_ce_qui_recouvre_une_ascii() {
        // "abcdefgh\0\0" re-reads as CJK in UTF-16: the ASCII coverage forbids it.
        let data = b"abcdefgh\0\0".to_vec();
        let mut ascii = Vec::new();
        let mut cov = vec![false; data.len() + 1];
        let mut over = 0;
        scan_ascii(&data, 0, 0, 4, &mut ascii, &mut cov, &mut over);
        assert_eq!(ascii.len(), 1);
        let mut wide = Vec::new();
        scan_utf16(&data, 0, 0, 4, &mut wide, &cov, &mut over);
        assert!(wide.is_empty(), "ASCII overlap not filtered");
    }

    #[test]
    fn blocs_textuels_utf16_restreints() {
        assert!(wide_is_text(0x0041)); // 'A'
        assert!(wide_is_text(0x30A2)); // katakana ア
        assert!(wide_is_text(0x4E00)); // CJK 一
        assert!(!wide_is_text(0x0000));
        assert!(!wide_is_text(0x0007));
        assert!(!wide_is_text(0xD800)); // half of a surrogate pair
    }
}
