//! Native Rust implementation of the LEVEL-5 mod operations: dump, verify, pack, merge and Criware (de)encryption.
//!
//! # Not ported
//!
//! The fifth Viola button, *Decrypt/Encrypt Criware*, needed no port: it already existed in
//! [`nie_formats::cpk::decrypt_block`], a faithful port of `CriwareCrypt.DecryptBlock`. The
//! [`crypto`] module only gives it a file facade and names the property that makes the operation
//! unique: the XOR is **involutive**, so encrypting and decrypting are the same computation.
//!
//! # Deliberate differences from upstream
//!
//! * Upstream `pack` only knows the XorShift envelope, while IEVR's `cpk_list.cfg.bin` is
//!   encrypted with **AES-256-CBC** (key and IV reversed from `nie.exe`). [`pack`] re-reads and
//!   re-writes in the envelope it actually finds, whatever it is.
//! * Upstream `dump` assigns one thread per pack drawing from an unsorted list; [`dump`] orders
//!   the packs by decreasing size, memory-maps them and can resume an interrupted dump. Details
//!   and reasons are in that module's documentation.

#![warn(missing_docs)]

pub mod crypto;
pub mod dump;
pub mod filtre;
pub mod manifeste;
pub mod merge;
pub mod pack;
pub mod patch;
pub mod presets;
pub mod verify;

pub use crypto::{CriwareKey, crypt_bytes, crypt_file};
pub use dump::{DumpOptions, DumpProgress, DumpReport, Echec, Raison, dump_all};
pub use filtre::Filtre;
pub use manifeste::{Dependance, FichierMod, Manifeste, Version, ordonner};
pub use merge::{Conflit, MergeReport, MergeStrategy, merge_dirs};
pub use pack::{CpkListCrypto, PackReport, Platform, decode_cpk_list, encode_cpk_list, pack_mod};
pub use verify::{Anomalie, Constat, VerifOptions, VerifReport, verifier};

use std::path::{Path, PathBuf};

/// Filtre glob minimal — seul `*` est reconnu, ce qui couvre les motifs réellement utilisés
/// (`*.g4tx`, `data/chr/*`). Écrit à la main plutôt qu'en tirant une dépendance de glob : la
/// syntaxe complète (classes, `**`, échappements) n'a aucun appelant ici et coûterait un crate.
#[must_use]
pub fn glob_match(motif: &str, texte: &str) -> bool {
    let mut morceaux = motif.split('*');
    let Some(debut) = morceaux.next() else {
        return true;
    };
    if !texte.starts_with(debut) {
        return false;
    }
    let mut pos = debut.len();
    let mut dernier = "";
    let mut a_joker = false;
    for m in morceaux {
        a_joker = true;
        dernier = m;
        if m.is_empty() {
            continue;
        }
        match texte[pos..].find(m) {
            Some(i) => pos += i + m.len(),
            None => return false,
        }
    }
    if !a_joker {
        return texte == motif; // aucun `*` : égalité stricte
    }
    // Un motif qui ne finit pas par `*` doit coller jusqu'au bout du texte.
    dernier.is_empty() || texte.ends_with(dernier)
}

/// Énumère récursivement les fichiers d'un dossier, en chemins relatifs normalisés (`/`).
///
/// L'ordre est **stable** (tri par chemin) : deux exécutions du même *pack* ou du même *merge*
/// produisent alors des sorties identiques, ce qui rend les résultats comparables et
/// diffusables sans bruit.
///
/// # Errors
/// Si un répertoire est illisible.
pub fn enumerer(racine: &Path) -> Result<Vec<(PathBuf, String)>, String> {
    let mut out = Vec::new();
    let mut pile = vec![racine.to_path_buf()];
    while let Some(dir) = pile.pop() {
        let lecture = std::fs::read_dir(&dir).map_err(|e| format!("{} : {e}", dir.display()))?;
        for e in lecture.flatten() {
            let chemin = e.path();
            match e.file_type() {
                Ok(t) if t.is_dir() => pile.push(chemin),
                Ok(t) if t.is_file() => {
                    let Ok(rel) = chemin.strip_prefix(racine) else {
                        continue;
                    };
                    out.push((chemin.clone(), rel.to_string_lossy().replace('\\', "/")));
                }
                // Les liens et les entrées illisibles sont ignorés plutôt que de faire échouer
                // toute l'opération sur un cas marginal.
                _ => {}
            }
        }
    }
    out.sort_unstable_by(|a, b| a.1.cmp(&b.1));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_glob_ne_reconnait_que_l_etoile() {
        assert!(glob_match("*.g4tx", "data/chr/c01.g4tx"));
        assert!(!glob_match("*.g4tx", "data/chr/c01.g4md"));
        assert!(glob_match("data/chr/*", "data/chr/c01.g4md"));
        assert!(!glob_match("data/chr/*", "data/menu/x"));
        assert!(glob_match("*chr*g4md", "data/chr/c01.g4md"));
        assert!(glob_match("*", "n'importe quoi"));
        assert!(glob_match("exact", "exact"));
        assert!(
            !glob_match("exact", "exactement"),
            "sans joker, l'égalité est stricte"
        );
        assert!(!glob_match("exact", "inexact"));
    }

    #[test]
    fn l_enumeration_est_recursive_et_ordonnee() {
        let dir = std::env::temp_dir().join(format!("nie-viola-enum-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("b/c")).expect("arborescence de test");
        std::fs::write(dir.join("z.bin"), b"z").expect("z");
        std::fs::write(dir.join("b/c/a.bin"), b"a").expect("a");
        let v = enumerer(&dir).expect("énumération");
        let rels: Vec<&str> = v.iter().map(|(_, r)| r.as_str()).collect();
        assert_eq!(
            rels,
            vec!["b/c/a.bin", "z.bin"],
            "récursif, trié, séparateurs normalisés"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}

#[cfg(feature = "crlib")]
#[allow(unsafe_code)]
mod abi_generated;
