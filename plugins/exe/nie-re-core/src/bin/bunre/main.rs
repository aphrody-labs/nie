// SPDX-License-Identifier: Apache-2.0
//! `bunre`: the native half of the Bun OS `re` product. One binary, one subcommand per capability:
//! `pipeline` (cross-OS RE pipeline, feature `pipeline`), `recompile` (static recompilation of
//! console BIOS/ROM images to native executables and WebAssembly over the pipeline), plus the
//! host probes and binary inspectors. Steam depot acquisition is `nie-steam-depot` (crate `nie-steam`).

#[cfg(feature = "decompile")]
mod decompile;
mod inspect;
#[cfg(feature = "pipeline")]
mod pipeline;

use std::ffi::OsString;
use std::process::ExitCode;

const USAGE: &str =
    "usage: bunre <pipeline|recompile|cpu|virtualization|disasm|strings|decompile> ...";

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let Some(command) = args.next() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    match command.to_str() {
        Some("cpu") => {
            let cpu_info = nie_re_core::cpu::detect_cpu();
            match serde_json::to_string_pretty(&cpu_info) {
                Ok(json) => {
                    println!("{json}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("bunre cpu error: {e}");
                    ExitCode::from(1)
                }
            }
        }
        Some("virtualization") => {
            let virt_info = nie_re_core::virtualization::detect_virtualization();
            match serde_json::to_string_pretty(&virt_info) {
                Ok(json) => {
                    println!("{json}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("bunre virtualization error: {e}");
                    ExitCode::from(1)
                }
            }
        }
        Some("disasm") => inspect::disasm(args),
        Some("strings") => inspect::strings(args),
        #[cfg(feature = "decompile")]
        Some("decompile") => decompile::main(args),
        #[cfg(not(feature = "decompile"))]
        Some("decompile") => {
            eprintln!(
                "bunre: this build lacks the `decompile` feature (build with --features decompile)"
            );
            ExitCode::from(2)
        }
        #[cfg(feature = "pipeline")]
        Some("pipeline") => match args
            .map(OsString::into_string)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(args) => pipeline::main(args),
            Err(_) => {
                eprintln!("bunre pipeline: arguments must be valid Unicode");
                ExitCode::from(2)
            }
        },
        #[cfg(feature = "pipeline")]
        Some("recompile") => match args
            .map(OsString::into_string)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(args) => pipeline::recompile(args),
            Err(_) => {
                eprintln!("bunre recompile: arguments must be valid Unicode");
                ExitCode::from(2)
            }
        },
        #[cfg(not(feature = "pipeline"))]
        Some("pipeline" | "recompile") => {
            eprintln!("bunre: this build lacks the `pipeline` feature");
            ExitCode::from(2)
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
