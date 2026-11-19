//! `nie fm` without the rest of the `nie` binary: the explorer alone, for hosts that build only
//! this crate (timings and screenshots on the build host).
//!
//! ```sh
//! cargo run --release -p nie-tui --example fm -- --snapshot 120x40 --select data/common/menu/x
//! ```

use anyhow::{Result, bail};
use nie_tui::app::{FmOptions, run_fm};

fn main() -> Result<()> {
    let mut options = FmOptions::default();
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| anyhow::anyhow!("{name} needs a value"));
        match flag.as_str() {
            "--source" => options.source = Some(value("--source")?),
            "--keymap" => options.keymap = Some(value("--keymap")?.into()),
            "--protocol" => options.protocol = Some(value("--protocol")?),
            "--cd" => options.cd = Some(value("--cd")?),
            "--select" => options.select = Some(value("--select")?),
            "--snapshot" => options.snapshot = Some(value("--snapshot")?),
            "--metrics" => options.metrics = Some(value("--metrics")?.into()),
            "--no-cache" => options.no_cache = true,
            other => bail!("unknown option {other}"),
        }
    }
    run_fm(&options)
}
