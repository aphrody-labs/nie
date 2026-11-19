//! `nie bindgen`: every language binding of a C ABI contract.
//!
//! The input is a contract in the format of `nie-abi` (`crates/nie-abi/contract.json` for nie
//! itself). The output is one directory per language, deterministic byte for byte: the same
//! contract always produces the same files, so a drift test can compare them with the committed
//! copy. The cdylib built from the crate that implements the contract is the only runtime
//! dependency of every binding.

mod emit;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail};
use nie_abi::Contract;

pub use emit::{Lang, emit_all};

/// Arguments of `nie bindgen`.
#[derive(Debug, clap::Parser)]
#[command(name = "bindgen", about = "Generate the C, C++, Rust, Python, C#, Go, Bun and Zig bindings of a C ABI contract")]
pub struct Args {
    /// Contract JSON (`nie-abi` format). Default: the contract of nie-abi.
    #[arg(long)]
    pub contract: Option<PathBuf>,
    /// Output directory.
    #[arg(long, default_value = "bindings")]
    pub out: PathBuf,
    /// Shared library name without prefix or extension. Default: the contract name without `_contract`.
    #[arg(long)]
    pub lib: Option<String>,
    /// Languages to emit (comma separated); default all.
    #[arg(long, value_delimiter = ',')]
    pub lang: Vec<String>,
    /// Write nothing; exit with an error when a file on disk differs from the generated text.
    #[arg(long)]
    pub check: bool,
}

/// Loads a contract file and validates it.
///
/// # Errors
/// When the file cannot be read or parsed, or breaks a contract rule.
pub fn load_contract(path: &Path) -> anyhow::Result<Contract> {
    let text = std::fs::read_to_string(path).with_context(|| path.display().to_string())?;
    parse_contract(&text)
}

/// Parses and validates a contract.
///
/// # Errors
/// When the JSON is malformed or a symbol breaks the rules of `nie-abi`.
pub fn parse_contract(text: &str) -> anyhow::Result<Contract> {
    let c: Contract = serde_json::from_str(text).context("contract json")?;
    let mut seen = std::collections::BTreeSet::new();
    for f in &c.functions {
        if !f.name.starts_with("nie_") {
            bail!("symbol {} must start with nie_", f.name);
        }
        if !seen.insert(f.name.as_str()) {
            bail!("duplicate symbol {}", f.name);
        }
        if f.args.iter().any(|a| a.ty == nie_abi::Ty::String) {
            bail!("{}: string is output only", f.name);
        }
        if matches!(f.returns, nie_abi::Ty::Str | nie_abi::Ty::Bytes) {
            bail!("{}: return a `string`", f.name);
        }
    }
    Ok(c)
}

/// The library name of a contract: its name without a `_contract` suffix.
#[must_use]
pub fn default_lib(c: &Contract) -> String {
    c.name.strip_suffix("_contract").unwrap_or(&c.name).to_owned()
}

/// Runs the command.
///
/// # Errors
/// On an unreadable contract, an unknown language, a write error or, with `--check`, drift.
pub fn run(args: &Args) -> anyhow::Result<()> {
    let contract = match &args.contract {
        Some(p) => load_contract(p)?,
        None => parse_contract(nie_abi::CONTRACT_JSON)?,
    };
    let lib = args.lib.clone().unwrap_or_else(|| default_lib(&contract));
    let langs = Lang::parse_list(&args.lang)?;
    let files = emit_all(&contract, &lib, &langs);
    let mut stale = String::new();
    for (rel, text) in &files {
        let path = args.out.join(rel);
        if args.check {
            if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
                let _ = writeln!(stale, "STALE {}", path.display());
            }
        } else {
            std::fs::create_dir_all(path.parent().context("parent")?)?;
            std::fs::write(&path, text).with_context(|| path.display().to_string())?;
            eprintln!("wrote {}", path.display());
        }
    }
    if !stale.is_empty() {
        bail!("generated bindings are out of date:\n{stale}run `nie bindgen` to refresh them");
    }
    Ok(())
}
