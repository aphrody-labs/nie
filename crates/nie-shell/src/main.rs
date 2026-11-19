//! `nie-shell contract` prints `contract/nie-app.json`.

use std::io::Write;
use std::process::ExitCode;

fn main() -> ExitCode {
    let text = match std::env::args().nth(1).as_deref() {
        Some("contract") => nie_shell::contract_json(),
        _ => {
            eprintln!("usage: nie-shell contract");
            return ExitCode::from(2);
        }
    };
    match std::io::stdout().write_all(text.as_bytes()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("nie-shell: {error}");
            ExitCode::FAILURE
        }
    }
}
