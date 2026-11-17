//! The committed generated files must equal what the contract produces.

use rg_ffi_contract::{emit_c_header, emit_openapi, emit_rust, emit_ts, load};

fn disk(rel: &str) -> String {
    std::fs::read_to_string(format!("{}/{rel}", env!("CARGO_MANIFEST_DIR")))
        .unwrap_or_else(|e| panic!("{rel}: {e}"))
}

#[test]
fn generated_files_match_the_contract() {
    let c = load();
    assert_eq!(disk("include/rg_ffi.h"), emit_c_header(&c));
    assert_eq!(disk("src/generated.rs"), emit_rust(&c));
    assert_eq!(disk("bindings/rg_ffi.ts"), emit_ts(&c));
    assert_eq!(disk("openapi/rg_ffi.openapi.json"), emit_openapi(&c));
}

#[test]
fn every_contract_function_has_an_implementation_and_an_export() {
    let c = load();
    let generated = disk("src/generated.rs");
    let impls = disk("src/impls.rs");
    for f in &c.functions {
        assert!(
            generated.contains(&format!("fn {}(", f.name)),
            "{} exported",
            f.name
        );
        assert!(
            impls.contains(&format!("pub fn {}(", f.name)),
            "{} implemented",
            f.name
        );
    }
}
