//! Native runtime for compiled Lua 5.2 chunks, plus the pure library primitives.
//!
//! The pure parts ([`lstr`], [`lsort`], [`bit32`]) have no dependency and also build for wasm.
//! The runtime proper (values, heap, tables, calls, coroutines, standard library) backs code
//! produced by [`crate::aot`] and needs the `aot` feature.

pub mod bit32;
pub mod lsort;
pub mod lstr;

#[cfg(feature = "aot")]
pub mod heap;
#[cfg(feature = "aot")]
pub mod host;
#[cfg(feature = "aot")]
pub mod ops;
#[cfg(feature = "aot")]
pub mod state;
#[cfg(feature = "aot")]
pub mod stdlib;
#[cfg(feature = "aot")]
pub mod table;
#[cfg(feature = "aot")]
pub mod value;

/// Position prefix of VM-raised errors in stripped chunks (`luaG_addinfo` with no debug info).
pub const ERROR_POS: &str = "?:0: ";

/// Error text raised when the fuel budget of a run is exhausted.
pub const FUEL_ERROR: &str = "script budget exhausted";
