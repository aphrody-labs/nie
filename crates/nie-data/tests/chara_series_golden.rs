#![allow(clippy::pedantic)]
//! Tests golden `chara_series` — port 1:1 d'inagle `belong-team.ts` (`loadCharaSeriesInfo`).
//!
//! Vérité terrain : **9 séries** ; `0x62E8448F` a le type 1. Aucun libellé de série n'existe dans
//! le jeu.

use nie_data::chara_series::{find_by_id, parse_chara_series_config};
use nie_data::hash::HashId;
use serde_json::json;

#[test]
fn parses_config_and_lookup() {
    let root = json!({
        "lists": [{
            "name": "m_charaSeriesInfoList",
            "values": [
                { "charaSeriesId": "0x62E8448F", "charaSeriesType": 1, "charaSeriesNameTextId": "0xE6D1097B" },
                { "charaSeriesId": "0xE177FC30", "charaSeriesType": 9, "charaSeriesNameTextId": "0x00000000" },
            ]
        }]
    });
    let series = parse_chara_series_config(&root);
    assert_eq!(series.len(), 2);
    assert_eq!(series[0].series_id, HashId(0x62E8_448F));
    assert_eq!(series[0].series_type, 1);
    let found = find_by_id(&series, HashId(0xE177_FC30)).expect("série 9 présente");
    assert_eq!(found.series_type, 9);
    assert_eq!(find_by_id(&series, HashId(0xDEAD_BEEF)), None);
}
