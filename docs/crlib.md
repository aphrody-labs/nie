# crlib: every library crate as a C-ABI library

A workspace member that is a library either builds as a C-ABI library ("crlib") or is listed with
the reason it does not. `crates/nie-abi/crlib.json` is the single list of every member; a test in
`crates/nie-abi/tests/crlib.rs` fails when a member is missing from it, when a crlib does not build
as `rlib`, `staticlib` and `cdylib`, or when a generated file drifts.

## What a crlib is

- `crate-type = ["rlib", "staticlib", "cdylib"]` and a cargo feature `crlib` (off by default, so
  Rust consumers and the other crates never see the C symbols).
- `<crate>/abi.json` lists the exported functions (types: `f64 u64 u32 i32 bool str string bytes`,
  the contract format of `nie-abi`). Their Rust bodies are in `<crate>/src/abi_impls.rs`.
- `nie-abi-gen --crlib .` writes `include/nie_<id>.h` and `<crate>/src/abi_generated.rs` (the
  `extern "C"` layer, panic-safe, behind `#[cfg(feature = "crlib")]`). Never edit them.
- Every crlib exports `nie_<id>_abi_version`, `nie_<id>_info` (JSON: crate, version, ABI version,
  symbols) and `nie_<id>_string_free`, plus its own `nie_<id>_*` functions.
- `crates/nie-ffi` keeps its own contract (library `nie`, `crates/nie-abi/contract.json`); it only
  gains the `staticlib` crate type.

Link one crlib per program: a staticlib bundles its dependencies, so two staticlibs that share a
dependency would define the same symbols twice. The `cdylib` form has no such limit.

## Build and check

```sh
cargo build --release -p nie-formats --features crlib    # target/release/libnie_formats.{a,so}
cargo run -p nie-abi -- --crlib .                        # regenerate headers and layers
cargo run -p nie-abi -- --crlib --check .                # drift check (CI)
cargo test -p nie-abi                                    # manifest, crate types, drift, symbols
```

`just compile --crlib` builds every crlib on the build host and collects the libraries and
headers.

## Adding a crate

1. Add the crate to `crates/nie-abi/crlib.json` (`status: crlib` with an `id`, or `excluded` with a
   `reason`).
2. For a crlib: set `crate-type`, add `crlib = []` to `[features]` (`["std", ...]` when the crate is
   `no_std` by default), add `abi.json`, and append to `src/lib.rs`:
   ```rust
   #[cfg(feature = "crlib")]
   #[allow(unsafe_code)]
   mod abi_generated;
   #[cfg(feature = "crlib")]
   mod abi_impls; // only when abi.json has functions
   ```
   A crate-level `#![forbid(unsafe_code)]` becomes
   `#![cfg_attr(not(feature = "crlib"), forbid(unsafe_code))]` plus
   `#![cfg_attr(feature = "crlib", deny(unsafe_code))]`.
3. Run `cargo run -p nie-abi -- --crlib .` and commit the generated files.

## Exclusions

Binary-only crates and services, the build-time generators, Rust trait contracts, GPU and engine
types (wgpu, Bevy), wasm-bindgen bindings (their JavaScript surface is their ABI) and host
automation have no portable C surface. `crlib.json` states the reason for each. There are no
proc-macro crates in the workspace.
