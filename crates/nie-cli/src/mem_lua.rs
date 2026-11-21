//! `nie mem lua-field` - read the value of a Lua table field in the live game.
//!
//! Some screen values live in **no game file**: the Lua chunk declares the key (`listRowNum`,
//! `listLineNum`, `pageNum`...) but never assigns it; the C++ engine sets it when the screen is
//! instantiated. File sources were exhausted without finding it, so the only possible read is
//! process memory.
//!
//! The chain, established by hand on the game process:
//!
//! 1. Lua interns short strings. An x86-64 `TString` (Lua 5.2) is
//!    `next(8) | tt(1) marked(1) extra(1) pad(1) hash(4) | len(8) | data...`: searching for `len`
//!    followed by the name and its NUL identifies the string object, and `TString* = addr(len) - 16`.
//! 2. A table entry is a `Node { TValue i_val; TKey i_key; }` = **40 bytes** (`TValue` 16; `TKey` =
//!    `value_(8) tt_(4) pad(4) next(8)` 24). Searching for the pointer to the `TString` gives the
//!    position of `i_key.value_`; the value is 16 bytes BEFORE it.
//! 3. `i_key.tt_` must be `LUA_TSTRING` (4); the observed runtime also sets the collectable bit
//!    0x40, hence the mask.
//!
//! The same process holds several Lua states (several interned `TString`s for one name) and
//! several instantiated screens: the command returns **all** occurrences rather than one answer;
//! the caller knows which screen it opened.

use anyhow::Context as _;

/// Size of a Lua 5.2 table `Node` on x86-64.
const NODE_SIZE: u64 = 40;
/// Offset of `i_key.value_` from the start of the `Node` (the size of a `TValue`).
const KEY_OFFSET: u64 = 16;
/// Offset of `TString.len` from the start of the `TString` object.
const TSTRING_LEN_OFFSET: u64 = 16;
/// Mask that removes the "collectable" bit from the type tag.
const TT_MASK: u32 = 0x3f;

/// A field read: where it lives and what it holds.
struct Releve {
    /// `Node` address (start of `i_val`).
    node: u64,
    /// Decoded value, made readable.
    valeur: String,
    /// Lua type tag of the value, with the collectable bit masked.
    tt: u32,
}

/// Readable name of a Lua type tag.
fn nom_type(tt: u32) -> &'static str {
    match tt & TT_MASK {
        0 => "nil",
        1 => "boolean",
        2 => "lightuserdata",
        3 => "number",
        4 => "string",
        5 => "table",
        6 => "function",
        7 => "userdata",
        8 => "thread",
        _ => "?",
    }
}

/// Render a `TValue` (8 payload bytes + tag) as readable text.
///
/// A Lua 5.2 `number` is a `double`: `2.0` prints as `2` to stay comparable with what a Lua script
/// would show, and the fractional form is kept when present.
fn decoder_valeur(charge: [u8; 8], tt: u32) -> String {
    let brut = u64::from_le_bytes(charge);
    match tt & TT_MASK {
        0 => "nil".to_string(),
        1 => if brut != 0 { "true" } else { "false" }.to_string(),
        3 => {
            let d = f64::from_le_bytes(charge);
            if d.fract() == 0.0 && d.abs() < 1e15 {
                format!("{}", d as i64)
            } else {
                format!("{d}")
            }
        }
        t => format!("{}@0x{brut:x}", nom_type(t)),
    }
}

/// Read `n` bytes, or return `None` if the range is not readable.
fn lire(pid: i32, addr: u64, n: usize) -> Option<Vec<u8>> {
    nie_trace::read_exact(pid, addr, n).ok()
}

