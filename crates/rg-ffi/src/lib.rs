//! C ABI of the Rust core. The `extern "C"` layer in `generated.rs` is produced from the contract
//! (`crates/rg-ffi-contract/contract.json`); the behaviour lives in `impls.rs` as plain safe Rust.
//! Regenerate with `cargo run -p rg-ffi-contract -- crates/rg-ffi`.

// The generated layer is the only place with unsafe code (raw pointers at the boundary).
#[allow(unsafe_code, unused_imports, missing_docs, clippy::missing_safety_doc)]
#[rustfmt::skip]
pub mod generated;
pub mod impls;
