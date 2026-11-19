//! @name@: C ABI library on the nie framework. `abi.json` is the contract in the `nie-abi` format;
//! `nie bindgen --contract abi.json --lib @ident@` emits the bindings of every language from it.

use std::ffi::{CString, c_char};

@plugin@
/// ABI version of this library; equals `abi_version` in `abi.json`.
#[unsafe(no_mangle)]
pub extern "C" fn nie_ffi_abi_version() -> u32 {
    1
}

/// Frees a string returned by this library.
///
/// # Safety
/// `s` must be NULL or a pointer returned by this library and not freed yet.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nie_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}

/// Version of the crate as an owned string; free it with `nie_string_free`.
#[unsafe(no_mangle)]
pub extern "C" fn nie_@ident@_version() -> *mut c_char {
    CString::new(env!("CARGO_PKG_VERSION")).map_or(std::ptr::null_mut(), CString::into_raw)
}

/// Wrapping sum of two integers.
#[unsafe(no_mangle)]
pub extern "C" fn nie_@ident@_add(a: i32, b: i32) -> i32 {
    a.wrapping_add(b)
}
