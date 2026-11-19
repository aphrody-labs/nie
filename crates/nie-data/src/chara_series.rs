//! `chara_series` — séries de jeu d'un personnage (`character/chara_series_config.cfg.bin`).
//!
//! ## Vérité terrain (anti-hallucination)
//!
//! - Parser TS de référence : `inagle`
//!   (`loadCharaSeriesInfo`) — **port 1:1**.
//! - Données : `data/common/gamedata/character/chara_series_config.cfg.bin` (VFS IEVR), format
//!   **RDBN** (`lists`). Liste `m_charaSeriesInfoList`, type `CHARA_SERIES_INFO`, **9 séries** ;
//!   champs réels (probe live) : `charaSeriesId` (Hash), `charaSeriesType` (Int),
//!   `charaSeriesNameTextId` (Hash). Ex. row0 : `0x62E8448F` / type 1 / `0xE6D1097B`.
//!
//! ## Noms de série
//!
//! Le jeu ne porte aucun libellé de série : `charaSeriesId` et `charaSeriesType` sont servis tels
//! quels, sans table de noms écrite à la main.

use serde_json::Value;

use crate::cfgbin::{field_hash, field_i64, list_values};
use crate::hash::HashId;

/// Une entrée `CHARA_SERIES_INFO` portée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharaSeriesInfo {
    /// `charaSeriesId` — identifiant de série (clé ; lié à `chara_base.series_id`).
    pub series_id: HashId,
    /// `charaSeriesType` — type de série (`1..=9`, clé des noms codés en dur).
    pub series_type: i64,
    /// `charaSeriesNameTextId` — hash de texte (conservé ; non utilisé pour le nom).
    pub name_text_id: HashId,
}

/// Parse la liste `m_charaSeriesInfoList` d'un `chara_series_config.cfg.bin.json` désérialisé.
///
/// Port de `loadCharaSeriesInfo` (lecture brute `data.lists[0].values`).
#[must_use]
pub fn parse_chara_series_config(root: &Value) -> alloc::vec::Vec<CharaSeriesInfo> {
    let mut out = alloc::vec::Vec::new();
    if let Some(values) = list_values(root, "m_charaSeriesInfoList") {
        for v in values {
            out.push(CharaSeriesInfo {
                series_id: field_hash(v, "charaSeriesId"),
                series_type: field_i64(v, "charaSeriesType").unwrap_or(0),
                name_text_id: field_hash(v, "charaSeriesNameTextId"),
            });
        }
    }
    out
}

/// Recherche une série par `series_id` (clé de jointure depuis `chara_base.series_id`).
#[must_use]
pub fn find_by_id(series: &[CharaSeriesInfo], series_id: HashId) -> Option<&CharaSeriesInfo> {
    series.iter().find(|s| s.series_id == series_id)
}
