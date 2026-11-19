//! @name@: WebAssembly module on the nie framework, built for `wasm32-unknown-unknown` with plain
//! `extern "C"` exports (no bindgen, no runtime dependency).

@plugin@
/// Wrapping sum of two integers.
#[unsafe(no_mangle)]
pub extern "C" fn add(a: i32, b: i32) -> i32 {
    a.wrapping_add(b)
}