/// Read a little-endian `u32`.
fn u32_at(buf: &[u8], off: usize) -> Option<u32> {
    buf.get(off..off + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Read a little-endian `u64`.
fn u64_at(buf: &[u8], off: usize) -> Option<u64> {
    let b = buf.get(off..off + 8)?;
    Some(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}

/// Return the name of a Lua `TString`, if the object has that shape.
fn nom_tstring(pid: i32, tstring: u64, max: usize) -> Option<String> {
    let buf = lire(pid, tstring, TSTRING_LEN_OFFSET as usize + 8 + max)?;
    let tt = *buf.get(8)?;
    if tt & TT_MASK as u8 != 4 {
        return None;
    }
    let len = u64_at(&buf, TSTRING_LEN_OFFSET as usize)? as usize;
    if len == 0 || len > max {
        return None;
    }
    let debut = TSTRING_LEN_OFFSET as usize + 8;
    let octets = buf.get(debut..debut + len)?;
    std::str::from_utf8(octets).ok().map(str::to_string)
}

/// Find interned `TString` objects holding exactly `nom`.
fn trouver_tstrings(pid: i32, nom: &str, limite: usize) -> Vec<u64> {
    let mut motif = (nom.len() as u64).to_le_bytes().to_vec();
    motif.extend_from_slice(nom.as_bytes());
    motif.push(0);
    let regions = nie_trace::module_regions(pid, "", true);
    nie_trace::scan_regions(pid, &regions, None, &motif, limite)
        .into_iter()
        .map(|h| h.addr - TSTRING_LEN_OFFSET)
        .collect()
}

/// Find `Node`s whose key is `tstring` and decode their value.
fn relever_nodes(pid: i32, tstring: u64, limite: usize) -> Vec<Releve> {
    let motif = tstring.to_le_bytes().to_vec();
    let regions = nie_trace::module_regions(pid, "", true);
    let mut out = Vec::new();
    for hit in nie_trace::scan_regions(pid, &regions, None, &motif, limite) {
        // `hit.addr` is the position of `i_key.value_`; the Node starts 16 bytes before.
        let node = match hit.addr.checked_sub(KEY_OFFSET) {
            Some(n) => n,
            None => continue,
        };
        let Some(buf) = lire(pid, node, NODE_SIZE as usize) else {
            continue;
        };
        // The key must be a string: otherwise the pointer was found somewhere other than a key
        // position (interned string table, a prototype constant pool...).
        let Some(key_tt) = u32_at(&buf, KEY_OFFSET as usize + 8) else {
            continue;
        };
        if key_tt & TT_MASK != 4 {
            continue;
        }
        let Some(val_tt) = u32_at(&buf, 8) else {
            continue;
        };
        let mut charge = [0u8; 8];
        charge.copy_from_slice(&buf[..8]);
        out.push(Releve {
            node,
            valeur: decoder_valeur(charge, val_tt),
            tt: val_tt & TT_MASK,
        });
    }
    out
}

/// Scan neighbouring `Node`s and return the `key -> value` pairs of the same hash table.
fn voisins(pid: i32, node: u64, rayon: i64, max_nom: usize) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for k in -rayon..=rayon {
        let Some(addr) = node.checked_add_signed(k * NODE_SIZE as i64) else {
            continue;
        };
        let Some(buf) = lire(pid, addr, NODE_SIZE as usize) else {
            continue;
        };
        let Some(key_tt) = u32_at(&buf, KEY_OFFSET as usize + 8) else {
            continue;
        };
        if key_tt & TT_MASK != 4 {
            continue;
        }
        let Some(key_ptr) = u64_at(&buf, KEY_OFFSET as usize) else {
            continue;
        };
        let Some(nom) = nom_tstring(pid, key_ptr, max_nom) else {
            continue;
        };
        let Some(val_tt) = u32_at(&buf, 8) else {
            continue;
        };
        let mut charge = [0u8; 8];
        charge.copy_from_slice(&buf[..8]);
        out.push((nom, decoder_valeur(charge, val_tt)));
    }
    out
}

/// Entry point of `nie mem lua-field`.
///
/// `rayon` > 0 also prints the neighbouring entries of each `Node` found, which yields the full
/// state table (`listNum`, `listRowNum`, `listLineNum`, `pageNum`, `lastLine`) instead of an
/// isolated field. `nombres_seuls` drops entries whose value is not a scalar, the main noise of
/// the method.
pub fn lua_field(
    pid: i32,
    nom: &str,
    limite_chaines: usize,
    limite_nodes: usize,
    rayon: i64,
    nombres_seuls: bool,
) -> anyhow::Result<()> {
    let pid = if pid > 0 {
        pid
    } else {
        nie_trace::find_pid_by_name("nie.exe")
            .context("nie.exe not found: start the game, or pass --pid")?
    };

    let tstrings = trouver_tstrings(pid, nom, limite_chaines);
    if tstrings.is_empty() {
        println!("  no interned TString '{nom}'; the field does not exist in this process.");
        return Ok(());
    }
    println!(
        "  {} TString object(s) '{nom}' (pid {pid})",
        tstrings.len()
    );

    let mut total = 0usize;
    for ts in &tstrings {
        let mut releves = relever_nodes(pid, *ts, limite_nodes);
        // A TString pointer also shows up in key position of UNRELATED entries (global tables,
        // `_LOADED`, constant pools). Keeping only numeric values removes that noise at once when
        // looking for a screen dimension.
        if nombres_seuls {
            releves.retain(|r| r.tt == 3 || r.tt == 1);
        }
        if releves.is_empty() {
            continue;
        }
        println!(
            "\n  TString 0x{ts:x} - {} table entry(ies)",
            releves.len()
        );
        for r in &releves {
            total += 1;
            println!("    node 0x{:012x}  {nom} = {}", r.node, r.valeur);
            if rayon > 0 {
                for (k, v) in voisins(pid, r.node, rayon, 64) {
                    if k != nom {
                        println!("      · {k} = {v}");
                    }
                }
            }
        }
    }
    if total == 0 {
        println!("\n  no table entry: the key is interned but no table holds it.");
    } else {
        println!("\n  {total} entry(ies) in total.");
    }
    Ok(())
}

// --- Avatar editor palettes ---

/// Size of a palette table entry: id then color, aligned to 16.
const TAILLE_ENTREE_PALETTE: usize = 16;

/// Read the `colorPresetID -> color` table of the avatar editor from the live game.
///
/// The catalogue (`m_CharaEditColorDataList`) only gives **ids**: the color values exist neither in
/// that file nor in the binary (the pattern of a known entry is absent there), nor in a form
/// resolvable from strings, unlike the `red`, `green` and `blue` channels. They are readable only
/// in process memory, where the game loads them.
///
/// Table shape, as read: per entry, the CRC-32 id little-endian on 4 bytes, then the **ARGB** color
/// on 4 bytes (alpha first, `0xFF` for an opaque color), the rest being alignment. Entries follow
/// each other, aligned to 4.
///
/// `identifiants` bounds the search: only the expected ids are kept, which drops the coincidences
/// a free memory scan would certainly produce.
pub fn palettes(
    pid: i32,
    identifiants: &[u32],
    debut: u64,
    longueur: usize,
) -> anyhow::Result<Vec<(u32, [u8; 4])>> {
    let pid = if pid > 0 {
        pid
    } else {
        nie_trace::find_pid_by_name("nie.exe")
            .context("nie.exe not found: start the game, or pass --pid")?
    };
    let attendus: std::collections::BTreeSet<u32> = identifiants.iter().copied().collect();
    let octets = nie_trace::read_exact(pid, debut, longueur)
        .map_err(|e| anyhow::anyhow!("read of {longueur} bytes at 0x{debut:x}: {e}"))?;

    let mut vus: std::collections::BTreeMap<u32, [u8; 4]> = std::collections::BTreeMap::new();
    let mut offset = 0usize;
    while offset + 8 <= octets.len() {
        let Some(id) = u32_at(&octets, offset) else {
            break;
        };
        if attendus.contains(&id) {
            let argb = [
                octets[offset + 4],
                octets[offset + 5],
                octets[offset + 6],
                octets[offset + 7],
            ];
            // An id can reappear elsewhere than at an entry head: the scan is free, in 4-byte steps. Real
            // table entries are OPAQUE: preferring `alpha = 255` drops those coincidences, which produced
            // semi-transparent colors.
            match vus.entry(id) {
                std::collections::btree_map::Entry::Vacant(v) => {
                    v.insert(argb);
                }
                std::collections::btree_map::Entry::Occupied(mut o) => {
                    if o.get()[0] != 0xFF && argb[0] == 0xFF {
                        o.insert(argb);
                    }
                }
            }
        }
        offset += 4;
    }
    let _ = TAILLE_ENTREE_PALETTE;
    Ok(vus.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_number_entier_se_rend_sans_partie_decimale() {
        assert_eq!(decoder_valeur(2.0f64.to_le_bytes(), 3), "2");
        assert_eq!(decoder_valeur(6.0f64.to_le_bytes(), 3), "6");
    }

    #[test]
    fn un_number_fractionnaire_garde_sa_partie_decimale() {
        assert_eq!(decoder_valeur(0.5f64.to_le_bytes(), 3), "0.5");
    }

    #[test]
    fn le_bit_collectable_ne_change_pas_le_type() {
        // The observed runtime marks strings 0x44 = LUA_TSTRING | 0x40.
        assert_eq!(nom_type(0x44), "string");
        assert_eq!(nom_type(4), "string");
        assert_eq!(nom_type(0x45), "table");
    }

    #[test]
    fn un_booleen_se_rend_en_mots() {
        assert_eq!(decoder_valeur(1u64.to_le_bytes(), 1), "true");
        assert_eq!(decoder_valeur(0u64.to_le_bytes(), 1), "false");
    }
}
