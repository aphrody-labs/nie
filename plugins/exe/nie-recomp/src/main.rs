//! `nie-recomp` command line.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use nie_recomp::{Image, inventory, source};

#[derive(Parser)]
#[command(version, about = "Static recompiler for nie.exe")]
struct Cli {
    /// Game binary: a local path or `ssh://host/absolute/path` (read in place, never copied to disk).
    #[arg(long, env = "NIE_EXE", global = true)]
    exe: Option<String>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Measure the binary and print the JSON inventory.
    Inventory {
        /// Write the JSON here instead of stdout.
        #[arg(long)]
        out: Option<std::path::PathBuf>,
    },
    /// Lift every function, compile an object file and report coverage.
    Lift {
        /// Directory for `recomp.o` and `lift.json` (outside the repository).
        #[arg(long)]
        out_dir: std::path::PathBuf,
        /// Only the first N functions.
        #[arg(long)]
        limit: Option<usize>,
        /// Run the Cranelift IR verifier (slower).
        #[arg(long)]
        verify: bool,
    },
}

fn load(cli: &Cli) -> Result<Image> {
    let spec = cli.exe.as_deref().context("set --exe or NIE_EXE")?;
    Image::parse(&source::load(spec)?)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match &cli.cmd {
        Cmd::Inventory { out } => {
            let json = serde_json::to_string_pretty(&inventory::build(&load(&cli)?))?;
            match out {
                Some(p) => std::fs::write(p, json).with_context(|| p.display().to_string())?,
                None => println!("{json}"),
            }
        }
        Cmd::Lift { out_dir, limit, verify } => lift(&load(&cli)?, out_dir, *limit, *verify)?,
    }
    Ok(())
}

fn lift(img: &Image, out_dir: &std::path::Path, limit: Option<usize>, verify: bool) -> Result<()> {
    use nie_recomp::{emit, lift::Plan};
    let t0 = std::time::Instant::now();
    let plan = Plan::new(img);
    let roots: Vec<_> = plan.roots.iter().take(limit.unwrap_or(usize::MAX)).collect();
    let mut m = emit::object_module(emit::host_isa(true, verify)?, "nie_recomp")?;
    let unit = emit::compile(&mut m, img, &plan, &roots)?;
    emit::define_table(&mut m, &unit)?;
    let obj = m.finish().emit()?;
    std::fs::create_dir_all(out_dir)?;
    std::fs::write(out_dir.join("recomp.o"), &obj)?;
    let report = serde_json::json!({
        "roots": plan.roots.len(),
        "attempted": roots.len(),
        "lifted_and_compiled": unit.lifted.len(),
        "percent_of_roots": (unit.lifted.len() as f64 * 10000.0 / plan.roots.len() as f64).round() / 100.0,
        "failed": unit.failed.len(),
        "trap_stubs": unit.stubs,
        "code_bytes": unit.code_bytes,
        "object_bytes": obj.len(),
        "seconds": t0.elapsed().as_secs_f64(),
        "failure_classes": unit.failure_classes().into_iter().take(60).collect::<Vec<_>>(),
        "lifted": unit.lifted.iter().map(|(va, _, mt)| serde_json::json!({
            "va": va, "insns": mt.insns, "callees": mt.callees, "dynamic": mt.dynamic,
        })).collect::<Vec<_>>(),
        "failures": unit.failed.iter().map(|(va, w)| (va, &w.0)).collect::<Vec<_>>(),
    });
    std::fs::write(out_dir.join("lift.json"), serde_json::to_vec(&report)?)?;
    let mut brief = report.clone();
    for k in ["lifted", "failures"] {
        brief.as_object_mut().map(|o| o.remove(k));
    }
    println!("{}", serde_json::to_string_pretty(&brief)?);
    Ok(())
}
