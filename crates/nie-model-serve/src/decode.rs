//! Decoders shared by the HTTP routes and the conformance check: a cfg.bin as JSON. Its typed
//! `nie-data` form is [`nie_formats::cfgbin::to_tagged_json`].

use nie_formats::cfgbin;

/// Décode un `cfg.bin`/`objbin`/`fxbin`/`mevbin` RDBN en JSON exploitable :
/// `{ format, lists: [ { name, type, count, rows: [ { champ: valeur } ] } ] }`.
/// Les noms de listes/types/champs sont résolus depuis la table de chaînes (lisible).
pub fn cfgbin_to_json(data: &[u8]) -> Option<serde_json::Value> {
    use serde_json::{Map, Value, json};
    if !cfgbin::is_rdbn(data) {
        // Format T2B (cfg.bin Level-5 classique, le cas réel sur IEVR) : arbre
        // hiérarchique {name, variables, children} — sérialisé directement.
        let cfg = cfgbin::cfgbin_parse(data).ok()?;
        return serde_json::to_value(&cfg).ok();
    }
    let rdbn = cfgbin::parse(data).ok()?;
    let lists = cfgbin::read_values(&rdbn, data);
    let lists_json: Vec<Value> = lists
        .iter()
        .map(|l| {
            let rows: Vec<Value> = l
                .rows
                .iter()
                .map(|row| {
                    let mut m = Map::new();
                    for (name, val) in &row.fields {
                        m.insert(name.clone(), cfgbin::rdbn_value_to_tagged_json(val));
                    }
                    Value::Object(m)
                })
                .collect();
            json!({ "name": l.name, "type": l.type_name, "count": rows.len(), "rows": rows })
        })
        .collect();
    Some(json!({ "format": "rdbn", "lists": lists_json }))
}
