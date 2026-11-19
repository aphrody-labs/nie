#![no_std]
#![deny(unsafe_code)]

use ostd::prelude::*;

#[ostd::main]
fn kernel_main() {
    println!("nie-os {} booted", env!("CARGO_PKG_VERSION"));
}
