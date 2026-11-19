//! The committed `bindings/` directory must equal what the contract of nie-abi produces.

use std::path::Path;

use nie_bindgen::{Lang, default_lib, emit_all, parse_contract};

#[test]
fn committed_bindings_match_the_contract() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).expect("root");
    let contract = parse_contract(nie_abi::CONTRACT_JSON).expect("contract");
    let lib = default_lib(&contract);
    let mut stale = Vec::new();
    for (rel, text) in emit_all(&contract, &lib, &Lang::ALL) {
        let on_disk = std::fs::read_to_string(root.join("bindings").join(&rel)).ok();
        if on_disk.as_deref() != Some(text.as_str()) {
            stale.push(rel);
        }
    }
    assert!(stale.is_empty(), "stale bindings, run `nie bindgen`: {stale:?}");
}
