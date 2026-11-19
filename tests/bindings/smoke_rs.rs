// Rust binding smoke test: abi_version and cfgbin_to_sql on a synthetic fixture.
#[path = "../../bindings/rust/nie_contract_sys.rs"]
#[allow(dead_code)]
mod nie_contract_sys;

fn main() {
    let path = std::env::args().nth(1).expect("fixture path");
    assert_eq!(nie_contract_sys::safe::abi_version(), nie_contract_sys::ABI_VERSION);
    let data = std::fs::read(path).expect("fixture");
    let sql = nie_contract_sys::safe::cfgbin_to_sql(&data, 1).expect("sql");
    assert!(sql.contains("CREATE TABLE") && sql.contains("ROOT"));
    assert!(nie_contract_sys::safe::cfgbin_to_sql(b"nope", 1).is_none());
    println!("rust ok {} {}", nie_contract_sys::safe::abi_version(), sql.len());
}
