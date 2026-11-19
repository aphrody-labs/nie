use std::ffi::CStr;

#[test]
fn exports_match_the_contract() {
    let contract: serde_like::Contract = serde_like::parse(include_str!("../abi.json"));
    assert_eq!(contract.abi_version, @ident@::nie_ffi_abi_version());
    for name in contract.functions {
        let src = include_str!("../src/lib.rs");
        assert!(src.contains(&format!("pub extern \"C\" fn {name}(")), "{name} exported");
    }
}

#[test]
fn calls_through_the_c_abi() {
    assert_eq!(@ident@::nie_@ident@_add(2, 3), 5);
    let v = @ident@::nie_@ident@_version();
    assert!(!v.is_null());
    let text = unsafe { CStr::from_ptr(v) }.to_str().expect("utf8").to_owned();
    unsafe { @ident@::nie_string_free(v) };
    assert_eq!(text, env!("CARGO_PKG_VERSION"));
}

/// Just enough JSON reading for this test, so the app needs no dependency.
mod serde_like {
    pub struct Contract {
        pub abi_version: u32,
        pub functions: Vec<String>,
    }

    pub fn parse(text: &str) -> Contract {
        let after = |key: &str| text.split(key).nth(1).expect(key);
        let abi_version = after("\"abi_version\":")
            .trim_start()
            .split(|c: char| !c.is_ascii_digit())
            .next()
            .expect("number")
            .parse()
            .expect("u32");
        let functions = text
            .split("\"name\": \"")
            .skip(1)
            .map(|s| s.split('"').next().expect("name").to_owned())
            .filter(|n| n.starts_with("nie_"))
            .collect();
        Contract { abi_version, functions }
    }
}
