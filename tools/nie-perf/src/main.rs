//! `nie-perf`: synthetic-fixture benchmarks of the nie hot paths. No game byte is read or written.
//!
//! ```text
//! nie-perf vfs prepare --root DIR [--packs 8] [--files 1500]
//! nie-perf vfs read    --root DIR [--rounds 3] [--parallel]            # Vfs::read of every file
//! nie-perf vfs extract --root DIR --out DIR [--parallel]               # the `nie vfs extract` loop
//! nie-perf texture [--mode via-png|via-png-default|direct] [--size 1024] [--count 16]
//! nie-perf frame [--frames 600] [--width 1280] [--height 720] [--timestamps on|off]   # feature gpu
//! ```
//!
//! Each run prints one JSON object on stdout whose `blake3` field is the equality witness across
//! modes and across builds. `hyperfine --warmup 3 --runs 20 --export-json` wraps the commands.

use std::collections::HashMap;

use anyhow::{Context, Result, bail};

mod texture;
mod vfs;

#[cfg(feature = "gpu")]
mod frame;

/// `--key value` and `--flag` arguments after the subcommand words.
pub struct Args {
    words: Vec<String>,
    values: HashMap<String, String>,
}

impl Args {
    fn parse(raw: &[String]) -> Self {
        let mut words = Vec::new();
        let mut values = HashMap::new();
        let mut i = 0;
        while i < raw.len() {
            if let Some(key) = raw[i].strip_prefix("--") {
                let has_value = raw.get(i + 1).is_some_and(|v| !v.starts_with("--"));
                if has_value {
                    values.insert(key.to_owned(), raw[i + 1].clone());
                    i += 2;
                } else {
                    values.insert(key.to_owned(), String::new());
                    i += 1;
                }
            } else {
                words.push(raw[i].clone());
                i += 1;
            }
        }
        Self { words, values }
    }

    pub fn word(&self, index: usize) -> Option<&str> {
        self.words.get(index).map(String::as_str)
    }

    pub fn flag(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    pub fn string(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn number<T: std::str::FromStr>(&self, key: &str, default: T) -> Result<T> {
        match self.values.get(key) {
            None => Ok(default),
            Some(raw) => raw
                .parse()
                .map_err(|_| anyhow::anyhow!("--{key}: invalid number `{raw}`")),
        }
    }

    pub fn path(&self, key: &str) -> Result<std::path::PathBuf> {
        self.string(key)
            .filter(|v| !v.is_empty())
            .map(std::path::PathBuf::from)
            .with_context(|| format!("--{key} is required"))
    }
}

/// Percentile `p` (0..=100) of an unsorted sample, nearest rank.
pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let rank = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

fn main() -> Result<()> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let args = Args::parse(&raw);
    let report = match args.word(0) {
        Some("vfs") => vfs::run(&args)?,
        Some("texture") => texture::run(&args)?,
        #[cfg(feature = "gpu")]
        Some("frame") => frame::run(&args)?,
        other => bail!("usage: nie-perf <vfs|texture|frame> ... (got {other:?})"),
    };
    println!("{report}");
    Ok(())
}
