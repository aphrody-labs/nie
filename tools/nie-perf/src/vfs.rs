//! VFS benchmark on a synthetic installation: `packs` CPK archives of `files` pseudo-random files.

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use nie_formats::cpk_encode::{CpkWriteEntry, encode_cpk};
use nie_formats::cpk_list::{CpkListEntry, encode as encode_cpk_list};
use nie_formats::vfs::Vfs;
use rayon::prelude::*;
use serde_json::{Value as Json, json};

use crate::Args;

const GROUPS: [&str; 4] = ["menu", "chara", "sound", "text"];

/// xorshift64*: a fixed seed per file keeps the fixture byte-identical across runs and builds.
fn payload(pack: usize, index: usize) -> Vec<u8> {
    let len = 2048 + (index * 977 + pack * 131) % 61_440;
    let mut state = 0x9E37_79B9_7F4A_7C15u64
        ^ ((pack as u64) << 32 | (index as u64 + 1)).wrapping_mul(0xD6E8_FEB8_6659_FD93);
    (0..len)
        .map(|_| {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            (state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 56) as u8
        })
        .collect()
}

fn pack_name(pack: usize) -> String {
    format!("perf_{pack:02}.cpk")
}

fn prepare(root: &Path, packs: usize, files: usize) -> Result<usize> {
    let packs_dir = root.join("data/packs");
    std::fs::create_dir_all(&packs_dir).context("create data/packs")?;
    let mut children = Vec::with_capacity(packs * files);
    for pack in 0..packs {
        let entries: Vec<CpkWriteEntry> = (0..files)
            .map(|index| CpkWriteEntry {
                directory: format!("data/common/{}", GROUPS[(index + pack) % GROUPS.len()]),
                filename: format!("file_{pack:02}_{index:05}.bin"),
                data: payload(pack, index),
            })
            .collect();
        let name = pack_name(pack);
        std::fs::write(
            packs_dir.join(&name),
            encode_cpk(&entries).map_err(anyhow::Error::msg)?,
        )
        .context("write cpk")?;
        for file in &entries {
            children.push(CpkListEntry {
                path: format!("{}/{}", file.directory, file.filename),
                pack: name.clone(),
                size: u32::try_from(file.data.len()).context("file size")?,
            });
        }
    }
    let count = children.len();
    std::fs::write(root.join("data/cpk_list.cfg.bin"), encode_cpk_list(&children))
        .context("write cpk_list.cfg.bin")?;
    Ok(count)
}

/// `syscr` and `syscw` of this process (Linux), the read and write syscall counters.
fn proc_io() -> Option<(u64, u64)> {
    let text = std::fs::read_to_string("/proc/self/io").ok()?;
    let field = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|v| v.trim().parse().ok())
    };
    Some((field("syscr:")?, field("syscw:")?))
}

fn write_one(out: &Path, internal: &str, data: &[u8]) -> Result<()> {
    let rel = internal.strip_prefix("data/common").unwrap_or(internal).trim_start_matches('/');
    let dest = out.join(rel);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&dest, data).with_context(|| format!("write {}", dest.display()))
}

pub fn run(args: &Args) -> Result<Json> {
    let mode = args.word(1).unwrap_or("read");
    let root = args.path("root")?;
    if mode == "prepare" {
        let packs = args.number("packs", 8usize)?;
        let files = args.number("files", 1500usize)?;
        let count = prepare(&root, packs, files)?;
        return Ok(json!({"cmd": "vfs prepare", "packs": packs, "files": count}));
    }
    if mode != "read" && mode != "extract" {
        bail!("vfs mode must be prepare, read or extract");
    }
    let out: Option<PathBuf> = if mode == "extract" { Some(args.path("out")?) } else { None };
    let rounds = args.number("rounds", 3usize)?.max(1);
    let parallel = args.flag("parallel");

    let mut vfs = Vfs::new();
    vfs.init(root.join("data")).context("Vfs::init")?;
    let mut paths: Vec<String> = vfs.iter().map(|(p, _)| p.to_owned()).collect();
    paths.sort();
    if paths.is_empty() {
        bail!("the VFS index of {} is empty", root.display());
    }

    let io_before = proc_io();
    let start = Instant::now();
    let mut digests: Vec<[u8; 32]> = Vec::new();
    let mut bytes = 0u64;
    for round in 0..rounds {
        let one = |path: &String| -> Result<(u64, [u8; 32])> {
            let data = vfs.read(path).map_err(|e| anyhow::anyhow!("{path}: {e}"))?;
            if let Some(out) = &out {
                write_one(out, path, &data)?;
            }
            Ok((data.len() as u64, *blake3::hash(&data).as_bytes()))
        };
        let results: Vec<Result<(u64, [u8; 32])>> = if parallel {
            paths.par_iter().map(one).collect()
        } else {
            paths.iter().map(one).collect()
        };
        let mut this = Vec::with_capacity(results.len());
        let mut total = 0u64;
        for r in results {
            let (len, digest) = r?;
            total += len;
            this.push(digest);
        }
        if round == 0 {
            digests = this;
            bytes = total;
        } else if digests != this {
            bail!("round {round} returned different bytes");
        }
    }
    let elapsed = start.elapsed();
    let io_after = proc_io();

    let mut witness = blake3::Hasher::new();
    for (path, digest) in paths.iter().zip(&digests) {
        witness.update(path.as_bytes());
        witness.update(digest);
    }
    let reads = (paths.len() * rounds) as f64;
    Ok(json!({
        "cmd": format!("vfs {mode}"),
        "files": paths.len(),
        "bytes_per_round": bytes,
        "rounds": rounds,
        "parallel": parallel,
        "threads": if parallel { rayon::current_num_threads() } else { 1 },
        "elapsed_ms": elapsed.as_secs_f64() * 1e3,
        "us_per_file": elapsed.as_secs_f64() * 1e6 / reads,
        "read_syscalls": io_before.zip(io_after).map(|(a, b)| b.0 - a.0),
        "write_syscalls": io_before.zip(io_after).map(|(a, b)| b.1 - a.1),
        "blake3": witness.finalize().to_hex().to_string(),
    }))
}
