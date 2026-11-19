//! Static recompilation of `nie.exe`: measure it, lift x86-64 to Cranelift, emit an object, verify it.

pub mod emit;
pub mod image;
pub mod inventory;
pub mod lift;
pub mod pdata;
pub mod source;

pub use image::Image;
