//! `nie-health <url> [--contains TEXT] [--timeout SECS]`: exit 0 when the GET answers 200 (and
//! the body contains TEXT), 1 otherwise. The HEALTHCHECK of every nie image.

use std::process::ExitCode;
use std::time::Duration;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut url = None;
    let mut needle = None;
    let mut timeout = 5u64;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--contains" => needle = args.next(),
            "--timeout" => timeout = args.next().and_then(|v| v.parse().ok()).unwrap_or(timeout),
            "-h" | "--help" => {
                println!("usage: nie-health <http://host:port/path> [--contains TEXT] [--timeout SECS]");
                return ExitCode::SUCCESS;
            }
            _ => url = Some(arg),
        }
    }
    let Some(url) = url else {
        eprintln!("usage: nie-health <http://host:port/path> [--contains TEXT] [--timeout SECS]");
        return ExitCode::from(2);
    };
    match nie_health::get(&url, Duration::from_secs(timeout)) {
        Ok(answer) if nie_health::passes(&answer, needle.as_deref()) => ExitCode::SUCCESS,
        Ok(answer) => {
            eprintln!("{url}: status {} body {:.200}", answer.status, answer.body);
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
